use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::centered_rect_abs;

pub(super) fn render_export_dialog(f: &mut Frame, area: Rect) {
    let popup = centered_rect_abs(42, 9, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Export Canvas ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let key  = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let body = Style::default().fg(Color::White);
    let dim  = Style::default().fg(Color::Rgb(110, 110, 110));

    let lines = vec![
        Line::from(vec![]),
        Line::from(vec![
            Span::styled("  [T]", key),
            Span::styled("  Plain text  (.txt)", body),
        ]),
        Line::from(vec![
            Span::styled("  [J]", key),
            Span::styled("  JSON layout (.json)", body),
        ]),
        Line::from(vec![]),
        Line::from(vec![
            Span::styled("  Esc", dim),
            Span::styled(" — cancel", dim),
        ]),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}
