mod annotation_dialog;
mod annotations;
mod canvas;
mod export_dialog;
mod file_dialog;
mod footer;
mod glyph_editor;
mod help;
mod overlays;
mod palette;
mod settings;
mod splash;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
    Frame,
};

use crate::app::{App, AppMode};
use crate::fluid::FluidType;

// ── Top-level router ──────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &App) {
    if app.mode == AppMode::Splash {
        splash::render_splash(f, app);
        return;
    }

    if app.mode == AppMode::Help {
        help::render_help(f, app);
        return;
    }

    if app.mode == AppMode::FileDialog {
        file_dialog::render_file_dialog(f, app);
        return;
    }

    if app.mode == AppMode::ConfirmNew {
        file_dialog::render_confirm_new(f, app);
        return;
    }

    if app.mode == AppMode::ConfirmQuit {
        file_dialog::render_confirm_quit(f, app);
        return;
    }

    if app.mode == AppMode::GlyphEditor {
        glyph_editor::render_glyph_editor(f, app);
        return;
    }

    let area = f.area();

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(9)])
        .split(area);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(vchunks[0]);

    canvas::render_canvas(f, app, hchunks[0]);
    palette::render_palette(f, app, hchunks[1]);
    footer::render_footer(f, app, vchunks[1]);

    if app.mode == AppMode::BomView {
        overlays::render_bom(f, app, area);
    }

    if app.mode == AppMode::AssemblyBrowser {
        overlays::render_assembly_browser(f, app, area);
    }

    if app.mode == AppMode::ComponentDetail {
        overlays::render_component_detail(f, app, area);
    }

    if app.mode == AppMode::Settings {
        settings::render_settings(f, app, area);
    }

    if app.mode == AppMode::ExportDialog {
        export_dialog::render_export_dialog(f, area);
    }

    if app.mode == AppMode::AnnotationDialog {
        annotation_dialog::render_annotation_dialog(f, app, area);
    }

}

// ── Shared primitives (used by multiple submodules) ───────────────────────────

pub(crate) fn fluid_bg(fluid: FluidType) -> Color {
    let (r, g, b) = fluid.bg_color();
    Color::Rgb(r, g, b)
}

pub(crate) fn fluid_fg(fluid: FluidType) -> Color {
    let (r, g, b) = fluid.fg_color();
    Color::Rgb(r, g, b)
}

pub(crate) fn scale_rgb(r: u8, g: u8, b: u8, factor: f32) -> Color {
    let s = |v: u8| (v as f32 * factor).clamp(0.0, 255.0) as u8;
    Color::Rgb(s(r), s(g), s(b))
}

pub(crate) fn key(label: &str) -> Span<'static> {
    Span::styled(
        label.to_string(),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn panel_block(title: &str, focused: bool) -> Block<'static> {
    use ratatui::widgets::BorderType;
    let style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Rgb(80, 80, 80))
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Thick } else { BorderType::Plain })
        .border_style(style)
        .title(format!(" {title} "))
}

pub(crate) fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let my = (100 - pct_y) / 2;
    let mx = (100 - pct_x) / 2;
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(my),
            Constraint::Percentage(pct_y),
            Constraint::Percentage(my),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(mx),
            Constraint::Percentage(pct_x),
            Constraint::Percentage(mx),
        ])
        .split(vert[1])[1]
}

pub(crate) fn centered_rect_abs(w: u16, h: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(w) / 2;
    let y = r.y + r.height.saturating_sub(h) / 2;
    Rect { x, y, width: w.min(r.width), height: h.min(r.height) }
}

pub(crate) fn format_pipe_length(ft: f32) -> String {
    let whole = ft.floor() as u32;
    let inches = ((ft - whole as f32) * 12.0).round() as u32;
    match (whole, inches) {
        (0, i) => format!("{}\"", i),
        (f, 0) => format!("{}'", f),
        (f, i) => format!("{}'{}\"", f, i),
    }
}

/// Character to draw at position (dr, dc) within the inner box of a composite component.
/// `fw`/`fh` are the box dimensions; `port_row` is the row that carries the E/W ports.
pub(crate) fn composite_box_char(
    fw: usize, fh: usize, port_row: usize, dr: usize, dc: usize, label: &str,
    north_inlet_dc: Option<usize>,
    side_ports: bool,
) -> char {
    let is_top    = dr == 0;
    let is_bottom = dr + 1 == fh;
    let is_port   = dr == port_row;

    if is_top {
        if north_inlet_dc == Some(dc) { return '╦'; }
        match dc { 0 => '╔', c if c == fw - 1 => '╗', _ => '═' }
    } else if is_bottom {
        match dc { 0 => '╚', c if c == fw - 1 => '╝', _ => '═' }
    } else if is_port {
        if side_ports {
            match dc { 0 => '╠', c if c == fw - 1 => '╣', c => label.chars().nth(c - 1).unwrap_or(' ') }
        } else {
            if dc == 0 || dc + 1 == fw { '║' } else { label.chars().nth(dc - 1).unwrap_or(' ') }
        }
    } else {
        if dc == 0 || dc + 1 == fw { '║' } else { ' ' }
    }
}
