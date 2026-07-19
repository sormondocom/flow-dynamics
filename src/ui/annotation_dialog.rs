use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, InputMode, TextEditTarget};

use super::centered_rect_abs;

const VIS_ROWS: usize = 3;

pub(super) fn render_annotation_dialog(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if matches!(app.input_mode, InputMode::EditingText(TextEditTarget::SourcePressure)) {
        render_pressure_dialog(f, app, area);
        return;
    }
    if matches!(app.input_mode, InputMode::EditingText(TextEditTarget::LinkPath)) {
        render_link_dialog(f, app, area);
        return;
    }

    let is_note = matches!(app.input_mode, InputMode::EditingText(TextEditTarget::NoteText));
    let is_edit = app.edit_annotation_pos.is_some();

    let (accent, dim_bg) = if is_note {
        (Color::Rgb(80, 220, 230), Color::Rgb(0, 12, 16))
    } else {
        (Color::Rgb(255, 230, 60), Color::Rgb(18, 16, 0))
    };

    // Note: 3 text rows + h-scroll row = +1 over label's single row.
    let h: u16 = if is_note { 10 } else { 8 };
    let w: u16 = 62u16.min(area.width.saturating_sub(4));
    let popup = centered_rect_abs(w, h, area);

    f.render_widget(Clear, popup);

    let title = match (is_note, is_edit) {
        (false, false) => " New Label ",
        (false, true)  => " Edit Label ",
        (true,  false) => " New Note ",
        (true,  true)  => " Edit Note ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(title, Style::default().fg(accent).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(dim_bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // content_w: inner minus 2-char indent, 1 v-scrollbar, 1 gap.
    let content_w = (inner.width as usize).saturating_sub(4);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::raw("")));

    if is_note {
        lines.push(Line::from(vec![
            Span::styled("  Note text  ", Style::default().fg(Color::Gray)),
            Span::styled("[Shift+Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" new line  ", Style::default().fg(Color::Gray)),
            Span::styled("[↑↓←→]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" navigate", Style::default().fg(Color::Gray)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Label text  ", Style::default().fg(Color::Gray)),
            Span::styled("[←→]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" navigate", Style::default().fg(Color::Gray)),
        ]));
    }
    lines.push(Line::from(Span::raw("")));

    if is_note {
        render_note_editor(&mut lines, app, content_w, accent);
    } else {
        render_label_editor(&mut lines, app, inner.width as usize, accent);
    }

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(if is_edit { " Update  " } else { " Confirm  " }),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel"),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_pressure_dialog(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = Color::Rgb(100, 180, 255);
    let dim_bg = Color::Rgb(0, 10, 20);
    let popup = centered_rect_abs(36, 7, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(" Inlet Pressure ", Style::default().fg(accent).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(dim_bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let buf = &app.input_buffer;
    let cursor_pos = app.note_cursor_pos.min(buf.len());
    let text_style   = Style::default().fg(Color::White).bg(Color::Rgb(20, 30, 50)).add_modifier(Modifier::BOLD);
    let cursor_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);

    let before   = &buf[..cursor_pos];
    let (cur_ch, after) = if cursor_pos < buf.len() {
        (&buf[cursor_pos..cursor_pos + 1], &buf[cursor_pos + 1..])
    } else {
        (" ", "")
    };

    // Field is 14 chars wide (36 popup - 2 border - 2 pad - 18 label)
    let field_w = (inner.width as usize).saturating_sub(12);
    let pad = field_w.saturating_sub(before.len() + 1 + after.len());

    let mut lines: Vec<Line> = vec![Line::from(Span::raw(""))];
    lines.push(Line::from(vec![
        Span::styled("  PSI (10–200)  ", Style::default().fg(Color::Rgb(160, 160, 160))),
    ]));
    lines.push(Line::from(Span::raw("")));

    let mut input_spans = vec![Span::raw("  ")];
    if !before.is_empty() { input_spans.push(Span::styled(before.to_string(), text_style)); }
    input_spans.push(Span::styled(cur_ch.to_string(), cursor_style));
    if !after.is_empty()  { input_spans.push(Span::styled(after.to_string(), text_style)); }
    if pad > 0 { input_spans.push(Span::styled(" ".repeat(pad), text_style)); }
    lines.push(Line::from(input_spans));

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Set  "),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel"),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_link_dialog(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = Color::Rgb(255, 185, 55);
    let dim_bg = Color::Rgb(18, 12, 0);
    let is_edit = app.edit_annotation_pos.is_some();
    let title = if is_edit { " Edit Link Path " } else { " New Diagram Link " };

    let w: u16 = 64u16.min(area.width.saturating_sub(4));
    let popup = centered_rect_abs(w, 9, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(title, Style::default().fg(accent).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(dim_bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let mut lines: Vec<Line> = vec![Line::from(Span::raw(""))];
    lines.push(Line::from(vec![
        Span::styled("  Path to linked diagram  ", Style::default().fg(Color::Gray)),
        Span::styled("[←→]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" navigate", Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::from(Span::raw("")));
    render_label_editor(&mut lines, app, inner.width as usize, accent);
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(if is_edit { " Update  " } else { " Place  " }),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel  "),
        Span::styled("⇒", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(" [Enter] on placed link to follow", Style::default().fg(Color::DarkGray)),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_label_editor(lines: &mut Vec<Line>, app: &App, inner_w: usize, accent: Color) {
    // Available width for text: inner minus 2-char indent.
    let label_w = inner_w.saturating_sub(2);

    let content = &app.input_buffer;
    let scroll_col = app.note_scroll_col;
    let cursor_pos = app.note_cursor_pos.min(content.len());

    let h_content = if scroll_col < content.len() { &content[scroll_col..] } else { "" };
    let h_display = &h_content[..h_content.len().min(label_w)];
    let cur_x = cursor_pos.saturating_sub(scroll_col);

    let text_style = Style::default()
        .fg(Color::White)
        .bg(Color::Rgb(28, 28, 52))
        .add_modifier(Modifier::BOLD);
    let cursor_style = Style::default()
        .bg(Color::White)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);

    // Build input row with inverted cursor block.
    let mut input_spans = vec![Span::raw("  ")];
    let before = &h_display[..cur_x.min(h_display.len())];
    let (cur_ch, after) = if cur_x < h_display.len() {
        (&h_display[cur_x..cur_x + 1], &h_display[cur_x + 1..])
    } else {
        (" ", "")
    };
    if !before.is_empty() {
        input_spans.push(Span::styled(before.to_string(), text_style));
    }
    input_spans.push(Span::styled(cur_ch.to_string(), cursor_style));
    if !after.is_empty() {
        input_spans.push(Span::styled(after.to_string(), text_style));
    }
    let pad = label_w.saturating_sub(before.len() + 1 + after.len());
    if pad > 0 {
        input_spans.push(Span::styled(" ".repeat(pad), text_style));
    }
    lines.push(Line::from(input_spans));

    // Horizontal scrollbar row.
    let max_len = content.len();
    let needs_hscroll = max_len > label_w;
    let track_w = label_w.saturating_sub(2); // space for ◄ and ►
    let (hthumb_start, hthumb_end) = if !needs_hscroll || track_w == 0 {
        (0, track_w)
    } else {
        let thumb_w = ((track_w * label_w) / max_len).max(1);
        let max_hscroll = max_len.saturating_sub(label_w);
        let start = if max_hscroll == 0 {
            0
        } else {
            (scroll_col * track_w.saturating_sub(thumb_w)) / max_hscroll
        };
        (start, (start + thumb_w).min(track_w))
    };
    let track_body: String = (0..track_w)
        .map(|i| {
            if !needs_hscroll {
                '─'
            } else if i >= hthumb_start && i < hthumb_end {
                '█'
            } else {
                '░'
            }
        })
        .collect();
    let (hbar_fg, harrow_style) = if needs_hscroll {
        (Color::Rgb(100, 100, 130), Style::default().fg(accent))
    } else {
        (Color::Rgb(40, 40, 60), Style::default().fg(Color::Rgb(40, 40, 60)))
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("◄", harrow_style),
        Span::styled(track_body, Style::default().fg(hbar_fg)),
        Span::styled("►", harrow_style),
    ]));
}

fn render_note_editor(lines: &mut Vec<Line>, app: &App, content_w: usize, accent: Color) {
    // ── Derive cursor position ────────────────────────────────────────────────
    let pos = app.note_cursor_pos.min(app.input_buffer.len());
    let before = &app.input_buffer[..pos];
    let cursor_line = before.chars().filter(|&c| c == '\n').count();
    let cursor_col  = match before.rfind('\n') {
        Some(nl) => pos - nl - 1,
        None     => pos,
    };

    let segments: Vec<&str> = if app.input_buffer.is_empty() {
        vec![""]
    } else {
        app.input_buffer.split('\n').collect()
    };
    let total = segments.len();

    let scroll_row = app.note_scroll_row.min(total.saturating_sub(VIS_ROWS));
    let scroll_col = app.note_scroll_col;

    // ── Proportional vertical scrollbar ──────────────────────────────────────
    let needs_vscroll = total > VIS_ROWS;
    let (vthumb_start, vthumb_end) = if !needs_vscroll {
        (0, VIS_ROWS)
    } else {
        let thumb_h = ((VIS_ROWS * VIS_ROWS) / total).max(1);
        let max_scroll = total - VIS_ROWS;
        let start = if max_scroll == 0 { 0 } else {
            (scroll_row * (VIS_ROWS - thumb_h)) / max_scroll
        };
        (start, (start + thumb_h).min(VIS_ROWS))
    };

    // ── Render 3 text rows ───────────────────────────────────────────────────
    let text_style  = Style::default().fg(Color::White).bg(Color::Rgb(28, 28, 52)).add_modifier(Modifier::BOLD);
    let track_bg    = Color::Rgb(18, 18, 35);
    let thumb_fg    = Color::Rgb(140, 140, 170);
    let track_fg    = Color::Rgb(45, 45, 65);

    let cursor_style = Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD);

    for row in 0..VIS_ROWS {
        let seg_idx = scroll_row + row;
        let on_cursor_line = seg_idx == cursor_line;
        let content = if seg_idx < total { segments[seg_idx] } else { "" };

        // Apply horizontal scroll; text is ASCII so byte indexing is safe.
        let h_content = if scroll_col < content.len() { &content[scroll_col..] } else { "" };
        let h_display = &h_content[..h_content.len().min(content_w)];

        // Vertical scrollbar character.
        let (vsb_ch, vsb_fg) = if !needs_vscroll {
            ('│', track_fg)
        } else if row >= vthumb_start && row < vthumb_end {
            ('█', thumb_fg)
        } else {
            ('░', track_fg)
        };
        let vsb_span = Span::styled(vsb_ch.to_string(), Style::default().fg(vsb_fg).bg(track_bg));

        if on_cursor_line {
            let cur_x = cursor_col.saturating_sub(scroll_col);

            if cur_x >= content_w {
                // Cursor scrolled off the right edge — render as a plain row.
                let pad = content_w.saturating_sub(h_display.len());
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{h_display}{:pad$}", "", pad = pad), text_style),
                    vsb_span,
                ]));
            } else {
                // Split the row: [before_cursor][cursor_char][after_cursor][pad]
                let before = &h_display[..cur_x.min(h_display.len())];

                let (cur_ch, after) = if cur_x < h_display.len() {
                    // Highlight the existing character that sits under the cursor.
                    let ch = &h_display[cur_x..cur_x + 1]; // ASCII: 1 byte
                    let rest = &h_display[cur_x + 1..];
                    (ch, rest)
                } else {
                    // Cursor is past the end of the line — show a blank cursor block.
                    (" ", "")
                };

                let used = before.len() + 1 + after.len();
                let pad  = content_w.saturating_sub(used);

                let mut spans = vec![Span::raw("  ")];
                if !before.is_empty() {
                    spans.push(Span::styled(before.to_string(), text_style));
                }
                spans.push(Span::styled(cur_ch.to_string(), cursor_style));
                if !after.is_empty() {
                    spans.push(Span::styled(after.to_string(), text_style));
                }
                if pad > 0 {
                    spans.push(Span::styled(" ".repeat(pad), text_style));
                }
                spans.push(vsb_span);
                lines.push(Line::from(spans));
            }
        } else {
            let pad = content_w.saturating_sub(h_display.len());
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{h_display}{:pad$}", "", pad = pad), text_style),
                vsb_span,
            ]));
        }
    }

    // ── Horizontal scrollbar ─────────────────────────────────────────────────
    let max_line_len = segments.iter().map(|s| s.len()).max().unwrap_or(0);
    let needs_hscroll = max_line_len > content_w;

    // Track body = content_w - 2 (subtract ◄ and ►).
    let track_w = content_w.saturating_sub(2);

    let (hthumb_start, hthumb_end) = if !needs_hscroll || track_w == 0 {
        (0, track_w)
    } else {
        let thumb_w = ((track_w * content_w) / max_line_len).max(1);
        let max_hscroll = max_line_len.saturating_sub(content_w);
        let start = if max_hscroll == 0 { 0 } else {
            (scroll_col * (track_w.saturating_sub(thumb_w))) / max_hscroll
        };
        (start, (start + thumb_w).min(track_w))
    };

    let track_body: String = (0..track_w).map(|i| {
        if !needs_hscroll   { '─' }
        else if i >= hthumb_start && i < hthumb_end { '█' }
        else                { '░' }
    }).collect();

    let (hbar_fg, harrow_style) = if needs_hscroll {
        (Color::Rgb(100, 100, 130), Style::default().fg(accent))
    } else {
        (Color::Rgb(40, 40, 60), Style::default().fg(Color::Rgb(40, 40, 60)))
    };

    let pos_label = format!("  {}L:{}C", cursor_line + 1, cursor_col + 1);
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("◄", harrow_style),
        Span::styled(track_body, Style::default().fg(hbar_fg)),
        Span::styled("►", harrow_style),
        Span::styled(pos_label, Style::default().fg(Color::Rgb(55, 55, 75))),
    ]));
}
