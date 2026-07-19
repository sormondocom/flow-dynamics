use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

pub(super) fn render_help(f: &mut Frame, app: &App) {
    let area = f.area();

    // Dark full-screen backdrop
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(4, 6, 18))),
        area,
    );

    let w = area.width.min(84);
    let h = area.height.saturating_sub(2).max(8);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let overlay = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, overlay);

    let block = Block::default()
        .title(Span::styled(
            " FLOW DYNAMICS — Help  (↑↓ / j/k  PgUp/PgDn  H/Esc to close) ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(55, 110, 175)))
        .style(Style::default().bg(Color::Rgb(4, 6, 18)));

    let inner = block.inner(overlay);
    f.render_widget(block, overlay);

    // Reserve bottom row for search bar when search is active.
    let search_bar_h: u16 = if app.help_search_active || !app.help_search.is_empty() { 1 } else { 0 };
    let visible_h = (inner.height.saturating_sub(search_bar_h)) as usize;
    let content_w = inner.width.saturating_sub(2) as usize; // -1 for scrollbar, -1 for padding
    let total = app.help_lines.len();
    let max_scroll = total.saturating_sub(visible_h);
    let scroll = app.help_scroll.min(max_scroll);

    let search_query_lc = if !app.help_search.is_empty() {
        Some(app.help_search.to_lowercase())
    } else {
        None
    };

    // Content lines
    for (i, raw) in app.help_lines.iter().skip(scroll).take(visible_h).enumerate() {
        let row = inner.y + i as u16;
        let line_matches = search_query_lc.as_ref()
            .map(|q| raw.to_lowercase().contains(q.as_str()))
            .unwrap_or(false);
        let paragraph = if line_matches {
            Paragraph::new(render_line(raw, content_w))
                .style(Style::default().bg(Color::Rgb(20, 50, 20)))
        } else {
            Paragraph::new(render_line(raw, content_w))
        };
        f.render_widget(
            paragraph,
            Rect::new(inner.x, row, inner.width.saturating_sub(1), 1),
        );
    }

    // Scrollbar (rightmost column of inner area)
    if total > visible_h {
        let bar_h = ((visible_h * visible_h) / total).max(1);
        let bar_y = if max_scroll == 0 { 0 } else {
            scroll * (visible_h - bar_h) / max_scroll
        };
        for i in 0..visible_h {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, style) = if in_bar {
                ('█', Style::default().fg(Color::Rgb(55, 110, 175)))
            } else {
                ('░', Style::default().fg(Color::Rgb(25, 35, 55)))
            };
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), style)),
                Rect::new(inner.x + inner.width - 1, inner.y + i as u16, 1, 1),
            );
        }
    }

    // Search bar at bottom of inner area
    if search_bar_h > 0 {
        let bar_y = inner.y + inner.height.saturating_sub(1);
        let query = &app.help_search;
        let available_w = inner.width as usize;
        let prefix = "/ ";
        let cursor_block = if app.help_search_active { "█" } else { "" };
        let hint = if app.help_search_active {
            "  [↑↓] jump matches  [Esc] clear"
        } else {
            "  [/] search"
        };
        let max_q_w = available_w
            .saturating_sub(prefix.len() + cursor_block.len() + hint.len());
        let display_q: String = query.chars().rev().take(max_q_w).collect::<String>().chars().rev().collect();
        let search_bar = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(display_q, Style::default().fg(Color::White).bg(Color::Rgb(20, 28, 50))),
            Span::styled(cursor_block, Style::default().fg(Color::Yellow).bg(Color::Rgb(20, 28, 50))),
            Span::styled(hint, Style::default().fg(Color::Rgb(80, 100, 130))),
        ]);
        f.render_widget(
            Paragraph::new(search_bar).style(Style::default().bg(Color::Rgb(8, 12, 28))),
            Rect::new(inner.x, bar_y, inner.width, 1),
        );
    }
}

/// Parse one line of help.txt markup into a styled ratatui Line.
///
/// Markup rules:
///   `# Title`    → full-width cyan header bar
///   `## Title`   → yellow bold sub-heading
///   `---`        → horizontal rule (─ repeated)
///   `[KEY]`      → yellow bold key badge anywhere in a line
///   `` `text` `` → light-cyan inline highlight
///   (empty)      → blank spacer
fn render_line(raw: &str, content_w: usize) -> Line<'static> {
    // Section header
    if let Some(title) = raw.strip_prefix("# ") {
        let padded = format!(" {:<width$}", title, width = content_w.saturating_sub(1));
        return Line::from(vec![Span::styled(
            padded,
            Style::default()
                .fg(Color::Rgb(240, 252, 255))
                .bg(Color::Rgb(0, 100, 140))
                .add_modifier(Modifier::BOLD),
        )]);
    }

    // Sub-heading
    if let Some(title) = raw.strip_prefix("## ") {
        return Line::from(vec![Span::styled(
            format!("  {title}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    // Horizontal rule
    if raw.trim() == "---" {
        return Line::from(vec![Span::styled(
            "─".repeat(content_w),
            Style::default().fg(Color::Rgb(40, 65, 95)),
        )]);
    }

    // Empty line
    if raw.is_empty() {
        return Line::from("");
    }

    // Regular line with inline [KEY] and `code` spans
    parse_inline(raw)
}

/// Split a line into spans, highlighting `[KEY]` in yellow-bold and `` `code` `` in cyan.
fn parse_inline(raw: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut rest = raw;

    while !rest.is_empty() {
        // Find the next markup trigger: [ or `
        let next_bracket  = rest.find('[');
        let next_backtick = rest.find('`');

        let trigger = match (next_bracket, next_backtick) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None)    => Some(a),
            (None, Some(b))    => Some(b),
            (None, None)       => None,
        };

        let pos = match trigger {
            None => {
                spans.push(plain(rest));
                break;
            }
            Some(p) => p,
        };

        // Plain text before this trigger
        if pos > 0 {
            spans.push(plain(&rest[..pos]));
        }

        if rest[pos..].starts_with('[') {
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find(']') {
                let key = &rest[..end];
                spans.push(Span::styled(
                    format!("[{key}]"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                rest = &rest[end + 1..];
            } else {
                spans.push(plain("["));
            }
        } else if rest[pos..].starts_with('`') {
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find('`') {
                let code = &rest[..end];
                spans.push(Span::styled(
                    code.to_string(),
                    Style::default().fg(Color::Rgb(100, 220, 220)),
                ));
                rest = &rest[end + 1..];
            } else {
                spans.push(plain("`"));
            }
        }
    }

    if spans.is_empty() {
        spans.push(plain(""));
    }

    Line::from(spans)
}

fn plain(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(Color::Rgb(185, 200, 215)))
}
