use std::collections::HashMap;
use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, AppMode};
use crate::components::{ComponentKind, ValveState};
use crate::fluid::FluidType;
use crate::glyphs::GlyphRegistry;
use crate::simulation::FlowState;

use super::{composite_box_char, fluid_bg, fluid_fg, scale_rgb, RenderPhaseUs};
use super::annotations::compute_annotations;

/// Pre-flattened view of SimResult for O(1) per-cell access during rendering.
/// Built once per frame from the sim HashMaps; avoids ~6 k hash lookups in the
/// inner render loop, which matters most on lower-spec machines.
struct SimFlat {
    w:      usize,
    states: Vec<FlowState>,     // [grid_h * grid_w], default Static
    flow:   Vec<(f32, f32)>,    // (gpm, velocity_fps)
    dirs:   Vec<(i8,  i8)>,     // flow direction
}

impl SimFlat {
    fn build(sim: &crate::simulation::SimResult, grid_h: usize, grid_w: usize) -> Self {
        let n = grid_h * grid_w;
        let mut states = vec![FlowState::Static; n];
        let mut flow   = vec![(0.0f32, 0.0f32); n];
        let mut dirs   = vec![(0i8, 0i8); n];
        for (&(r, c), &s) in &sim.cell_states {
            if r < grid_h && c < grid_w { states[r * grid_w + c] = s; }
        }
        for (&(r, c), fd) in &sim.flow_data {
            if r < grid_h && c < grid_w { flow[r * grid_w + c] = (fd.flow_gpm, fd.velocity_fps); }
        }
        for (&(r, c), &d) in &sim.flow_dirs {
            if r < grid_h && c < grid_w { dirs[r * grid_w + c] = d; }
        }
        Self { w: grid_w, states, flow, dirs }
    }

    #[inline(always)]
    fn state(&self, r: usize, c: usize) -> FlowState { self.states[r * self.w + c] }
    #[inline(always)]
    fn flow_gpm_vel(&self, r: usize, c: usize) -> (f32, f32) { self.flow[r * self.w + c] }
    #[inline(always)]
    fn flow_dir(&self, r: usize, c: usize) -> (i8, i8) { self.dirs[r * self.w + c] }
}

