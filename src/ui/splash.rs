use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Paragraph},
    Frame,
};

use crate::app::App;
use crate::components::ComponentKind;
use crate::grid::Grid;
use crate::simulation::FlowState;

// ── Public entry-point ────────────────────────────────────────────────────────
//
// The outer pipe loop is a closed circuit — every edge and corner participates
// in the same flow animation; there are no special Source/Drain labels on it.
//
// Inner content:
//   • splash.json found → layout rendered centered with a diagonal wave effect
//   • splash.json absent → brief instruction text

pub(super) fn render_splash(f: &mut Frame, app: &App) {
    let area = f.area();

    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(4, 6, 18))),
        area,
    );

    let bw = (area.width.saturating_sub(8) as usize).clamp(50, 72);
    let bh = 16usize;
    let bx = area.x as usize + (area.width as usize).saturating_sub(bw) / 2;
    let by = area.y as usize + (area.height as usize).saturating_sub(bh) / 2;

    if by + bh > area.height as usize || bx + bw > area.width as usize {
        f.render_widget(
            Paragraph::new("FLOW DYNAMICS — Press any key")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            area,
        );
        return;
    }

    // ── Animated closed pipe loop ─────────────────────────────────────────────
    let mut path: Vec<(usize, usize, bool)> = Vec::new();
    for c in 0..bw          { path.push((by,        bx + c,      true));  } // top →
    for r in 1..bh           { path.push((by + r,    bx + bw - 1, false)); } // right ↓
    for c in (0..bw-1).rev() { path.push((by + bh-1, bx + c,      true));  } // bottom ←
    for r in (1..bh-1).rev() { path.push((by + r,    bx,          false)); } // left ↑

    let n = path.len();

    let tick    = app.tick as usize;
    let shimmer = (tick / 3) % 3;

    let pipe_col = Color::Rgb(55, 110, 175);
    let head_col = Color::Rgb(80, 185, 255);
    let tail_col = Color::Rgb(32,  74, 102);
    let flow_bg  = Color::Rgb(0,   25,  50);

    let packet_gap = (n / 6).max(10);
    let h_chars: [char; 3] = ['≈', '≋', '∿'];
    let v_chars: [char; 3] = ['⋮', '⁞', '⋱'];

    for (pos_idx, &(row, col, is_horiz)) in path.iter().enumerate() {
        let base  = path_char(pos_idx, bw, bh);
        let phase = (pos_idx + n - tick % n) % n % packet_gap;

        let (ch, style) = if phase == 0 {
            let c = if is_horiz { h_chars[shimmer] } else { v_chars[shimmer] };
            (c, Style::default().fg(head_col).bg(flow_bg).add_modifier(Modifier::BOLD))
        } else if phase == 1 {
            (base, Style::default().fg(tail_col).bg(flow_bg))
        } else {
            (base, Style::default().fg(pipe_col))
        };

        f.render_widget(
            Paragraph::new(ch.to_string()).style(style),
            Rect::new(col as u16, row as u16, 1, 1),
        );
    }

    // ── Inner content ─────────────────────────────────────────────────────────
    // Reserve the last 2 inner rows for the "Press any key" prompt + padding.
    let inner_x = bx + 1;
    let inner_y = by + 1;
    let inner_w = bw.saturating_sub(2);
    let logo_h  = bh.saturating_sub(4); // rows available for the logo (12)

    if let Some(grid) = &app.sim.splash_grid {
        render_splash_grid(f, app, grid, inner_x, inner_y, inner_w, logo_h);
    } else {
        render_fallback(f, inner_x, inner_y, inner_w, logo_h, tick);
    }

    // ── "Press any key" prompt ────────────────────────────────────────────────
    let pulse = if (tick / 4).is_multiple_of(2) { 255u8 } else { 155u8 };
    f.render_widget(
        Paragraph::new(Span::styled(
            "Press any key to begin",
            Style::default()
                .fg(Color::Rgb(pulse, pulse, 0))
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Rect::new(
            (bx + 2) as u16,
            (by + bh - 2) as u16,
            bw.saturating_sub(4) as u16,
            1,
        ),
    );
}

// ── Splash grid renderer ──────────────────────────────────────────────────────

fn render_splash_grid(
    f: &mut Frame,
    app: &App,
    grid: &Grid,
    area_x: usize,
    area_y: usize,
    area_w: usize,
    area_h: usize,
) {
    // Find bounding box of all occupied cells (anchors + composite satellites).
    let (mut min_r, mut min_c) = (usize::MAX, usize::MAX);
    let (mut max_r, mut max_c) = (0usize, 0usize);
    let mut any = false;

    for r in 0..grid.height {
        for c in 0..grid.width {
            if grid.get(r, c).is_some() || grid.satellite_anchor(r, c).is_some() {
                min_r = min_r.min(r); max_r = max_r.max(r);
                min_c = min_c.min(c); max_c = max_c.max(c);
                any = true;
            }
        }
    }

    if !any { return; }

    let logo_h = max_r - min_r + 1;
    let logo_w = max_c - min_c + 1;

    // Center the logo inside the available area.
    let off_r = area_y + area_h.saturating_sub(logo_h) / 2;
    let off_c = area_x + area_w.saturating_sub(logo_w) / 2;

    let wave_period = 20usize;
    let wave_offset = (app.tick as usize / 3) % wave_period;

    let shimmer = (app.tick as usize / 2) % 3;
    let h_chars: [char; 3] = ['≈', '≋', '∿'];
    let v_chars: [char; 3] = ['⋮', '⁞', '⋱'];
    let flow_bg  = Color::Rgb(0, 25, 50);
    let flow_fg  = Color::Rgb(80, 185, 255);
    let flow_dim = Color::Rgb(32, 74, 102);

    for r in min_r..=max_r {
        for c in min_c..=max_c {
            let sr = off_r + (r - min_r);
            let sc = off_c + (c - min_c);
            if sr >= area_y + area_h || sc >= area_x + area_w {
                continue;
            }

            // Check if this cell is Flowing in the simulation result.
            let flow_state = app.sim.splash_sim.as_ref()
                .and_then(|sim| sim.cell_states.get(&(r, c)).cloned());

            let (ch, style) = if flow_state == Some(FlowState::Flowing) {
                // Animated water: the packet travels left-to-right along the flow line.
                let packet_len = 6usize;
                let period = (max_c - min_c + 1).max(1);
                let phase = (c + period - (app.tick as usize / 2) % period) % period % packet_len;
                let comp = grid.get(r, c);
                let is_horiz = comp.map(|cp| {
                    use ComponentKind::*;
                    matches!(cp.kind, PipeH | ElbowNE | ElbowNW | ElbowSE | ElbowSW
                        | TeeNEW | TeeSEW | Cross)
                }).unwrap_or(true);
                let water_char = if is_horiz { h_chars[shimmer] } else { v_chars[shimmer] };
                let (fg, bold) = match phase {
                    0 => (flow_fg,  true),
                    1 => (flow_dim, false),
                    _ => (flow_fg,  false),
                };
                let s = if bold {
                    Style::default().fg(fg).bg(flow_bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg).bg(flow_bg)
                };
                (water_char, s)
            } else {
                // Diagonal wave for non-flowing cells (letters).
                let wave_pos = ((r - min_r) + (c - min_c) + wave_period - wave_offset) % wave_period;
                let (ch, style) = resolve_cell(app, grid, r, c);
                if ch == ' ' || ch == '\0' {
                    continue;
                }
                (ch, apply_wave(style, wave_pos))
            };

            f.render_widget(
                Paragraph::new(ch.to_string()).style(style),
                Rect::new(sc as u16, sr as u16, 1, 1),
            );
        }
    }
}

/// Returns the display character and base style for a grid cell, without
/// any animation applied.  Returns ('\0', _) for cells that should be skipped.
fn resolve_cell(app: &App, grid: &Grid, r: usize, c: usize) -> (char, Style) {
    // ── Satellite cell (part of a composite component) ────────────────────────
    if let Some((ar, ac)) = grid.satellite_anchor(r, c) {
        let Some(comp) = grid.get(ar, ac) else {
            return ('\0', Style::default());
        };
        let pr      = comp.effective_port_row();
        let (fw, fh) = comp.effective_footprint();
        let dr      = r.wrapping_add(pr).wrapping_sub(ar);
        let dc      = c.wrapping_sub(ac);
        let label   = comp.effective_composite_label();
        let (_, _, ae, aw) = comp.kind.connections();
        let side_ports = ae || aw;
        let north_dc = comp.composite_north_inlet_offset().map(|(_, dc)| dc as usize);
        let ch      = super::composite_box_char(fw, fh, pr, dr, dc, label, north_dc, side_ports);
        let [red, g, b] = app.glyph_registry
            .resolve(comp.kind, comp.material, comp.diameter)
            .fg;
        return (ch, Style::default().fg(Color::Rgb(red, g, b)));
    }

    let Some(comp) = grid.get(r, c) else {
        return ('\0', Style::default());
    };

    // ── Composite anchor ──────────────────────────────────────────────────────
    if comp.effective_is_composite() {
        let (fw, fh) = comp.effective_footprint();
        let pr      = comp.effective_port_row();
        let label   = comp.effective_composite_label();
        let (_, _, ae, aw) = comp.kind.connections();
        let side_ports = ae || aw;
        let north_dc = comp.composite_north_inlet_offset().map(|(_, dc)| dc as usize);
        let ch      = super::composite_box_char(fw, fh, pr, pr, 0, label, north_dc, side_ports);
        let [red, g, b] = app.glyph_registry
            .resolve(comp.kind, comp.material, comp.diameter)
            .fg;
        return (ch, Style::default().fg(Color::Rgb(red, g, b)));
    }

    // ── Single-cell component ─────────────────────────────────────────────────
    let glyph = if comp.kind == ComponentKind::Custom {
        match &comp.custom_id {
            Some(id) => app.glyph_registry
                .custom_components()
                .iter()
                .find(|d| &d.id == id)
                .map(|d| d.glyph.clone())
                .unwrap_or_else(|| app.glyph_registry.resolve(comp.kind, comp.material, comp.diameter)),
            None => app.glyph_registry.resolve(comp.kind, comp.material, comp.diameter),
        }
    } else {
        app.glyph_registry.resolve(comp.kind, comp.material, comp.diameter)
    };

    let [red, g, b] = glyph.fg;
    let (fg, bold) = match comp.kind {
        ComponentKind::Source => (Color::LightGreen,   true),
        ComponentKind::Sink   => (Color::LightMagenta, true),
        _                     => (Color::Rgb(red, g, b), false),
    };
    let style = if bold {
        Style::default().fg(fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(fg)
    };
    (glyph.symbol, style)
}

/// Scale foreground RGB brightness by a wave factor.
/// Non-Rgb colors (e.g. LightGreen/LightMagenta for Source/Sink) are untouched.
fn apply_wave(style: Style, wave_pos: usize) -> Style {
    let (r, g, b) = match style.fg {
        Some(Color::Rgb(r, g, b)) => (r, g, b),
        _ => return style,
    };
    let scale = match wave_pos {
        0     => 1.6_f32,
        1..=2 => 1.3,
        3..=5 => 1.0,
        _     => 0.65,
    };
    let s = |v: u8| (v as f32 * scale).clamp(0.0, 255.0) as u8;
    let bold = wave_pos <= 2 || style.add_modifier.contains(Modifier::BOLD);
    let base = Style::default().fg(Color::Rgb(s(r), s(g), s(b)));
    if bold { base.add_modifier(Modifier::BOLD) } else { base }
}

// ── Fallback when no splash.json is present ───────────────────────────────────

fn render_fallback(
    f: &mut Frame,
    inner_x: usize,
    inner_y: usize,
    inner_w: usize,
    inner_h: usize,
    tick: usize,
) {
    let mid_y = inner_y + inner_h / 2;

    let title_col = if (tick / 6).is_multiple_of(2) { Color::Cyan } else { Color::Rgb(80, 185, 255) };
    f.render_widget(
        Paragraph::new(Span::styled(
            "FLOW DYNAMICS",
            Style::default().fg(title_col).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Rect::new(inner_x as u16, mid_y.saturating_sub(1) as u16, inner_w as u16, 1),
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "Build your logo in-app, then save it as  splash.json",
            Style::default().fg(Color::Rgb(60, 80, 110)),
        ))
        .alignment(Alignment::Center),
        Rect::new(inner_x as u16, (mid_y + 1) as u16, inner_w as u16, 1),
    );
}

// ── Border helpers ────────────────────────────────────────────────────────────

fn path_char(pos: usize, bw: usize, bh: usize) -> char {
    if pos == 0                    { '╔' }
    else if pos < bw - 1           { '═' }
    else if pos == bw - 1          { '╗' }
    else if pos < bw + bh - 2      { '║' }
    else if pos == bw + bh - 2     { '╝' }
    else if pos < 2 * bw + bh - 3  { '═' }
    else if pos == 2 * bw + bh - 3 { '╚' }
    else                            { '║' }
}
