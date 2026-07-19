use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::components::{ComponentKind, PipeMaterial};
use crate::glyphs::{GlyphRegistry, COLOR_PALETTE, COLOR_PALETTE_COLS};

pub(super) fn render_palette(f: &mut Frame, app: &App, area: Rect) {
    use crate::app::Focus;

    let palette_focused = matches!(app.focus, Focus::Palette | Focus::PaletteColors);
    let border_style = if palette_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let border_type = if palette_focused { BorderType::Thick } else { BorderType::Plain };

    let (mr, mg, mb) = GlyphRegistry::material_color(app.selected_material);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_style)
        .title(format!(
            " {}  {} ",
            app.selected_diameter.label(),
            app.selected_material.label()
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Bottom section: materials + color swatches inline on the same rows.
    let color_rows = ((COLOR_PALETTE.len() + COLOR_PALETTE_COLS - 1) / COLOR_PALETTE_COLS) as u16;
    let bottom_h = 8u16.max(color_rows + 2);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(bottom_h)])
        .split(inner);
    let list_area   = chunks[0];
    let mat_area    = chunks[1];

    // ── Component list ────────────────────────────────────────────────────────
    const LABEL_W: usize = 15;
    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(list_area);

    let list_focused   = app.focus == Focus::Palette;
    let colors_focused = app.focus == Focus::PaletteColors;
    let hdr_fg  = if list_focused   { Color::Rgb(120, 120, 120) } else { Color::Rgb(50, 50, 50) };
    let hdr_sep = if list_focused   { Color::Rgb(80, 80, 80)    } else { Color::Rgb(35, 35, 35) };
    let hdr_len = if list_focused   { Color::Rgb(80, 140, 160)  } else { Color::Rgb(35, 65, 75) };
    let _ = colors_focused;

    if app.palette_search_active {
        // ── Search bar replaces header row ────────────────────────────────────
        let query = &app.palette_search;
        let query_count = if query.is_empty() {
            app.palette.len()
        } else {
            let q = query.to_lowercase();
            (0..app.palette.len()).filter(|&i| app.palette_item_matches(i, &q)).count()
        };
        let count_text = format!(" {query_count} match");
        let available_w = list_chunks[0].width as usize;
        let prefix = "/ ";
        let suffix = "│";
        let cursor_block = "█";
        let esc_hint = " [Esc]";
        // Truncate query to fit in the available width
        let max_q_w = available_w
            .saturating_sub(prefix.len() + cursor_block.len() + count_text.len() + suffix.len() + esc_hint.len());
        let display_q: String = query.chars().rev().take(max_q_w).collect::<String>().chars().rev().collect();
        let search_line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(display_q, Style::default().fg(Color::White).bg(Color::Rgb(25, 28, 50))),
            Span::styled(cursor_block, Style::default().fg(Color::Yellow).bg(Color::Rgb(25, 28, 50))),
            Span::styled(count_text, Style::default().fg(Color::Rgb(100, 180, 100))),
            Span::styled(suffix, Style::default().fg(Color::Rgb(50, 50, 50))),
            Span::styled(esc_hint, Style::default().fg(Color::Rgb(100, 100, 100))),
        ]);
        f.render_widget(Paragraph::new(search_line), list_chunks[0]);
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("    {:<width$}", "Component", width = LABEL_W),
                    Style::default().fg(hdr_fg),
                ),
                Span::styled("│", Style::default().fg(hdr_sep)),
                Span::styled(" Len  ", Style::default().fg(hdr_fg)),
                Span::styled("[L]", Style::default().fg(hdr_len)),
            ])),
            list_chunks[0],
        );
    }

    let visible_h = list_chunks[1].height as usize;
    let palette_len = app.palette.len();
    let pal_scroll = if palette_len > visible_h && app.palette_idx >= visible_h {
        (app.palette_idx + 1).saturating_sub(visible_h).min(palette_len.saturating_sub(visible_h))
    } else {
        0
    };

    let search_query = if app.palette_search_active && !app.palette_search.is_empty() {
        Some(app.palette_search.to_lowercase())
    } else {
        None
    };

    let items: Vec<ListItem> = app
        .palette
        .iter()
        .enumerate()
        .skip(pal_scroll)
        .take(visible_h)
        .map(|(i, kind)| {
            let custom_ci = app.palette_custom_indices.get(i).copied().flatten();
            let (sym, display_label, [r, gr, b]) = if *kind == ComponentKind::Custom {
                let customs = app.glyph_registry.custom_components();
                if let Some(ci) = custom_ci.filter(|&ci| ci < customs.len()) {
                    let def = &customs[ci];
                    (def.glyph.symbol, def.label.as_str(), def.glyph.fg)
                } else {
                    ('?', "Custom Comp", [150u8, 150, 150])
                }
            } else {
                let g = app.glyph_registry.resolve(*kind, app.selected_material, app.selected_diameter);
                (g.symbol, kind.label(), g.fg)
            };
            let selected = i == app.palette_idx;
            // Dim items that don't match the active search query.
            let matches = search_query.as_ref().map(|q| app.palette_item_matches(i, q)).unwrap_or(true);
            let len_text = if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let in_val = (app.default_lengths.get(kind).copied().unwrap_or(1.0) * 12.0).round() as i32;
                format!("{:>3}\"", in_val)
            } else if kind.has_arm_stubs() {
                let arm = app.default_arm_lengths.get(kind).copied().unwrap_or([0.0; 4]);
                let (n, s, e, w) = kind.connections();
                let dirs: [(&str, bool, f32); 4] = [("N", n, arm[0]), ("S", s, arm[1]), ("E", e, arm[2]), ("W", w, arm[3])];
                dirs.iter()
                    .filter(|(_, active, _)| *active)
                    .map(|(d, _, ft)| {
                        let in_val = (ft * 12.0).round() as i32;
                        if in_val > 0 { format!("{}:{:>2}\"", d, in_val) } else { format!("{}:--", d) }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                String::new()
            };
            let has_len = !len_text.is_empty();

            let (name_style, sep_style, len_style) = if selected {
                (
                    Style::default().fg(Color::Rgb(190, 200, 215)).add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Rgb(80, 80, 80)),
                    if has_len {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                )
            } else if !matches {
                // Dim non-matching items during search.
                (
                    Style::default().fg(Color::Rgb(45, 45, 45)),
                    Style::default().fg(Color::Rgb(30, 30, 30)),
                    Style::default().fg(Color::Rgb(35, 35, 35)),
                )
            } else {
                (
                    Style::default().fg(Color::Rgb(r, gr, b)),
                    Style::default().fg(Color::Rgb(40, 40, 40)),
                    Style::default().fg(Color::Rgb(80, 80, 80)),
                )
            };

            // Selected:   ">[{sym} {label:<14}]"  = 2+1+1+14+1 = 19 chars
            // Unselected: "  {sym} {label:<15}"   = 2+1+1+15   = 19 chars
            // Both keep the │ separator at the same column as the header.
            let name_text = if selected {
                format!(">[{sym} {display_label:<14}]")
            } else {
                format!("  {sym} {display_label:<15}")
            };
            ListItem::new(Line::from(vec![
                Span::styled(name_text, name_style),
                Span::styled("│", sep_style),
                Span::styled(format!(" {len_text}"), len_style),
            ]))
        })
        .collect();

    f.render_widget(List::new(items), list_chunks[1]);

    // Scrollbar for the palette list
    if palette_len > visible_h && visible_h > 0 {
        let bar_len = visible_h;
        let bar_h = ((bar_len * bar_len) / palette_len).max(1).min(bar_len);
        let max_scroll = palette_len.saturating_sub(visible_h);
        let bar_y = if max_scroll == 0 { 0 } else { pal_scroll * (bar_len - bar_h) / max_scroll };
        let bar_col = list_chunks[1].x + list_chunks[1].width.saturating_sub(1);
        for i in 0..bar_len {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, col) = if in_bar {
                ('█', Color::Rgb(70, 70, 100))
            } else {
                ('░', Color::Rgb(25, 25, 35))
            };
            f.render_widget(
                Paragraph::new(Span::styled(
                    ch.to_string(),
                    Style::default().fg(col),
                )),
                ratatui::layout::Rect::new(bar_col, list_chunks[1].y + i as u16, 1, 1),
            );
        }
    }

    // ── Material legend + inline color swatches ───────────────────────────────
    const ALL_MATS: [PipeMaterial; 6] = [
        PipeMaterial::Copper,
        PipeMaterial::PEX,
        PipeMaterial::PE,
        PipeMaterial::GalvanizedIron,
        PipeMaterial::BlackPlastic,
        PipeMaterial::CastIron,
    ];

    let color_active       = app.selected_component_kind().supports_color_override();
    let is_custom          = app.build_custom_rgb.is_some();
    let palette_rows_count = (COLOR_PALETTE.len() + COLOR_PALETTE_COLS - 1) / COLOR_PALETTE_COLS;

    // Material scroll: auto-follow selected material so the list stays visible
    // when more materials are added.  Material portion is always exactly 19 chars
    // wide (4 prefix + 2 swatch + 13 label), which aligns column 20 with the
    // "│ Len" column in the component list header above.
    let data_rows        = (bottom_h as usize).saturating_sub(2); // title row + custom-hint row
    let selected_mat_idx = ALL_MATS.iter().position(|&m| m == app.selected_material).unwrap_or(0);
    let mat_scroll       = selected_mat_idx.saturating_sub(data_rows.saturating_sub(1));

    let mat_title_fg = if app.focus == Focus::PaletteColors {
        Color::Rgb(120, 120, 120)
    } else {
        Color::Rgb(60, 60, 60)
    };
    let color_head_style = if app.focus == Focus::PaletteColors && color_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if color_active {
        Style::default().fg(Color::Rgb(70, 70, 70))
    } else {
        Style::default().fg(Color::Rgb(35, 35, 35))
    };

    // Title row — material title (19 chars), gap, color title
    let mut combined: Vec<Line> = vec![Line::from(vec![
        Span::styled("─ Materials [1-6] ─", Style::default().fg(mat_title_fg)),
        Span::raw("   "),
        Span::styled("Color ─", color_head_style),
    ])];

    // Data rows: one material entry (19 chars) + space + one color-palette row
    for display_row in 0..data_rows {
        let mat_idx = mat_scroll + display_row;
        let mut spans: Vec<Span> = Vec::new();

        // Material portion.
        // Unselected: " 1 ■ label_13chars"  (18 chars) + 3-space gap = 21 before colors
        // Selected:   ">1[■ label_13chars]"  (19 chars) + 2-space gap = 21 before colors
        let mat_gap;
        if mat_idx < ALL_MATS.len() {
            let mat      = ALL_MATS[mat_idx];
            let (r, g, b) = GlyphRegistry::material_color(mat);
            let selected  = mat == app.selected_material;
            if selected {
                mat_gap = "  ";
                spans.push(Span::styled(
                    format!(">{}[", mat_idx + 1),  // e.g. ">1["
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("■ ", Style::default().fg(Color::Rgb(r, g, b))));
                spans.push(Span::styled(
                    format!("{:<13}", mat.label()),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else {
                mat_gap = "   ";
                spans.push(Span::styled(
                    format!(" {} ", mat_idx + 1),
                    Style::default().fg(Color::Rgb(50, 50, 50)),
                ));
                spans.push(Span::styled("■ ", Style::default().fg(Color::Rgb(r, g, b))));
                spans.push(Span::styled(
                    format!("{:<13}", mat.label()),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            }
        } else {
            mat_gap = "   ";
            spans.push(Span::raw(format!("{:<18}", "")));
        }

        // Gap between material and color columns (width varies to hold column alignment)
        spans.push(Span::raw(mat_gap));

        // Color swatch row — always starts from palette row 0
        if display_row < palette_rows_count {
            let base = display_row * COLOR_PALETTE_COLS;
            for col in 0..COLOR_PALETTE_COLS {
                let i = base + col;
                if i >= COLOR_PALETTE.len() { break; }
                let (r, g, b, _) = COLOR_PALETTE[i];
                let sel = color_active && !is_custom && i == app.build_color_cursor;
                let fg  = if color_active {
                    Color::Rgb(r, g, b)
                } else {
                    Color::Rgb((r / 5).max(8), (g / 5).max(8), (b / 5).max(8))
                };
                spans.push(Span::styled(
                    "■ ",
                    Style::default()
                        .fg(fg)
                        .add_modifier(if sel { Modifier::REVERSED } else { Modifier::empty() }),
                ));
            }
        }

        combined.push(Line::from(spans));
    }

    // Custom hint row — blank material portion + space + hint, same column 20
    if color_active {
        let hint = if let Some([r, g, b]) = app.build_custom_rgb {
            Span::styled(
                format!("■  RGB({r},{g},{b})"),
                Style::default().fg(Color::Rgb(r, g, b)).add_modifier(Modifier::REVERSED),
            )
        } else {
            Span::styled("[E] custom R,G,B", Style::default().fg(Color::Rgb(55, 55, 55)))
        };
        combined.push(Line::from(vec![
            Span::raw(format!("{:<19}", "")),
            Span::raw("   "),
            hint,
        ]));
    }

    let _ = (mr, mg, mb);
    f.render_widget(Paragraph::new(combined), mat_area);
}