pub(super) fn render_canvas(f: &mut Frame, app: &App, area: Rect) -> RenderPhaseUs {
    use crate::app::Focus;
    use ratatui::widgets::{Block, Borders, BorderType};

    let focused = app.focus == Focus::Canvas;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let border_type = if focused { BorderType::Thick } else { BorderType::Plain };

    let mode_label = match app.mode {
        AppMode::Splash           => "SPLASH",
        AppMode::Build            => "BUILD",
        AppMode::Simulating       => "SIMULATE",
        AppMode::Paused           => "PAUSED",
        AppMode::GlyphEditor      => "GLYPH EDITOR",
        AppMode::BomView          => "BOM",
        AppMode::Selecting        => "SELECTING",
        AppMode::AssemblyBrowser  => "ASSEMBLIES",
        AppMode::Stamping         => "STAMP",
        AppMode::ComponentDetail  => "DETAIL",
        AppMode::Help             => "HELP",
        AppMode::Settings         => "SETTINGS",
        AppMode::FileDialog       => "FILE",
        AppMode::ConfirmNew       => "NEW?",
        AppMode::ConfirmQuit      => "QUIT?",
        AppMode::ExportDialog     => "EXPORT",
        AppMode::AnnotationDialog => "LABEL/NOTE",
        AppMode::CostEstimator    => "COST",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_style)
        .title(format!(
            " Flow Dynamics  [{mode_label}]  {}  col:{} row:{} ",
            app.sim.fluid_type.label(),
            app.canvas.cursor.1, app.canvas.cursor.0
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let viewport_h = inner.height as usize;
    let viewport_w = inner.width as usize;
    let (vr, vc) = app.canvas.viewport;

    let t_start = Instant::now();

    let annotations: HashMap<(usize, usize), (char, Style)> = if app.show_annotations {
        compute_annotations(app)
    } else {
        HashMap::new()
    };

    let sel_rect = app.selection.select_start.map(|s| {
        let e = app.canvas.cursor;
        (s.0.min(e.0), s.1.min(e.1), s.0.max(e.0), s.1.max(e.1))
    });

    let cursor_anchor = app.canvas.grid.effective_pos(app.canvas.cursor.0, app.canvas.cursor.1);
    let cursor_is_composite = app.canvas.grid.get(cursor_anchor.0, cursor_anchor.1)
        .map(|c| c.effective_is_composite())
        .unwrap_or(false);

    // Pre-compute annotation overlay: Label/Note components spread bordered text
    // across empty cells to the right of their anchor.
    // Label: anchor='[', then text chars, then ']'
    // Note:  anchor='[', then '†', then ']'
    let mut label_overlay: std::collections::HashMap<(usize, usize), (char, Style)> =
        std::collections::HashMap::new();
    {
        let label_style = Style::default().fg(Color::Rgb(255, 230, 60)).add_modifier(Modifier::BOLD);
        let note_style  = Style::default().fg(Color::Rgb(80, 220, 230)).add_modifier(Modifier::BOLD);
        let link_style  = Style::default().fg(Color::Rgb(255, 185, 55)).add_modifier(Modifier::BOLD);

        let cell_empty = |r: usize, c: usize| {
            app.canvas.grid.get(r, c).is_none() && app.canvas.grid.satellite_anchor(r, c).is_none()
        };

        for sr in 0..viewport_h {
            let gr = vr + sr;
            for sc in 0..viewport_w {
                let gc = vc + sc;
                if let Some(comp) = app.canvas.grid.get(gr, gc) {
                    match comp.kind {
                        ComponentKind::Label => {
                            if let Some(text) = &comp.text {
                                // anchor shows '['; text chars at gc+1..gc+N; ']' at gc+N+1
                                let mut ok = true;
                                for (i, ch) in text.chars().enumerate() {
                                    let tc = gc + i + 1;
                                    if tc >= vc + viewport_w || !cell_empty(gr, tc) { ok = false; break; }
                                    label_overlay.insert((gr, tc), (ch, label_style));
                                }
                                if ok {
                                    let close_c = gc + text.chars().count() + 1;
                                    if close_c < vc + viewport_w && cell_empty(gr, close_c) {
                                        label_overlay.insert((gr, close_c), (']', label_style));
                                    }
                                }
                            }
                        }
                        ComponentKind::Note => {
                            if let Some(text) = &comp.text {
                                let segs: Vec<&str> = text.split('\n').collect();
                                let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                                // inner_w = 1 left-pad + max_w text + 1 right-pad
                                let inner_w = max_w + 2;
                                let right_c = gc + inner_w + 1;

                                // Top border: anchor is *; ═×inner_w; ╗
                                for ci in 1..=inner_w {
                                    let col = gc + ci;
                                    if col >= vc + viewport_w || !cell_empty(gr, col) { break; }
                                    label_overlay.insert((gr, col), ('═', note_style));
                                }
                                if right_c < vc + viewport_w && cell_empty(gr, right_c) {
                                    label_overlay.insert((gr, right_c), ('╗', note_style));
                                }

                                // Top blank padding row
                                {
                                    let row = gr + 1;
                                    if row < vr + viewport_h {
                                        if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', note_style)); }
                                        for ci in 1..=inner_w {
                                            let col = gc + ci;
                                            if col < vc + viewport_w && cell_empty(row, col) {
                                                label_overlay.insert((row, col), (' ', note_style));
                                            }
                                        }
                                        if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                            label_overlay.insert((row, right_c), ('║', note_style));
                                        }
                                    }
                                }

                                // Content rows: ║ space text space ║
                                for (li, seg) in segs.iter().enumerate() {
                                    let row = gr + li + 2;
                                    if row >= vr + viewport_h { break; }
                                    if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', note_style)); }
                                    if gc + 1 < vc + viewport_w && cell_empty(row, gc + 1) {
                                        label_overlay.insert((row, gc + 1), (' ', note_style));
                                    }
                                    let chars: Vec<char> = seg.chars().collect();
                                    let mut ok = true;
                                    for ci in 0..max_w {
                                        let col = gc + 2 + ci;
                                        if col >= vc + viewport_w { ok = false; break; }
                                        if !cell_empty(row, col) { ok = false; break; }
                                        label_overlay.insert((row, col), (chars.get(ci).copied().unwrap_or(' '), note_style));
                                    }
                                    let rpad = gc + max_w + 2;
                                    if ok && rpad < vc + viewport_w && cell_empty(row, rpad) {
                                        label_overlay.insert((row, rpad), (' ', note_style));
                                    }
                                    if ok && right_c < vc + viewport_w && cell_empty(row, right_c) {
                                        label_overlay.insert((row, right_c), ('║', note_style));
                                    }
                                }

                                // Bottom blank padding row
                                {
                                    let row = gr + segs.len() + 2;
                                    if row < vr + viewport_h {
                                        if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', note_style)); }
                                        for ci in 1..=inner_w {
                                            let col = gc + ci;
                                            if col < vc + viewport_w && cell_empty(row, col) {
                                                label_overlay.insert((row, col), (' ', note_style));
                                            }
                                        }
                                        if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                            label_overlay.insert((row, right_c), ('║', note_style));
                                        }
                                    }
                                }

                                // Bottom border: ╚═══╝
                                let bot_row = gr + segs.len() + 3;
                                if bot_row < vr + viewport_h {
                                    if cell_empty(bot_row, gc) { label_overlay.insert((bot_row, gc), ('╚', note_style)); }
                                    let mut ok = true;
                                    for ci in 1..=inner_w {
                                        let col = gc + ci;
                                        if col >= vc + viewport_w { ok = false; break; }
                                        if !cell_empty(bot_row, col) { ok = false; break; }
                                        label_overlay.insert((bot_row, col), ('═', note_style));
                                    }
                                    if ok && right_c < vc + viewport_w && cell_empty(bot_row, right_c) {
                                        label_overlay.insert((bot_row, right_c), ('╝', note_style));
                                    }
                                }

                                // [E]dit hint on the row above when cursor is on this note.
                                if app.canvas.cursor == (gr, gc) && gr > vr {
                                    let edit_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                                    for (hi, ch) in ['[', 'E', ']', 'd', 'i', 't'].iter().enumerate() {
                                        let col = gc + hi;
                                        if col < vc + viewport_w {
                                            label_overlay.insert((gr - 1, col), (*ch, edit_style));
                                        }
                                    }
                                }
                            }
                        }
                        ComponentKind::Link => {
                            let path_text = comp.text.as_deref().unwrap_or("(no path)");
                            let text_w = path_text.chars().count();
                            let inner_w = text_w + 2;
                            let right_c = gc + inner_w + 1;

                            // Top border: ⇒═══╗ (anchor at gc rendered by cell_char_and_style)
                            for ci in 1..=inner_w {
                                let col = gc + ci;
                                if col >= vc + viewport_w || !cell_empty(gr, col) { break; }
                                label_overlay.insert((gr, col), ('═', link_style));
                            }
                            if right_c < vc + viewport_w && cell_empty(gr, right_c) {
                                label_overlay.insert((gr, right_c), ('╗', link_style));
                            }

                            // Blank padding row
                            {
                                let row = gr + 1;
                                if row < vr + viewport_h {
                                    if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', link_style)); }
                                    for ci in 1..=inner_w {
                                        let col = gc + ci;
                                        if col < vc + viewport_w && cell_empty(row, col) {
                                            label_overlay.insert((row, col), (' ', link_style));
                                        }
                                    }
                                    if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                        label_overlay.insert((row, right_c), ('║', link_style));
                                    }
                                }
                            }

                            // Content row: ║ path ║
                            {
                                let row = gr + 2;
                                if row < vr + viewport_h {
                                    if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', link_style)); }
                                    if gc + 1 < vc + viewport_w && cell_empty(row, gc + 1) {
                                        label_overlay.insert((row, gc + 1), (' ', link_style));
                                    }
                                    let chars: Vec<char> = path_text.chars().collect();
                                    let mut ok = true;
                                    for (ci, &ch) in chars.iter().enumerate().take(text_w) {
                                        let col = gc + 2 + ci;
                                        if col >= vc + viewport_w { ok = false; break; }
                                        if !cell_empty(row, col) { ok = false; break; }
                                        label_overlay.insert((row, col), (ch, link_style));
                                    }
                                    let rpad = gc + text_w + 2;
                                    if ok && rpad < vc + viewport_w && cell_empty(row, rpad) {
                                        label_overlay.insert((row, rpad), (' ', link_style));
                                    }
                                    if ok && right_c < vc + viewport_w && cell_empty(row, right_c) {
                                        label_overlay.insert((row, right_c), ('║', link_style));
                                    }
                                }
                            }

                            // Blank padding row
                            {
                                let row = gr + 3;
                                if row < vr + viewport_h {
                                    if cell_empty(row, gc) { label_overlay.insert((row, gc), ('║', link_style)); }
                                    for ci in 1..=inner_w {
                                        let col = gc + ci;
                                        if col < vc + viewport_w && cell_empty(row, col) {
                                            label_overlay.insert((row, col), (' ', link_style));
                                        }
                                    }
                                    if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                        label_overlay.insert((row, right_c), ('║', link_style));
                                    }
                                }
                            }

                            // Bottom border: ╚════╝
                            let bot_row = gr + 4;
                            if bot_row < vr + viewport_h {
                                if cell_empty(bot_row, gc) { label_overlay.insert((bot_row, gc), ('╚', link_style)); }
                                let mut ok = true;
                                for ci in 1..=inner_w {
                                    let col = gc + ci;
                                    if col >= vc + viewport_w { ok = false; break; }
                                    if !cell_empty(bot_row, col) { ok = false; break; }
                                    label_overlay.insert((bot_row, col), ('═', link_style));
                                }
                                if ok && right_c < vc + viewport_w && cell_empty(bot_row, right_c) {
                                    label_overlay.insert((bot_row, right_c), ('╝', link_style));
                                }
                            }

                            // [Enter]/[E] hint above anchor when cursor is here.
                            if app.canvas.cursor == (gr, gc) && gr > vr {
                                let hint_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                                for (hi, ch) in ['[', 'E', ']', 'e', 'd', 'i', 't', ' ', 'p', 'a', 't', 'h'].iter().enumerate() {
                                    let col = gc + hi;
                                    if col < vc + viewport_w {
                                        label_overlay.insert((gr - 1, col), (*ch, hint_style));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Pending annotation preview: spread text at cursor as if already placed.
        if let Some((kind, text)) = &app.text_input.pending_annotation {
            let (cr, cc) = app.canvas.cursor;
            match kind {
                ComponentKind::Label => {
                    let mut ok = true;
                    for (i, ch) in text.chars().enumerate() {
                        let tc = cc + i + 1;
                        if tc >= vc + viewport_w || !cell_empty(cr, tc) { ok = false; break; }
                        label_overlay.insert((cr, tc), (ch, label_style));
                    }
                    if ok {
                        let close_c = cc + text.chars().count() + 1;
                        if close_c < vc + viewport_w && cell_empty(cr, close_c) {
                            label_overlay.insert((cr, close_c), (']', label_style));
                        }
                    }
                }
                ComponentKind::Note => {
                    let segs: Vec<&str> = text.split('\n').collect();
                    let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                    let inner_w = max_w + 2;
                    let right_c = cc + inner_w + 1;

                    // Top border: cursor cell is *; ═×inner_w; ╗
                    for ci in 1..=inner_w {
                        let col = cc + ci;
                        if col >= vc + viewport_w || !cell_empty(cr, col) { break; }
                        label_overlay.insert((cr, col), ('═', note_style));
                    }
                    if right_c < vc + viewport_w && cell_empty(cr, right_c) {
                        label_overlay.insert((cr, right_c), ('╗', note_style));
                    }

                    // Top blank padding row
                    {
                        let row = cr + 1;
                        if row < vr + viewport_h {
                            if cell_empty(row, cc) { label_overlay.insert((row, cc), ('║', note_style)); }
                            for ci in 1..=inner_w {
                                let col = cc + ci;
                                if col < vc + viewport_w && cell_empty(row, col) {
                                    label_overlay.insert((row, col), (' ', note_style));
                                }
                            }
                            if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                label_overlay.insert((row, right_c), ('║', note_style));
                            }
                        }
                    }

                    // Content rows: ║ space text space ║
                    for (li, seg) in segs.iter().enumerate() {
                        let row = cr + li + 2;
                        if row >= vr + viewport_h { break; }
                        if cell_empty(row, cc) { label_overlay.insert((row, cc), ('║', note_style)); }
                        if cc + 1 < vc + viewport_w && cell_empty(row, cc + 1) {
                            label_overlay.insert((row, cc + 1), (' ', note_style));
                        }
                        let chars: Vec<char> = seg.chars().collect();
                        let mut ok = true;
                        for ci in 0..max_w {
                            let col = cc + 2 + ci;
                            if col >= vc + viewport_w { ok = false; break; }
                            if !cell_empty(row, col) { ok = false; break; }
                            label_overlay.insert((row, col), (chars.get(ci).copied().unwrap_or(' '), note_style));
                        }
                        let rpad = cc + max_w + 2;
                        if ok && rpad < vc + viewport_w && cell_empty(row, rpad) {
                            label_overlay.insert((row, rpad), (' ', note_style));
                        }
                        if ok && right_c < vc + viewport_w && cell_empty(row, right_c) {
                            label_overlay.insert((row, right_c), ('║', note_style));
                        }
                    }

                    // Bottom blank padding row
                    {
                        let row = cr + segs.len() + 2;
                        if row < vr + viewport_h {
                            if cell_empty(row, cc) { label_overlay.insert((row, cc), ('║', note_style)); }
                            for ci in 1..=inner_w {
                                let col = cc + ci;
                                if col < vc + viewport_w && cell_empty(row, col) {
                                    label_overlay.insert((row, col), (' ', note_style));
                                }
                            }
                            if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                label_overlay.insert((row, right_c), ('║', note_style));
                            }
                        }
                    }

                    // Bottom border: ╚═══╝
                    let bot_row = cr + segs.len() + 3;
                    if bot_row < vr + viewport_h {
                        if cell_empty(bot_row, cc) { label_overlay.insert((bot_row, cc), ('╚', note_style)); }
                        let mut ok = true;
                        for ci in 1..=inner_w {
                            let col = cc + ci;
                            if col >= vc + viewport_w { ok = false; break; }
                            if !cell_empty(bot_row, col) { ok = false; break; }
                            label_overlay.insert((bot_row, col), ('═', note_style));
                        }
                        if ok && right_c < vc + viewport_w && cell_empty(bot_row, right_c) {
                            label_overlay.insert((bot_row, right_c), ('╝', note_style));
                        }
                    }
                }
                ComponentKind::Link => {
                    let text_w = text.chars().count();
                    let inner_w = text_w + 2;
                    let right_c = cc + inner_w + 1;
                    // Top border
                    for ci in 1..=inner_w {
                        let col = cc + ci;
                        if col >= vc + viewport_w || !cell_empty(cr, col) { break; }
                        label_overlay.insert((cr, col), ('═', link_style));
                    }
                    if right_c < vc + viewport_w && cell_empty(cr, right_c) {
                        label_overlay.insert((cr, right_c), ('╗', link_style));
                    }
                    // Blank, content, blank, bottom
                    for (dy, kind_ch) in [(1usize, None), (2, Some(text)), (3usize, None)] {
                        let row = cr + dy;
                        if row >= vr + viewport_h { break; }
                        if cell_empty(row, cc) { label_overlay.insert((row, cc), ('║', link_style)); }
                        if let Some(content) = kind_ch {
                            if cc + 1 < vc + viewport_w && cell_empty(row, cc + 1) {
                                label_overlay.insert((row, cc + 1), (' ', link_style));
                            }
                            let mut ok = true;
                            for (ci, ch) in content.chars().enumerate() {
                                let col = cc + 2 + ci;
                                if col >= vc + viewport_w { ok = false; break; }
                                if !cell_empty(row, col) { ok = false; break; }
                                label_overlay.insert((row, col), (ch, link_style));
                            }
                            let rpad = cc + text_w + 2;
                            if ok && rpad < vc + viewport_w && cell_empty(row, rpad) {
                                label_overlay.insert((row, rpad), (' ', link_style));
                            }
                            if ok && right_c < vc + viewport_w && cell_empty(row, right_c) {
                                label_overlay.insert((row, right_c), ('║', link_style));
                            }
                        } else {
                            for ci in 1..=inner_w {
                                let col = cc + ci;
                                if col < vc + viewport_w && cell_empty(row, col) {
                                    label_overlay.insert((row, col), (' ', link_style));
                                }
                            }
                            if right_c < vc + viewport_w && cell_empty(row, right_c) {
                                label_overlay.insert((row, right_c), ('║', link_style));
                            }
                        }
                    }
                    let bot_row = cr + 4;
                    if bot_row < vr + viewport_h {
                        if cell_empty(bot_row, cc) { label_overlay.insert((bot_row, cc), ('╚', link_style)); }
                        let mut ok = true;
                        for ci in 1..=inner_w {
                            let col = cc + ci;
                            if col >= vc + viewport_w { ok = false; break; }
                            if !cell_empty(bot_row, col) { ok = false; break; }
                            label_overlay.insert((bot_row, col), ('═', link_style));
                        }
                        if ok && right_c < vc + viewport_w && cell_empty(bot_row, right_c) {
                            label_overlay.insert((bot_row, right_c), ('╝', link_style));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let t_after_label = Instant::now();

    // Pre-compute which empty viewport cells show the flood animation.
    // Scanning outward from the small set of Pressurized cells is far cheaper than
    // checking all 4 neighbors inside the inner render loop for every empty cell.
    let flood_candidates: std::collections::HashSet<(usize, usize)> = {
        let mut set = std::collections::HashSet::new();
        if let Some(sim) = &app.sim.sim_result {
            for (&(ar, ac), state) in &sim.cell_states {
                if *state != FlowState::Pressurized { continue; }
                let Some(comp) = app.canvas.grid.get(ar, ac) else { continue };
                if comp.effective_is_composite() || comp.kind.is_sealed_terminal() { continue; }
                let c = comp.connections();
                // (row offset, col offset, connection-open flag)
                for (dr, dc, open) in [(-1i32, 0i32, c.0), (1, 0, c.1), (0, 1, c.2), (0, -1, c.3)] {
                    if !open { continue; }
                    let nr = ar as i32 + dr;
                    let nc = ac as i32 + dc;
                    if nr < 0 || nc < 0 { continue; }
                    let (nr, nc) = (nr as usize, nc as usize);
                    if nr < vr || nr >= vr + viewport_h || nc < vc || nc >= vc + viewport_w { continue; }
                    if nr >= app.canvas.grid.height || nc >= app.canvas.grid.width { continue; }
                    if app.canvas.grid.get(nr, nc).is_some() { continue; }
                    if app.canvas.grid.satellite_anchor(nr, nc).is_some() { continue; }
                    set.insert((nr, nc));
                }
            }
        }
        set
    };

    let t_after_flood = Instant::now();

    // Build the flat sim cache once per frame — Vec index is ~10× faster than
    // HashMap lookup with tuple keys, which is critical on slower machines.
    let sim_flat: Option<SimFlat> = app.sim.sim_result.as_ref().map(|sim| {
        SimFlat::build(sim, app.canvas.grid.height, app.canvas.grid.width)
    });

    let flood_style = Style::default().fg(Color::Rgb(55, 140, 255)).bg(Color::Rgb(0, 15, 45));
    let dot_style   = Style::default().fg(Color::Rgb(35, 35, 35));

    let mut lines: Vec<Line> = Vec::with_capacity(viewport_h);

    for screen_row in 0..viewport_h {
        let grid_row = vr + screen_row;
        // Smaller initial cap — most rows have far fewer spans than cells thanks to batching.
        let mut spans: Vec<Span> = Vec::with_capacity(viewport_w / 4 + 4);
        // Batch consecutive empty-dot cells into one span to reduce ratatui's internal work.
        let mut dot_run: usize = 0;

        macro_rules! flush_dots {
            () => {
                if dot_run > 0 {
                    // One allocation covers the whole run; cheaper on slow machines than N
                    // individual Span pushes, and ratatui processes far fewer spans per row.
                    let mut s = String::with_capacity(dot_run * 2); // "·" is 2 bytes UTF-8
                    for _ in 0..dot_run { s.push('·'); }
                    spans.push(Span::styled(s, dot_style));
                    dot_run = 0;
                }
            };
        }

        for screen_col in 0..viewport_w {
            let grid_col = vc + screen_col;
            let is_cursor = app.canvas.cursor == (grid_row, grid_col);

            let sat_anchor = app.canvas.grid.satellite_anchor(grid_row, grid_col);
            let cell_comp  = app.canvas.grid.get(grid_row, grid_col);
            let cell_anchor = sat_anchor.unwrap_or((grid_row, grid_col));
            let is_satellite = sat_anchor.is_some();
            let effective_cursor = is_cursor
                || (cursor_is_composite && cell_anchor == cursor_anchor);

            // ── Stamp ghost overlay ───────────────────────────────────────
            if app.mode == AppMode::Stamping {
                if let Some(asm) = &app.selection.pending_stamp {
                    let (cr, cc) = app.canvas.cursor;
                    if grid_row >= cr && grid_col >= cc {
                        let ar = grid_row - cr;
                        let ac = grid_col - cc;
                        let ghost_style = Style::default()
                            .fg(Color::Rgb(100, 130, 190))
                            .bg(Color::Rgb(18, 22, 38));
                        if let Some(comp) = asm.get(ar, ac) {
                            let ch = match comp.kind {
                                crate::components::ComponentKind::Label => '[',
                                crate::components::ComponentKind::Note  => '*',
                                crate::components::ComponentKind::Link  => '⇒',
                                _ if comp.effective_is_composite() => {
                                    let (fw, fh) = comp.effective_footprint();
                                    let pr = comp.effective_port_row();
                                    let label = comp.effective_composite_label();
                                    let (_, _, ae, aw) = comp.connections();
                                    super::composite_box_char(fw, fh, pr, pr, 0, label, None, ae || aw)
                                }
                                _ => {
                                    let g = app.glyph_registry.resolve(comp.kind, comp.material, comp.diameter);
                                    g.symbol
                                }
                            };
                            flush_dots!();
                            spans.push(Span::styled(ch.to_string(), ghost_style));
                            continue;
                        }
                        if let Some(ch) = asm.annotation_ghost_char(ar, ac) {
                            flush_dots!();
                            spans.push(Span::styled(ch.to_string(), ghost_style));
                            continue;
                        }
                        if let Some(ch) = asm.composite_ghost_char(ar, ac) {
                            flush_dots!();
                            spans.push(Span::styled(ch.to_string(), ghost_style));
                            continue;
                        }
                    }
                }
            }

            // ── Selection highlight ───────────────────────────────────────
            if let Some((r0, c0, r1, c1)) = sel_rect {
                if grid_row >= r0 && grid_row <= r1 && grid_col >= c0 && grid_col <= c1 {
                    let is_edge = grid_row == r0 || grid_row == r1
                        || grid_col == c0 || grid_col == c1;
                    let sel_bg = if is_edge { Color::Rgb(40, 60, 20) } else { Color::Rgb(20, 35, 10) };
                    if cell_comp.is_none() && !is_satellite {
                        if let Some(&(lch, lstyle)) = label_overlay.get(&(grid_row, grid_col)) {
                            flush_dots!();
                            spans.push(Span::styled(lch.to_string(), lstyle.bg(sel_bg)));
                            continue;
                        }
                    }
                    flush_dots!();
                    let (ch, style) = cell_char_and_style(app, sim_flat.as_ref(), grid_row, grid_col, effective_cursor, sat_anchor, cell_comp);
                    spans.push(Span::styled(ch.to_string(), style.bg(sel_bg)));
                    continue;
                }
            }

            // ── Composite ghost footprint preview ─────────────────────────
            if app.mode == AppMode::Build && cell_comp.is_none() && !is_satellite {
                if let Some((ch, style)) = composite_ghost_cell(app, grid_row, grid_col) {
                    flush_dots!();
                    spans.push(Span::styled(ch.to_string(), style));
                    continue;
                }
            }

            // ── Normal cell ───────────────────────────────────────────────
            if !effective_cursor && cell_comp.is_none() && !is_satellite {
                if let Some(&(lch, lstyle)) = label_overlay.get(&(grid_row, grid_col)) {
                    flush_dots!();
                    spans.push(Span::styled(lch.to_string(), lstyle));
                } else if let Some(&(ach, astyle)) = annotations.get(&(grid_row, grid_col)) {
                    flush_dots!();
                    spans.push(Span::styled(ach.to_string(), astyle));
                } else if flood_candidates.contains(&(grid_row, grid_col)) {
                    let phase = (app.tick as usize)
                        .wrapping_add(grid_row.wrapping_mul(3))
                        .wrapping_add(grid_col.wrapping_mul(7)) % 4;
                    flush_dots!();
                    spans.push(Span::styled(['~', '≈', '~', ' '][phase].to_string(), flood_style));
                } else {
                    dot_run += 1; // accumulate into current run — no allocation here
                }
            } else {
                flush_dots!();
                let (ch, style) = cell_char_and_style(app, sim_flat.as_ref(), grid_row, grid_col, effective_cursor, sat_anchor, cell_comp);
                spans.push(Span::styled(ch.to_string(), style));
            }
        }
        // Inline flush (no reset — dot_run goes out of scope after this row).
        if dot_run > 0 {
            let mut s = String::with_capacity(dot_run * 2);
            for _ in 0..dot_run { s.push('·'); }
            spans.push(Span::styled(s, dot_style));
        }
        lines.push(Line::from(spans));
    }

    let t_after_loop = Instant::now();

    f.render_widget(Paragraph::new(lines), inner);

    let t_after_para = Instant::now();

    // ── Scrollbars ────────────────────────────────────────────────────────────
    let grid_h = app.canvas.grid.height;
    let grid_w = app.canvas.grid.width;

    // Vertical scrollbar on the right edge
    if grid_h > viewport_h && viewport_h > 1 {
        let bar_col = inner.x + inner.width.saturating_sub(1);
        let bar_len = viewport_h;
        let bar_h = ((bar_len * bar_len) / grid_h).max(1).min(bar_len);
        let max_scroll = grid_h.saturating_sub(viewport_h);
        let bar_y = if max_scroll == 0 { 0 } else { vr * (bar_len - bar_h) / max_scroll };
        for i in 0..bar_len {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, col) = if in_bar {
                ('█', Color::Rgb(70, 70, 100))
            } else {
                ('░', Color::Rgb(25, 25, 35))
            };
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(col))),
                Rect::new(bar_col, inner.y + i as u16, 1, 1),
            );
        }
    }

    // Horizontal scrollbar on the bottom edge
    if grid_w > viewport_w && viewport_w > 1 {
        let bar_row = inner.y + inner.height.saturating_sub(1);
        let bar_len = viewport_w;
        let bar_w = ((bar_len * bar_len) / grid_w).max(1).min(bar_len);
        let max_scroll = grid_w.saturating_sub(viewport_w);
        let bar_x = if max_scroll == 0 { 0 } else { vc * (bar_len - bar_w) / max_scroll };
        for i in 0..bar_len {
            let in_bar = i >= bar_x && i < bar_x + bar_w;
            let (ch, col) = if in_bar {
                ('▬', Color::Rgb(70, 70, 100))
            } else {
                ('─', Color::Rgb(25, 25, 35))
            };
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(col))),
                Rect::new(inner.x + i as u16, bar_row, 1, 1),
            );
        }
    }

    let t_end = Instant::now();
    RenderPhaseUs {
        label_overlay:    t_after_label.duration_since(t_start).as_micros() as u64,
        flood_candidates: t_after_flood.duration_since(t_after_label).as_micros() as u64,
        span_loop:        t_after_loop.duration_since(t_after_flood).as_micros() as u64,
        paragraph_render: t_after_para.duration_since(t_after_loop).as_micros() as u64,
        scrollbars:       t_end.duration_since(t_after_para).as_micros() as u64,
        palette_us:       0,
        footer_us:        0,
    }
}


#[allow(clippy::too_many_arguments)]
fn cell_char_and_style<'a>(
    app: &'a App,
    sim_flat: Option<&SimFlat>,
    row: usize,
    col: usize,
    is_cursor: bool,
    sat_anchor: Option<(usize, usize)>,
    cell_comp: Option<&'a crate::components::Component>,
) -> (char, Style) {
    // ── Satellite cell (part of a composite component) ────────────────────
    if let Some((ar, ac)) = sat_anchor {
        if let Some(comp) = app.canvas.grid.get(ar, ac) {
            let pr = comp.effective_port_row();
            let (fw, fh) = comp.effective_footprint();
            let dr = row.wrapping_add(pr).wrapping_sub(ar);
            let dc = col.wrapping_sub(ac);
            let label = comp.effective_composite_label();
            let base_ch = cell_override_or_default(app, comp, dr, dc, fw, fh, pr, label);
            let ch = composite_animated_char(app, sim_flat, comp, dr, dc, fw, fh, pr, base_ch, ar, ac);
            let style = composite_style(app, sim_flat, ar, ac, comp, dr, dc, fw, fh, is_cursor);
            return (ch, style);
        }
    }

    let Some(comp) = cell_comp else {
        if is_cursor {
            // If an annotation is pending placement, show its bracket regardless of palette.
            if let Some((ann_kind, _)) = &app.text_input.pending_annotation {
                let (anchor_ch, [pr, pg, pb], bg) = match ann_kind {
                    ComponentKind::Label => ('[',  [255u8, 230, 60], Color::Rgb(60, 55, 0)),
                    ComponentKind::Note  => ('*',  [80u8, 220, 230], Color::Rgb(0, 45, 55)),
                    _                    => ('[',  [200u8, 200, 200], Color::Rgb(50, 50, 50)),
                };
                return (anchor_ch, Style::default().bg(bg).fg(Color::Rgb(pr, pg, pb)).add_modifier(Modifier::BOLD));
            }
            // Show the selected component's symbol as a placement preview.
            let kind = app.selected_component_kind();
            let (sym, [pr, pg, pb]) = if kind == ComponentKind::Custom {
                let customs = app.glyph_registry.custom_components();
                let (s, fg) = app.selected_custom_idx()
                    .and_then(|ci| customs.get(ci))
                    .map(|c| (c.glyph.symbol, c.glyph.fg))
                    .unwrap_or(('?', [150, 150, 150]));
                (s, fg)
            } else if kind == ComponentKind::Label {
                ('[', [255, 230, 60])
            } else if kind == ComponentKind::Note {
                ('[', [80, 220, 230])
            } else if kind.supports_color_override() {
                (kind.symbol(), app.selected_build_color())
            } else {
                let g = app.glyph_registry.resolve(kind, app.pal.selected_material, app.pal.selected_diameter);
                (g.symbol, g.fg)
            };
            return (sym, Style::default().bg(Color::Rgb(50, 50, 50)).fg(Color::Rgb(pr, pg, pb)));
        }
        return ('·', Style::default().fg(Color::Rgb(35, 35, 35)));
    };

    // ── Label anchor — shows '['; overlay pre-pass fills text + ']' ────────
    if comp.kind == ComponentKind::Label {
        let style = if is_cursor {
            Style::default().bg(Color::Rgb(60, 55, 0)).fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(255, 230, 60)).add_modifier(Modifier::BOLD)
        };
        return ('[', style);
    }

    // ── Note anchor — shows '*'; overlay pre-pass fills the box ────────────
    if comp.kind == ComponentKind::Note {
        let style = if is_cursor {
            Style::default().bg(Color::Rgb(0, 45, 55)).fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(80, 220, 230)).add_modifier(Modifier::BOLD)
        };
        return ('*', style);
    }

    // ── Link anchor — shows '⇒'; overlay pre-pass fills the box ───────────
    if comp.kind == ComponentKind::Link {
        let style = if is_cursor {
            Style::default().bg(Color::Rgb(45, 25, 0)).fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(255, 185, 55)).add_modifier(Modifier::BOLD)
        };
        return ('⇒', style);
    }

    // ── Composite anchor (W port / left border char at dr=port_row, dc=0) ──
    if comp.effective_is_composite() {
        let (fw, fh) = comp.effective_footprint();
        let pr = comp.effective_port_row();
        let label = comp.effective_composite_label();
        let base_ch = cell_override_or_default(app, comp, pr, 0, fw, fh, pr, label);
        let ch = composite_animated_char(app, sim_flat, comp, pr, 0, fw, fh, pr, base_ch, row, col);
        let style = composite_style(app, sim_flat, row, col, comp, pr, 0, fw, fh, is_cursor);
        return (ch, style);
    }

    let glyph = resolve_glyph_for_comp(app, comp);
    let [mr, mg, mb] = glyph.fg;

    if comp.valve_state == Some(ValveState::Closed) {
        let style = if is_cursor {
            Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red)
        };
        return ('X', style);
    }

    let (ch, fg, bg_opt) = cell_appearance(app, sim_flat, comp, row, col, glyph.symbol, mr, mg, mb);

    let mut style = Style::default().fg(fg);
    if let Some(bg) = bg_opt {
        style = style.bg(bg);
    }
    if matches!(comp.kind, ComponentKind::Source | ComponentKind::Sink) {
        style = style.add_modifier(Modifier::BOLD);
    }

    // Hot/cold tint in build mode (sim colors already encoded in fg/bg above).
    if sim_flat.is_none() && !is_cursor {
        use crate::components::LineTemp;
        style = match comp.line_temp {
            LineTemp::Cold  => style.fg(Color::Rgb(80, 160, 255)),
            LineTemp::Hot   => style.fg(Color::Rgb(255, 90, 70)),
            LineTemp::Recirc => style.fg(Color::Rgb(255, 165, 40)),
            LineTemp::Unset => style,
        };
        // DWV components always render in a distinct warm-brown to stand out from supply
        if comp.kind.is_dwv() {
            style = match comp.kind {
                crate::components::ComponentKind::PTrap    => style.fg(Color::Rgb(230, 200, 60)),
                crate::components::ComponentKind::Vent     => style.fg(Color::Rgb(100, 200, 100)),
                _ => style.fg(Color::Rgb(160, 110, 60)),
            };
        }
    }

    if is_cursor {
        if ch == '█' {
            // '█' fills the entire cell with fg; black fg would make it invisible.
            style = Style::default().bg(Color::Rgb(50, 50, 50)).fg(Color::White).add_modifier(Modifier::BOLD);
        } else {
            style = Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD);
        }
    }

    (ch, style)
}

