use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::{centered_rect, key};

pub(super) fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(70, 80, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Settings  [Q]/[C] close ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let h1    = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let body  = Style::default().fg(Color::White);
    let dim   = Style::default().fg(Color::Rgb(110, 110, 110));
    let hilite = Style::default().fg(Color::Black).bg(Color::Cyan);

    let mut lines: Vec<Line> = Vec::new();

    // ── Glyph Files section ───────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("Glyph Library Files", h1)));
    lines.push(Line::from(Span::styled(
        "Auto-loaded at startup in order (later files override earlier ones).",
        dim,
    )));
    lines.push(Line::from(""));

    if app.config.glyph_files.is_empty() {
        lines.push(Line::from(Span::styled("  (no glyph files configured)", dim)));
    } else {
        for (i, path) in app.config.glyph_files.iter().enumerate() {
            let label = format!(
                " {:>2}. {}",
                i + 1,
                path.display()
            );
            let style = if i == app.dialog.settings_idx { hilite } else { body };
            lines.push(Line::from(Span::styled(label, style)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("────────────────────────────────", dim)));
    lines.push(Line::from(""));

    // ── Status line ───────────────────────────────────────────────────────────
    if !app.dialog.settings_status.is_empty() {
        let (msg, col) = if app.dialog.settings_status.starts_with("OK") {
            (app.dialog.settings_status.as_str(), Color::LightGreen)
        } else {
            (app.dialog.settings_status.as_str(), Color::Yellow)
        };
        lines.push(Line::from(Span::styled(msg, Style::default().fg(col))));
        lines.push(Line::from(""));
    }

    // ── Grid Scale section ────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("Grid Scale", h1)));
    lines.push(Line::from(Span::styled(
        "Sets what physical distance each canvas cell represents.",
        dim,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Current scale: ", dim),
        Span::styled(app.config.grid_scale_label(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        key("[G]"),
        Span::raw(" cycle"),
    ]));
    lines.push(Line::from(Span::styled(
        "  New pipes placed will default to this cell length.",
        dim,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("────────────────────────────────", dim)));
    lines.push(Line::from(""));

    // ── Config file path ──────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("Config file: ", dim),
        Span::styled("flow-dynamics.config.json", body),
    ]));
    lines.push(Line::from(""));

    // ── Key hints ─────────────────────────────────────────────────────────────
    lines.push(Line::from(vec![
        key("[A]"), Span::raw("Add file  "),
        key("[D/Del]"), Span::raw("Remove  "),
        key("[↑↓]"), Span::raw("Select  "),
        key("[L]"), Span::raw("Load now  "),
        key("[G]"), Span::raw("Scale  "),
        key("[Q/C]"), Span::styled("Close", Style::default().fg(Color::Red)),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}
