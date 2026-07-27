mod app;
mod assembly;
mod canvas_state;
mod component_detail_state;
mod components;
mod config;
mod cost_config;
mod dialog_state;
mod file_dialog;
mod fluid;
mod glyphs;
mod grid;
mod input;
mod palette_state;
mod selection_state;
mod sim_state;
mod simulation;
mod text_input_state;
mod ui;
mod undo_state;

use std::{
    io,
    sync::mpsc,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode, GRID_COLS_MIN, GRID_ROWS_MIN};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Size the grid to fill the current terminal, with enforced minimums.
    let initial_size = terminal.size()?;
    let grid_cols = ((initial_size.width as f32 * 0.72) as usize)
        .saturating_sub(2)
        .max(GRID_COLS_MIN);
    let grid_rows = (initial_size.height.saturating_sub(11) as usize).max(GRID_ROWS_MIN);
    let mut app = App::new(grid_cols, grid_rows);
    app.load_config();
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // Background simulation thread — receives (Grid, FluidType, GlyphRegistry) jobs,
    // returns SimResult without blocking the UI render loop.
    let (sim_tx, sim_rx) = mpsc::channel::<(grid::Grid, fluid::FluidType, glyphs::GlyphRegistry)>();
    let (res_tx, res_rx) = mpsc::channel::<simulation::SimResult>();
    std::thread::spawn(move || {
        for (g, fluid_type, registry) in sim_rx {
            let result = simulation::simulate(&g, fluid_type, &registry);
            let _ = res_tx.send(result);
        }
    });
    let mut sim_pending = false;

    // Timing log: written to debug_timing.log in the working directory.
    // Each line: render_ms,handle_ms,event_kind,key_code
    // Delete this file and the instrumentation block when the bottleneck is identified.
    let mut timing_log = std::fs::File::create("debug_timing.log")
        .map(std::io::BufWriter::new)
        .ok();

    loop {
        let size = terminal.size()?;
        let canvas_h = size.height.saturating_sub(11) as usize;
        let canvas_w = (size.width as f32 * 0.72) as usize - 2;

        // Non-blocking: pick up any finished sim result before rendering.
        if let Ok(result) = res_rx.try_recv() {
            if matches!(app.mode, AppMode::Simulating | AppMode::Paused) && !app.sim.sim_refreshed {
                app.sim.sim_result = Some(result);
            }
            sim_pending = false;
        }

        let t_render = Instant::now();
        let mut phase = ui::RenderPhaseUs::default();
        terminal.draw(|f| { phase = ui::render(f, &app); })?;
        let render_ms = t_render.elapsed().as_micros();

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let kind_label = match key.kind {
                    KeyEventKind::Press   => "Press",
                    KeyEventKind::Repeat  => "Repeat",
                    KeyEventKind::Release => "Release",
                };
                let t_handle = Instant::now();
                // Only act on Press and Repeat — Release has no effect.
                if key.kind != KeyEventKind::Release {
                    input::handle_key(&mut app, key.code, key.modifiers, canvas_h, canvas_w);
                }
                let handle_us = t_handle.elapsed().as_micros();

                if let Some(ref mut log) = timing_log {
                    use std::io::Write;
                    let _ = writeln!(
                        log,
                        "render={render_ms}µs handle={handle_us}µs kind={kind_label} key={:?}  [label={}µs flood={}µs loop={}µs para={}µs bars={}µs pal={}µs foot={}µs]",
                        key.code,
                        phase.label_overlay, phase.flood_candidates, phase.span_loop,
                        phase.paragraph_render, phase.scrollbars, phase.palette_us, phase.footer_us,
                    );
                    let _ = log.flush();
                }

                // If handle_key triggered refresh_sim(), drain any stale in-flight result.
                if app.sim.sim_refreshed {
                    while res_rx.try_recv().is_ok() {}
                    sim_pending = false;
                    app.sim.sim_refreshed = false;
                }

                // DWV validation is cheap — refresh every key event when dwv_mode is on.
                if app.dwv_mode {
                    app.refresh_dwv();
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            // Send a new sim job every 4 ticks (800 ms) while simulating, but only
            // when the previous job has been picked up to avoid piling up work.
            if app.mode == AppMode::Simulating && !sim_pending && app.tick.is_multiple_of(4) {
                let _ = sim_tx.send((
                    app.canvas.grid.clone(),
                    app.sim.fluid_type,
                    app.glyph_registry.clone(),
                ));
                sim_pending = true;
            }
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