#[allow(clippy::too_many_arguments)]
fn cell_appearance(
    app: &App,
    sim_flat: Option<&SimFlat>,
    comp: &crate::components::Component,
    row: usize,
    col: usize,
    base_ch: char,
    mr: u8, mg: u8, mb: u8,
) -> (char, Color, Option<Color>) {
    let fluid = app.sim.fluid_type;
    let f_bg  = fluid_bg(fluid);
    let f_fg  = fluid_fg(fluid);

    if let Some(sf) = sim_flat {
        match sf.state(row, col) {
            FlowState::Flowing => {
                let (gpm, velocity) = sf.flow_gpm_vel(row, col);

                match comp.kind {
                    ComponentKind::Source => (base_ch, Color::LightGreen, None),
                    ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet
                    | ComponentKind::BasinSink => (base_ch, Color::LightMagenta, None),
                    ComponentKind::PressureGauge => {
                        // Keep ⊙ visible; gold glow on fluid background signals an active gauge reading
                        (base_ch, Color::Rgb(220, 200, 60), Some(f_bg))
                    }
                    ComponentKind::FlowMeterH | ComponentKind::FlowMeterV => {
                        // Keep ⊗ symbol visible; teal glow on fluid background signals active metering
                        (base_ch, Color::Rgb(60, 200, 180), Some(f_bg))
                    }
                    _ => {
                        // Packet density: tighter spacing at higher flow rates
                        let period: usize = if gpm > 5.0 { 3 } else if gpm > 2.0 { 4 } else { 6 };
                        let slow_tick = (app.tick / 4) as usize;
                        // Packet scroll speed proportional to velocity: every N ticks advance one cell
                        let move_divisor: u64 = if velocity >= 5.0 { 1 }
                            else if velocity >= 3.0 { 2 }
                            else if velocity >= 1.0 { 3 }
                            else { 5 };
                        let move_frame = (app.tick / move_divisor) as usize;
                        let pos = flow_pos(comp.kind, row, col);
                        let flow_dir = sf.flow_dir(row, col);
                        let reversed = match comp.kind {
                            ComponentKind::PipeH | ComponentKind::BallValveH
                            | ComponentKind::CheckValveH | ComponentKind::Reducer => flow_dir.1 < 0,
                            ComponentKind::PipeV | ComponentKind::BallValveV
                            | ComponentKind::CheckValveV => flow_dir.0 < 0,
                            _ => false,
                        };
                        let phase = if reversed {
                            (pos + move_frame % period) % period
                        } else {
                            (pos + period - move_frame % period) % period
                        };
                        let pipe_color = scale_rgb(
                            mr, mg, mb,
                            if gpm > 5.0 { 1.35 } else if gpm > 2.0 { 1.1 } else { 0.85 },
                        );
                        match phase {
                            0 => {
                                let ch = fluid_packet_char(fluid, comp.kind, slow_tick);
                                (ch, f_fg, Some(f_bg))
                            }
                            1 => {
                                let ch = fluid_packet_char(fluid, comp.kind, slow_tick + 1);
                                let (fr, fg2, fb) = fluid.fg_color();
                                let dim = scale_rgb(fr, fg2, fb, 0.45);
                                (ch, dim, Some(f_bg))
                            }
                            _ => (base_ch, pipe_color, Some(f_bg)),
                        }
                    }
                }
            }
            FlowState::Pressurized => {
                let (fr, fg2, fb) = fluid.fg_color();
                let dim = scale_rgb(fr, fg2, fb, 0.3);
                (base_ch, dim, Some(f_bg))
            }
            FlowState::Static => (base_ch, Color::Rgb(70, 70, 70), None),
        }
    } else {
        let color = match comp.kind {
            ComponentKind::Source => Color::LightGreen,
            ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet
            | ComponentKind::BasinSink => Color::LightMagenta,
            ComponentKind::SolidBlock => {
                if let Some([r, g, b]) = comp.color_override {
                    Color::Rgb(r, g, b)
                } else {
                    Color::Rgb(110, 110, 110)
                }
            }
            _ => Color::Rgb(mr, mg, mb),
        };
        (base_ch, color, None)
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_style(
    app: &App,
    sim_flat: Option<&SimFlat>,
    ar: usize,
    ac: usize,
    comp: &crate::components::Component,
    dr: usize,
    dc: usize,
    fw: usize,
    fh: usize,
    is_cursor: bool,
) -> Style {
    let (r, g, b) = cell_rgb(app, comp, dr, dc);
    if is_cursor {
        return Style::default()
            .bg(Color::White)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD);
    }
    if let Some(sf) = sim_flat {
        let state = sf.state(ar, ac);
        match comp.kind {
            ComponentKind::BasinSink => match state {
                FlowState::Pressurized => {
                    let (fr, fg2, fb) = app.sim.fluid_type.fg_color();
                    let bright = scale_rgb(fr, fg2, fb, 1.4);
                    Style::default().fg(bright).bg(fluid_bg(app.sim.fluid_type))
                }
                FlowState::Flowing => {
                    let fg = scale_rgb(r, g, b, 1.3);
                    Style::default().fg(fg).bg(fluid_bg(app.sim.fluid_type))
                }
                _ => Style::default().fg(Color::Rgb(r, g, b)),
            },
            ComponentKind::WaterHeater => match state {
                FlowState::Flowing => {
                    let is_interior = dr > 0 && dr + 1 < fh && dc > 0 && dc + 1 < fw;
                    if is_interior {
                        Style::default().fg(Color::Rgb(220, 120, 40)).bg(Color::Rgb(30, 15, 5))
                    } else {
                        let fg = scale_rgb(r, g, b, 1.3);
                        Style::default().fg(fg).bg(fluid_bg(app.sim.fluid_type))
                    }
                }
                FlowState::Pressurized => {
                    let (fr, fg2, fb) = app.sim.fluid_type.fg_color();
                    let dim = scale_rgb(fr, fg2, fb, 0.3);
                    Style::default().fg(dim).bg(fluid_bg(app.sim.fluid_type))
                }
                _ => Style::default().fg(Color::Rgb(r, g, b)),
            },
            _ => match state {
                FlowState::Flowing => {
                    let fg = scale_rgb(r, g, b, 1.3);
                    Style::default().fg(fg).bg(fluid_bg(app.sim.fluid_type))
                }
                FlowState::Pressurized => {
                    let (fr, fg2, fb) = app.sim.fluid_type.fg_color();
                    let dim = scale_rgb(fr, fg2, fb, 0.3);
                    Style::default().fg(dim).bg(fluid_bg(app.sim.fluid_type))
                }
                _ => Style::default().fg(Color::Rgb(r, g, b)),
            },
        }
    } else {
        Style::default().fg(Color::Rgb(r, g, b))
    }
}

#[allow(clippy::too_many_arguments)]
fn cell_override_or_default(
    app: &App,
    comp: &crate::components::Component,
    dr: usize,
    dc: usize,
    fw: usize,
    fh: usize,
    port_row: usize,
    label: &str,
) -> char {
    if let Some(id) = &comp.custom_id {
        // Custom composite: fw/fh are canvas dims (composite_size), coords are 0-based canvas coords.
        let customs = app.glyph_registry.custom_components();
        if let Some(def) = customs.iter().find(|d| &d.id == id) {
            if let Some(ch) = def.get_cell(dr, dc) {
                return ch;
            }
            // Port cells — face outward toward their pipe.
            if def.get_port_at(dr, dc).is_some() {
                return if dc == 0        { '╣' }  // West edge  → opens left
                    else if dc + 1 == fw { '╠' }  // East edge  → opens right
                    else if dr == 0      { '╩' }  // North edge → opens up
                    else                 { '╦' }; // South edge → opens down
            }
        }
        return composite_box_char(fw, fh, port_row, dr, dc, label, None, true);
    }
    let (ae, aw) = { let (_, _, e, w) = comp.kind.connections(); (e, w) };
    let side_ports = ae || aw;
    let north_inlet_dc = comp.composite_north_inlet_offset()
        .map(|(_dr, dc)| dc as usize);
    composite_box_char(fw, fh, port_row, dr, dc, label, north_inlet_dc, side_ports)
}

/// Handles both static shape characters and animated overlay for specific composite
/// component kinds.  Called after `cell_override_or_default` so it receives the
/// base box-drawing character as `default_ch` and only changes positions it owns.
#[allow(clippy::too_many_arguments)]
fn composite_animated_char(
    app: &App,
    sim_flat: Option<&SimFlat>,
    comp: &crate::components::Component,
    dr: usize,
    dc: usize,
    fw: usize,
    fh: usize,
    port_row: usize,
    default_ch: char,
    anchor_r: usize,
    anchor_c: usize,
) -> char {
    let flow = sim_flat.map(|sf| sf.state(anchor_r, anchor_c));
    let tick = app.tick as usize;

    match comp.kind {
        // ── Toilet ───────────────────────────────────────────────────────────
        // Tank top outline at dr=1; curved bowl at dr=fh-2 (dr=3).
        // Interior of each animates during flush (Flowing).
        ComponentKind::Toilet => {
            // inner box left corner column = 2, right = fw-3
            let lc = 2usize;
            let rc = fw - 3; // = 8 for fw=11
            if dr == 1 {
                if dc == lc { return '┌'; }
                if dc == rc { return '┐'; }
                if dc > lc && dc < rc {
                    if let Some(FlowState::Flowing) = &flow {
                        return ['~', '≈', '·', ' '][(tick / 4 + dc) % 4];
                    }
                    return '─';
                }
            }
            if dr + 2 == fh {
                if dc == lc { return '╰'; }
                if dc == rc { return '╯'; }
                if dc > lc && dc < rc {
                    if let Some(FlowState::Flowing) = &flow {
                        return ['~', '≈', '·', ' '][(tick / 4 + dc + 2) % 4];
                    }
                    return '─';
                }
            }
            default_ch
        }

        // ── Water Heater ─────────────────────────────────────────────────────
        // Inner cylinder at dr=1 and dr=fh-2.  The composite_label already embeds
        // ═│ … │═ in the port row so the cylinder sides appear there automatically.
        // Interior of cylinder shows heating animation when Flowing.
        ComponentKind::WaterHeater => {
            // inner box: left wall dc=2, right wall dc=fw-3 (=12 for fw=15)
            let lc = 2usize;
            let rc = fw - 3;
            if dr == 1 || dr + 2 == fh {
                let is_top = dr == 1;
                if dc == lc { return if is_top { '┌' } else { '└' }; }
                if dc == rc { return if is_top { '┐' } else { '┘' }; }
                if dc > lc && dc < rc {
                    if let Some(FlowState::Flowing) = &flow {
                        let heat = ['·', '∘', '°', '·', ' ', '·'];
                        return heat[(tick / 5 + dr * fw + dc) % heat.len()];
                    }
                    return '─';
                }
            }
            default_ch
        }

        // ── Water Softener ───────────────────────────────────────────────────
        // Two rectangular resin/brine tanks side by side; ◎ symbol in the gap.
        // fw=17: left tank dc=1..5, right tank dc=11..15, center symbol dc=8.
        ComponentKind::WaterSoftener => {
            if dr == 1 || dr + 2 == fh {
                let is_top = dr == 1;
                // Left tank
                if dc == 1  { return if is_top { '┌' } else { '└' }; }
                if dc == 5  { return if is_top { '┐' } else { '┘' }; }
                if (2..=4).contains(&dc) {
                    if let Some(FlowState::Flowing) = &flow {
                        return ['~', '─', '·', '─'][(tick / 4 + dc) % 4];
                    }
                    return '─';
                }
                // Center resin exchange indicator
                if dc == fw / 2 {
                    if let Some(FlowState::Flowing) = &flow {
                        return ['◎', '○', '◉', '●'][(tick / 4) % 4];
                    }
                    return '◎';
                }
                // Right tank
                if dc == 11 { return if is_top { '┌' } else { '└' }; }
                if dc == 15 { return if is_top { '┐' } else { '┘' }; }
                if (12..=14).contains(&dc) {
                    if let Some(FlowState::Flowing) = &flow {
                        return ['~', '─', '·', '─'][(tick / 4 + dc) % 4];
                    }
                    return '─';
                }
            }
            default_ch
        }

        // ── Basin Sink ────────────────────────────────────────────────────────
        // Inner basin walls at dc=1 and dc=fw-2 throughout non-border rows.
        // Bottom of basin has a drain junction (┴) at center (fw/2).
        // One row below the box bottom: drain port indicator (╨) always shown.
        // Overflow animation (Pressurized) covers the entire top area.
        ComponentKind::BasinSink => {
            // Drain port indicator below the box — always shown
            if dr + 1 == fh && dc == fw / 2 { return '╨'; }

            match &flow {
                Some(FlowState::Pressurized) => {
                    // OVERFLOW: water spills over the top; overrides inner shapes
                    let water: [char; 4] = ['≋', '≈', '~', '≈'];
                    if dr == 0 && dc > 0 && dc + 1 < fw {
                        return water[(tick / 2 + dc) % 4];
                    }
                    if dc == 0 && dr > 0 && dr < port_row {
                        return water[(tick / 3 + dr) % 4];
                    }
                    if dc + 1 == fw && dr > 0 && dr < port_row {
                        return water[(tick / 3 + dr + 2) % 4];
                    }
                    if dr > 0 && dr < port_row && dc > 0 && dc + 1 < fw {
                        return water[(tick / 4 + dr + dc) % 4];
                    }
                    default_ch
                }
                _ => {
                    // STATIC BASIN SHAPE + optional fill animation
                    let lc = 1usize;        // left inner wall (same col as label │)
                    let rc = fw - 2;        // right inner wall (=11 for fw=13)
                    let drain_col = fw / 2; // drain channel col (=6 for fw=13)

                    // Inner basin top outline
                    if dr == 1 {
                        if dc == lc { return '┌'; }
                        if dc == rc { return '┐'; }
                        if dc > lc && dc < rc {
                            if let Some(FlowState::Flowing) = &flow {
                                return ['·', '≈', '·', '~'][(tick / 5 + dc) % 4];
                            }
                            return '─';
                        }
                    }
                    // Inner basin bottom outline with drain junction
                    if dr + 2 == fh {
                        if dc == lc { return '└'; }
                        if dc == rc { return '┘'; }
                        if dc == drain_col {
                            if let Some(FlowState::Flowing) = &flow {
                                return ['┴', '↓', '┴', '╨'][(tick / 6) % 4];
                            }
                            return '┴';
                        }
                        if dc > lc && dc < rc {
                            if let Some(FlowState::Flowing) = &flow {
                                return ['·', '≈', '·', '~'][(tick / 5 + dc + 2) % 4];
                            }
                            return '─';
                        }
                    }
                    default_ch
                }
            }
        }

        _ => default_ch,
    }
}

fn resolve_glyph_for_comp(app: &App, comp: &crate::components::Component) -> crate::glyphs::GlyphDef {
    if comp.kind == ComponentKind::Custom {
        if let Some(id) = &comp.custom_id {
            if let Some(def) = app.glyph_registry.custom_components().iter().find(|d| &d.id == id) {
                return def.glyph.clone();
            }
        }
    }
    app.glyph_registry.resolve(comp.kind, comp.material, comp.diameter)
}

fn cell_rgb(app: &App, comp: &crate::components::Component, dr: usize, dc: usize) -> (u8, u8, u8) {
    use crate::glyphs::PortKind;
    if comp.kind == ComponentKind::Custom {
        if let Some(id) = &comp.custom_id {
            if let Some(def) = app.glyph_registry.custom_components().iter().find(|d| &d.id == id) {
                if let Some([r, g, b]) = def.get_cell_color(dr, dc) {
                    return (r, g, b);
                }
                if let Some(port) = def.get_port_at(dr, dc) {
                    return match port.kind {
                        PortKind::Inlet  => (60, 200, 100),
                        PortKind::Outlet => (80, 160, 255),
                        PortKind::Drain  => (220, 130, 40),
                    };
                }
                let [r, g, b] = def.glyph.fg;
                return (r, g, b);
            }
        }
    }
    GlyphRegistry::material_color(comp.material)
}

fn flow_pos(kind: ComponentKind, row: usize, col: usize) -> usize {
    match kind {
        ComponentKind::PipeH
        | ComponentKind::BallValveH
        | ComponentKind::CheckValveH
        | ComponentKind::Reducer => col,
        ComponentKind::PipeV
        | ComponentKind::BallValveV
        | ComponentKind::CheckValveV => row,
        _ => row.wrapping_add(col),
    }
}

/// Returns the ghost preview character and style for a composite footprint cell,
/// or None if the cell is outside the footprint of the currently selected composite.
/// Only fires for empty, non-satellite cells while the app is in Build mode.
fn composite_ghost_cell(app: &App, row: usize, col: usize) -> Option<(char, Style)> {
    use super::composite_box_char;

    let kind = app.selected_component_kind();
    let (cursor_r, cursor_c) = app.canvas.cursor;

    // ── Custom composite ──────────────────────────────────────────────────────
    if kind == ComponentKind::Custom {
        let customs = app.glyph_registry.custom_components();
        let ci = app.selected_custom_idx()?;
        let def = customs.get(ci)?;
        let (fw, fh) = def.composite_size?;  // canvas dims directly (v2.0, no implicit buffer)
        let port_row = fh / 2;
        let dr_i = row as isize - cursor_r as isize + port_row as isize;
        let dc_i = col as isize - cursor_c as isize;
        if dr_i < 0 || dc_i < 0 { return None; }
        let (dr, dc) = (dr_i as usize, dc_i as usize);
        if dr >= fh || dc >= fw { return None; }
        let label = def.label.as_str();
        let ch = if let Some(ch) = def.get_cell(dr, dc) {
            ch
        } else if def.get_port_at(dr, dc).is_some() {
            if dc == 0 { '╣' } else if dc + 1 == fw { '╠' } else if dr == 0 { '╩' } else { '╦' }
        } else {
            composite_box_char(fw, fh, port_row, dr, dc, label, None, true)
        };
        return Some((ch, ghost_style(dr == port_row && dc == 0)));
    }

    // ── Built-in composite ────────────────────────────────────────────────────
    if !kind.is_composite() { return None; }

    let (fw, fh) = kind.footprint();
    let port_row = kind.port_row();
    let dr_i = row as isize - cursor_r as isize + port_row as isize;
    let dc_i = col as isize - cursor_c as isize;
    if dr_i < 0 || dc_i < 0 { return None; }
    let (dr, dc) = (dr_i as usize, dc_i as usize);
    if dr >= fh || dc >= fw { return None; }

    let label = kind.composite_label();
    let (_, _, ae, aw) = kind.connections();
    let ch = composite_box_char(
        fw, fh, port_row, dr, dc, label,
        kind.composite_north_inlet_dc(fw),
        ae || aw,
    );
    Some((ch, ghost_style(dr == port_row && dc == 0)))
}

fn ghost_style(is_anchor: bool) -> Style {
    if is_anchor {
        Style::default().bg(Color::Rgb(40, 45, 65)).fg(Color::Rgb(160, 170, 200))
    } else {
        Style::default().bg(Color::Rgb(18, 22, 32)).fg(Color::Rgb(65, 75, 100))
    }
}

fn fluid_packet_char(fluid: FluidType, kind: ComponentKind, tick: usize) -> char {
    let chars = match kind {
        ComponentKind::PipeH
        | ComponentKind::BallValveH
        | ComponentKind::CheckValveH
        | ComponentKind::Reducer => fluid.h_chars(),
        ComponentKind::PipeV
        | ComponentKind::BallValveV
        | ComponentKind::CheckValveV => fluid.v_chars(),
        _ => fluid.fit_chars(),
    };
    chars[tick % chars.len()]
}
