use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, InputMode, TextEditTarget};
use crate::components::ComponentKind;
use crate::glyphs::{
    GlyphEditorFocus, PortKind, CHAR_PALETTE, CHAR_PALETTE_COLS,
    CHAR_PALETTE_SYMBOLICS_LEN, COLOR_PALETTE, COLOR_PALETTE_COLS,
};

use super::{composite_box_char, panel_block};

pub(super) fn render_glyph_editor(f: &mut Frame, app: &App) {
    use ratatui::widgets::{Block, Borders};

    let area = f.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Glyph Editor — [G/Q] exit  [Tab] panel  [Enter] apply  [N] new  [R] rename  [C] copy  [Del] delete  [W] composite  [I/O/D] ports  [E] color  [S] save  [L] load ");
    let inner_area = outer.inner(area);
    f.render_widget(outer, area);

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(inner_area);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(52),
            Constraint::Percentage(28),
        ])
        .split(vchunks[0]);

    render_editor_component_list(f, app, hchunks[0]);

    let static_len = ComponentKind::all_palette().len();
    let center_composite: Option<(usize, usize, usize)> = if app.editor.kind_idx >= static_len {
        let ci = app.editor.kind_idx - static_len;
        let customs = app.glyph_registry.custom_components();
        if ci < customs.len() {
            customs[ci].composite_size.map(|(w, h)| (ci, w, h))
        } else { None }
    } else { None };

    if let Some((ci, canvas_w, canvas_h)) = center_composite {
        // Display adds +2 visual buffer ring: display_fw = canvas_w + 2.
        let display_fw = canvas_w + 2;
        let display_fh = canvas_h + 2;
        let avail_h = hchunks[1].height;
        let grid_h_exact = (display_fh + 2) as u16;
        let center_split = if grid_h_exact + 8 <= avail_h {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(grid_h_exact), Constraint::Min(0)])
                .split(hchunks[1])
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(0)])
                .split(hchunks[1])
        };
        render_composite_grid_editor(f, app, center_split[0], ci, display_fw, display_fh, display_fh / 2);
        render_editor_char_grid(f, app, center_split[1]);
    } else {
        render_editor_char_grid(f, app, hchunks[1]);
    }

    render_editor_color_picker(f, app, hchunks[2]);
    render_editor_status(f, app, vchunks[1]);
}

fn render_editor_component_list(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.editor.focus == GlyphEditorFocus::ComponentList;
    let block = panel_block("Components", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let palette = ComponentKind::all_palette();
    let static_len = palette.len();
    let customs = app.glyph_registry.custom_components();

    let mut all_items: Vec<ListItem> = palette
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let g = app.glyph_registry.resolve(kind, app.pal.selected_material, app.pal.selected_diameter);
            let sym = g.symbol;
            let [r, gr, b] = g.fg;
            let label = format!("{sym} {}", kind.label());
            if i == app.editor.kind_idx {
                ListItem::new(format!("> {label}"))
                    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                ListItem::new(format!("  {label}"))
                    .style(Style::default().fg(Color::Rgb(r, gr, b)))
            }
        })
        .collect();

    for (ci, def) in customs.iter().enumerate() {
        let i = static_len + ci;
        let [r, gr, b] = def.glyph.fg;
        let label = format!("{} {}", def.glyph.symbol, def.label);
        if i == app.editor.kind_idx {
            all_items.push(
                ListItem::new(format!("> {label}"))
                    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            );
        } else {
            all_items.push(
                ListItem::new(format!("  {label}"))
                    .style(Style::default().fg(Color::Rgb(r, gr, b))),
            );
        }
    }

    let total = all_items.len();
    let visible_h = inner.height as usize;
    let scroll = if total > visible_h && app.editor.kind_idx >= visible_h {
        (app.editor.kind_idx + 1).saturating_sub(visible_h).min(total.saturating_sub(visible_h))
    } else {
        0
    };
    let items: Vec<ListItem> = all_items.into_iter().skip(scroll).take(visible_h).collect();
    f.render_widget(List::new(items), inner);

    // Scrollbar
    if total > visible_h && visible_h > 0 {
        use ratatui::text::Span as S;
        let bar_len = visible_h;
        let bar_h = ((bar_len * bar_len) / total).max(1).min(bar_len);
        let max_scroll = total.saturating_sub(visible_h);
        let bar_y = if max_scroll == 0 { 0 } else { scroll * (bar_len - bar_h) / max_scroll };
        let bar_col = inner.x + inner.width.saturating_sub(1);
        for i in 0..bar_len {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, col) = if in_bar {
                ('█', Color::Rgb(70, 70, 100))
            } else {
                ('░', Color::Rgb(25, 25, 35))
            };
            f.render_widget(
                ratatui::widgets::Paragraph::new(S::styled(ch.to_string(), Style::default().fg(col))),
                ratatui::layout::Rect::new(bar_col, inner.y + i as u16, 1, 1),
            );
        }
    }
}

