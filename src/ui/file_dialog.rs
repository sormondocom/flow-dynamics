use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::file_dialog::FileDialogMode;

// ── File dialog ───────────────────────────────────────────────────────────────

pub(super) fn render_file_dialog(f: &mut Frame, app: &App) {
    let area = f.area();

    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(4, 6, 18))),
        area,
    );

    let Some(fd) = &app.file_dialog else { return };

    let w = area.width.min(90);
    let h = area.height.saturating_sub(4).max(12);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, dialog);

    let title = match fd.mode {
        FileDialogMode::Save => " Save Layout ",
        FileDialogMode::Open => " Open Layout ",
    };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(55, 110, 175)))
        .style(Style::default().bg(Color::Rgb(8, 12, 28)));

    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let mut row = inner.y;
    let cw = inner.width as usize;

    // ── Current path ──────────────────────────────────────────────────────────
    let path_str = fd.current_dir.to_string_lossy();
    let path_display = if path_str.len() > cw.saturating_sub(4) {
        format!("…{}", &path_str[path_str.len().saturating_sub(cw.saturating_sub(5))..])
    } else {
        path_str.to_string()
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  {path_display}"),
            Style::default().fg(Color::Rgb(100, 160, 220)).add_modifier(Modifier::BOLD),
        )),
        Rect::new(inner.x, row, inner.width, 1),
    );
    row += 1;

    // Separator
    render_hline(f, inner.x, row, inner.width);
    row += 1;

    // ── Entry list ────────────────────────────────────────────────────────────
    // Reserve: 1 separator + 2 (filename row + separator) for save, 1 for error
    let filename_rows: u16 = if fd.mode == FileDialogMode::Save { 3 } else { 1 };
    let error_rows:    u16 = if fd.error_msg.is_some() { 1 } else { 0 };
    let list_h = inner.height
        .saturating_sub(2 + filename_rows + error_rows) as usize; // 2 = path row + sep

    let total = fd.entries.len();
    let scroll = if fd.selected >= list_h {
        (fd.selected - list_h + 1).min(total.saturating_sub(list_h))
    } else {
        0
    };

    for (i, entry) in fd.entries.iter().enumerate().skip(scroll).take(list_h) {
        let selected = i == fd.selected && !fd.focus_input;
        let is_parent = entry.name == "..";

        let cursor = if selected { "▶" } else { " " };
        let icon = if is_parent {
            "▲"
        } else if entry.is_dir {
            "▸"
        } else {
            "·"
        };
        let label = if is_parent {
            format!("  {cursor} {icon}  ..  (parent directory)")
        } else if entry.is_dir {
            format!("  {cursor} {icon}  {}/", entry.name)
        } else {
            format!("  {cursor} {icon}  {}", entry.name)
        };

        let style = if selected {
            Style::default()
                .fg(Color::Rgb(240, 252, 255))
                .bg(Color::Rgb(25, 55, 100))
                .add_modifier(Modifier::BOLD)
        } else if entry.is_dir {
            Style::default().fg(Color::Rgb(100, 180, 255))
        } else {
            Style::default().fg(Color::Rgb(185, 200, 215))
        };

        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{:<width$}", label, width = cw.saturating_sub(1)),
                style,
            )),
            Rect::new(inner.x, row + (i - scroll) as u16, inner.width.saturating_sub(1), 1),
        );
    }

    // Scrollbar
    if total > list_h && list_h > 0 {
        let bar_h = ((list_h * list_h) / total).max(1);
        let max_scroll = total.saturating_sub(list_h);
        let bar_y = if max_scroll == 0 { 0 } else { scroll * (list_h - bar_h) / max_scroll };
        for i in 0..list_h {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, col) = if in_bar {
                ('█', Color::Rgb(55, 110, 175))
            } else {
                ('░', Color::Rgb(25, 35, 55))
            };
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(col))),
                Rect::new(inner.x + inner.width - 1, row + i as u16, 1, 1),
            );
        }
    }

    row += list_h as u16;

    // ── Error message ─────────────────────────────────────────────────────────
    if let Some(err) = &fd.error_msg {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("  ✗ {err}"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Rect::new(inner.x, row, inner.width, 1),
        );
        row += 1;
    }

    // ── Filename input (Save mode only) ───────────────────────────────────────
    if fd.mode == FileDialogMode::Save {
        render_hline(f, inner.x, row, inner.width);
        row += 1;

        let input_focused = fd.focus_input;
        let label = "  Filename: ";
        let input_w = inner.width.saturating_sub(label.len() as u16 + 2);
        let cursor_char = if input_focused { "█" } else { "" };
        let input_text = format!("{}{}{}", label, &fd.filename_input, cursor_char);
        let (fg, bg) = if input_focused {
            (Color::Rgb(240, 252, 255), Color::Rgb(20, 40, 70))
        } else {
            (Color::Rgb(150, 170, 190), Color::Rgb(8, 12, 28))
        };
        let _ = input_w;
        f.render_widget(
            Paragraph::new(Span::styled(
                input_text,
                Style::default().fg(fg).bg(bg),
            )),
            Rect::new(inner.x, row, inner.width, 1),
        );
    }

    // ── Hint bar at bottom of dialog ──────────────────────────────────────────
    let hint_y = dialog.y + dialog.height - 1;
    let hints = if fd.mode == FileDialogMode::Save {
        " [↑↓/jk] Navigate  [Enter] Open/Save  [Tab] Filename  [Bsp/←] Up Dir  [Esc] Cancel "
    } else {
        " [↑↓/jk] Navigate  [Enter] Open/Select  [Bsp/←] Up Dir  [Esc] Cancel "
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            hints,
            Style::default().fg(Color::Rgb(80, 100, 130)),
        )),
        Rect::new(dialog.x + 1, hint_y, dialog.width.saturating_sub(2), 1),
    );
}

// ── Confirm-new overlay ───────────────────────────────────────────────────────

pub(super) fn render_confirm_new(f: &mut Frame, app: &App) {
    let area = f.area();

    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(4, 6, 18))),
        area,
    );

    let w: u16 = 52;
    let h: u16 = 11;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, dialog);

    let is_follow_link = app.pending_link_path.is_some();
    let title = if is_follow_link { " Follow Link " } else { " New Diagram " };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(160, 130, 0)))
        .style(Style::default().bg(Color::Rgb(20, 18, 6)));
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let cy = inner.y;
    let cx = inner.x;
    let cw = inner.width;

    f.render_widget(
        Paragraph::new("The current diagram has unsaved content.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(200, 190, 140))),
        Rect::new(cx, cy, cw, 1),
    );
    f.render_widget(
        Paragraph::new("What would you like to do?")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(140, 130, 90))),
        Rect::new(cx, cy + 1, cw, 1),
    );

    let opts: &[(&str, &str, &str)] = if is_follow_link {
        &[
            ("S", "Save & follow link",             "save & follow"),
            ("D", "Discard changes & follow link",  "discard"),
            ("C", "Cancel — stay in current diagram", "cancel"),
        ]
    } else {
        &[
            ("S", "Save & start new diagram",        "save & new"),
            ("D", "Discard changes & start new",     "discard"),
            ("C", "Cancel — keep current diagram",   "cancel"),
        ]
    };

    for (i, &(key, label, _)) in opts.iter().enumerate() {
        let selected = app.confirm_new_choice == i;
        let cursor = if selected { "▶ " } else { "  " };
        let (key_col, label_col, bg) = if selected {
            (
                Color::Yellow,
                Color::Rgb(240, 230, 160),
                Color::Rgb(40, 35, 5),
            )
        } else {
            (
                Color::Rgb(160, 140, 40),
                Color::Rgb(140, 130, 90),
                Color::Rgb(20, 18, 6),
            )
        };

        let row = cy + 3 + i as u16;
        let line = Line::from(vec![
            Span::raw(format!("    {cursor}")),
            Span::styled(
                format!("[{key}]"),
                Style::default().fg(key_col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {label}"),
                Style::default().fg(label_col),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line).style(Style::default().bg(bg)),
            Rect::new(cx, row, cw, 1),
        );
    }
}

// ── Confirm-quit overlay ──────────────────────────────────────────────────────

pub(super) fn render_confirm_quit(f: &mut Frame, app: &App) {
    let area = f.area();

    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(4, 6, 18))),
        area,
    );

    let w: u16 = 52;
    let h: u16 = 11;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, dialog);

    let block = Block::default()
        .title(Span::styled(
            " Quit Flow Dynamics ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(140, 30, 30)))
        .style(Style::default().bg(Color::Rgb(20, 6, 6)));
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let cy = inner.y;
    let cx = inner.x;
    let cw = inner.width;

    f.render_widget(
        Paragraph::new("Are you sure you want to quit?")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(220, 180, 180))),
        Rect::new(cx, cy, cw, 1),
    );
    f.render_widget(
        Paragraph::new("Any unsaved changes will be lost.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(150, 100, 100))),
        Rect::new(cx, cy + 1, cw, 1),
    );

    let opts: &[(&str, &str)] = &[
        ("S", "Save & Quit"),
        ("Q", "Quit without saving"),
        ("C", "Cancel — stay in the app"),
    ];

    for (i, &(key, label)) in opts.iter().enumerate() {
        let selected = app.confirm_quit_choice == i;
        let cursor = if selected { "▶ " } else { "  " };
        let (key_col, label_col, bg) = if selected {
            (Color::Red, Color::Rgb(255, 200, 200), Color::Rgb(50, 10, 10))
        } else {
            (Color::Rgb(160, 60, 60), Color::Rgb(160, 110, 110), Color::Rgb(20, 6, 6))
        };

        let row = cy + 3 + i as u16;
        let line = Line::from(vec![
            Span::raw(format!("    {cursor}")),
            Span::styled(
                format!("[{key}]"),
                Style::default().fg(key_col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {label}"),
                Style::default().fg(label_col),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line).style(Style::default().bg(bg)),
            Rect::new(cx, row, cw, 1),
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn render_hline(f: &mut Frame, x: u16, y: u16, w: u16) {
    f.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(w as usize),
            Style::default().fg(Color::Rgb(40, 65, 95)),
        )),
        Rect::new(x, y, w, 1),
    );
}