fn render_composite_grid_editor(
    f: &mut Frame,
    app: &App,
    area: Rect,
    ci: usize,
    fw: usize,  // display width  = canvas_w + 2  (includes visual buffer ring)
    fh: usize,  // display height = canvas_h + 2
    _port_row: usize,
) {
    let focused = app.editor.focus == GlyphEditorFocus::CompositeGrid;
    // Canvas dimensions (the actual footprint stored in composite_size)
    let canvas_w = fw.saturating_sub(2);
    let canvas_h = fh.saturating_sub(2);
    let canvas_pr = canvas_h / 2;

    let (vr, vc) = app.editor.composite_viewport;
    let scroll_info = if fh > 20 || fw > 40 {
        format!(" [{},{}]", vr, vc)
    } else {
        String::new()
    };
    let block = panel_block(
        &format!("Tile Grid [{canvas_w}×{canvas_h}]{scroll_info}  ↑↓←→  Enter place  Del clear  I/O/D port  (·=margin  60×60 max)"),
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let customs = app.glyph_registry.custom_components();
    if ci >= customs.len() { return; }
    let def = &customs[ci];
    let label = &def.label;
    let (cursor_r, cursor_c) = app.editor.composite_cursor;
    let [base_r, base_g, base_b] = def.glyph.fg;

    let max_rows = inner.height as usize;
    let max_cols = inner.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    for dr in vr..(vr + max_rows).min(fh) {
        let mut spans: Vec<Span> = Vec::new();
        for dc in vc..(vc + max_cols).min(fw) {
            let is_cursor = focused && dr == cursor_r && dc == cursor_c;
            // Display buffer ring at display dc=0, dc=fw-1, dr=0, dr=fh-1.
            let is_visual_buffer = dr == 0 || dr + 1 == fh || dc == 0 || dc + 1 == fw;
            // Data (canvas) coordinates: data = display - 1.
            let (data_r, data_c) = (dr.wrapping_sub(1), dc.wrapping_sub(1));
            let has_override = !is_visual_buffer && def.get_cell(data_r, data_c).is_some();
            let port = if !is_visual_buffer { def.get_port_at(data_r, data_c) } else { None };
            let ch = if let Some(p) = port {
                port_glyph_char(fw, fh, dr, dc, &p.kind)
            } else if is_visual_buffer {
                '·'
            } else {
                def.get_cell(data_r, data_c).unwrap_or_else(|| {
                    composite_box_char(canvas_w, canvas_h, canvas_pr, data_r, data_c, label, None, true)
                })
            };
            let [cr, cg, cb] = if !is_visual_buffer {
                def.get_cell_color(data_r, data_c).unwrap_or([base_r, base_g, base_b])
            } else {
                [base_r, base_g, base_b]
            };
            let style = if is_cursor {
                Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else if let Some(p) = port {
                match p.kind {
                    PortKind::Inlet  => Style::default().fg(Color::Rgb(60, 200, 100)).add_modifier(Modifier::BOLD),
                    PortKind::Outlet => Style::default().fg(Color::Rgb(80, 160, 255)).add_modifier(Modifier::BOLD),
                    PortKind::Drain  => Style::default().fg(Color::Rgb(220, 130, 40)).add_modifier(Modifier::BOLD),
                }
            } else if is_visual_buffer {
                Style::default().fg(Color::Rgb(45, 45, 45))
            } else if has_override {
                Style::default().fg(Color::Rgb(cr, cg, cb)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(cr, cg, cb))
            };
            spans.push(Span::styled(ch.to_string(), style));
        }
        if focused && dr == cursor_r {
            let preview_ch = app.editor.current_symbol();
            // Buffer ring: dc=0 or dc=fw-1 or dr=0 or dr=fh-1.
            let in_buffer = cursor_r == 0 || cursor_r + 1 == fh || cursor_c == 0 || cursor_c + 1 == fw;
            // Canvas border (where ports go): display dc=1/canvas_w or dr=1/canvas_h.
            let on_box_border = cursor_c == 1 || cursor_c == fw - 2 || cursor_r == 1 || cursor_r == fh - 2;
            let cursor_on_border = !in_buffer && on_box_border;
            // Data coords for port lookup.
            let (dcr, dcc) = (cursor_r.wrapping_sub(1), cursor_c.wrapping_sub(1));
            let port_hint = if cursor_on_border {
                match def.get_port_at(dcr, dcc) {
                    None                       => "  [I]nlet  [O]utlet  [D]rain",
                    Some(p) if p.kind == PortKind::Inlet  => "  [I]=Inlet✓  [O]utlet  [D]rain",
                    Some(p) if p.kind == PortKind::Outlet => "  [I]nlet  [O]=Outlet✓  [D]rain",
                    Some(_)                    => "  [I]nlet  [O]utlet  [D]=Drain✓",
                }
            } else {
                ""
            };
            spans.push(Span::styled(
                format!("  ← '{preview_ch}'{port_hint}"),
                Style::default().fg(Color::Rgb(80, 80, 80)),
            ));
        }
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_editor_char_grid(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.editor.focus == GlyphEditorFocus::CharGrid;
    let block = panel_block("Character Picker  ↑↓←→  [W] make composite", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cols = CHAR_PALETTE_COLS;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "── Symbolics ──────────────────────────────────────",
        Style::default().fg(Color::Rgb(70, 70, 70)),
    )));

    let mut idx = 0;
    while idx < CHAR_PALETTE.len() {
        if idx == CHAR_PALETTE_SYMBOLICS_LEN {
            lines.push(Line::from(Span::styled(
                "── Alpha-Numerics ─────────────────────────────────",
                Style::default().fg(Color::Rgb(70, 70, 70)),
            )));
        }
        let mut spans: Vec<Span> = Vec::new();
        for col in 0..cols {
            let i = idx + col;
            if i >= CHAR_PALETTE.len() { break; }
            let ch = CHAR_PALETTE[i];
            let style = if i == app.editor.char_cursor {
                Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            spans.push(Span::styled(format!("{ch} "), style));
        }
        lines.push(Line::from(spans));
        idx += cols;
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_editor_color_picker(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.editor.focus == GlyphEditorFocus::ColorPicker;
    let title = if focused {
        " Color Picker  [↑↓←→] select  [E] custom RGB "
    } else {
        " Color Picker "
    };
    let block = panel_block(title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cols = COLOR_PALETTE_COLS;
    let mut lines: Vec<Line> = Vec::new();

    let mut idx = 0;
    while idx < COLOR_PALETTE.len() {
        let mut spans: Vec<Span> = Vec::new();
        for col in 0..cols {
            let i = idx + col;
            if i >= COLOR_PALETTE.len() { break; }
            let (r, g, b, label) = COLOR_PALETTE[i];
            let is_custom_active = app.editor.custom_rgb.is_some();
            let selected = !is_custom_active && i == app.editor.color_cursor;
            let bullet_style = Style::default()
                .fg(Color::Rgb(r, g, b))
                .add_modifier(if selected { Modifier::BOLD | Modifier::REVERSED } else { Modifier::empty() });
            spans.push(Span::styled(format!("■{label:<9} "), bullet_style));
        }
        lines.push(Line::from(spans));
        idx += cols;
    }

    // Custom RGB swatch row
    if let Some([r, g, b]) = app.editor.custom_rgb {
        lines.push(Line::from(vec![
            Span::styled(
                format!("■ Custom  RGB({r},{g},{b})"),
                Style::default()
                    .fg(Color::Rgb(r, g, b))
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "[E] enter custom R,G,B",
            Style::default().fg(Color::Rgb(90, 90, 90)),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_editor_status(f: &mut Frame, app: &App, area: Rect) {
    let ed = &app.editor;

    let palette = ComponentKind::all_palette();
    let static_len = palette.len();
    let kind_label: String = if ed.kind_idx < static_len {
        palette[ed.kind_idx].label().to_string()
    } else {
        let ci = ed.kind_idx - static_len;
        let customs = app.glyph_registry.custom_components();
        if ci < customs.len() { customs[ci].label.clone() } else { "?".to_string() }
    };
    let preview_sym = ed.current_symbol();
    let [pr, pg, pb] = ed.current_color();

    let mut lines: Vec<Line> = Vec::new();

    if let InputMode::EditingText(target) = app.text_input.input_mode {
        let prompt = match target {
            TextEditTarget::SaveLibrary    => "Save library to file: ",
            TextEditTarget::LoadLibrary    => "Load library from file: ",
            TextEditTarget::NewCompName    => "New component name: ",
            TextEditTarget::RenameComp     => "Rename component: ",
            TextEditTarget::CopyComp       => "Copy as new component (name): ",
            TextEditTarget::CompWidth      => "Composite size as WxH (e.g. 17x5, min 3×3; 0 = single-cell): ",
            TextEditTarget::AssemblyName   => "Assembly name: ",
            TextEditTarget::AddGlyphFile   => "",
            TextEditTarget::CustomRgb      => "Custom RGB (R,G,B): ",
            TextEditTarget::BuildCustomRgb => "",
            TextEditTarget::LabelText | TextEditTarget::NoteText | TextEditTarget::SourcePressure
            | TextEditTarget::PrvSetpoint | TextEditTarget::LinkPath | TextEditTarget::CostPrice => "",
        };
        lines.push(Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}|", app.text_input.input_buffer),
                Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 80)).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  [Enter] confirm  [Esc] cancel", Style::default().fg(Color::Gray)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Scope: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("[{}]", ed.mat_label()),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", ed.diam_label()),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    Preview → "),
            Span::styled(
                format!("{preview_sym} "),
                Style::default().fg(Color::Rgb(pr, pg, pb)).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("for {kind_label}"),
                Style::default().fg(Color::White),
            ),
        ]));
    }

    lines.push(Line::from(Span::styled(
        ed.status.as_str(),
        Style::default().fg(Color::Rgb(120, 120, 120)),
    )));

    f.render_widget(Paragraph::new(lines), area);
}

/// Box-drawing connector char for a port at (dr, dc) in extended footprint space.
/// All port types face outward toward their external connection — kind is shown by color only.
fn port_glyph_char(fw: usize, _fh: usize, dr: usize, dc: usize, _kind: &PortKind) -> char {
    if dc == 1           { '╣' } // West border  → arm left,  opening faces outward (left)
    else if dc == fw - 2 { '╠' } // East border  → arm right, opening faces outward (right)
    else if dr == 1      { '╩' } // North border → arm up,    opening faces outward (up)
    else                 { '╦' } // South border → arm down,  opening faces outward (down)
}
