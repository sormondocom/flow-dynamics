use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, InputMode};
use crate::components::ComponentKind;
use crate::glyphs::GlyphRegistry;

use super::{centered_rect, centered_rect_abs, format_pipe_length};

pub(super) fn render_bom(f: &mut Frame, app: &App, area: Rect) {
    use std::collections::BTreeMap;

    let popup = centered_rect(68, 88, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Bill of Materials  [B/Q] close ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let h1   = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let body = Style::default().fg(Color::White);
    let dim  = Style::default().fg(Color::Rgb(110, 110, 110));
    let sym_s = Style::default().fg(Color::Cyan);

    let mut pipe_map: BTreeMap<(String, String), (char, usize, f32)> = BTreeMap::new();

    let group_defs: &[(&str, char, fn(ComponentKind) -> bool)] = &[
        ("Source (Inlet)",  'S', |k| k == ComponentKind::Source),
        ("Drain (Outlet)",  'D', |k| k == ComponentKind::Sink),
        ("Toilet",          '○', |k| k == ComponentKind::Toilet),
        ("Faucet/Sink",     '≈', |k| k == ComponentKind::Faucet),
        ("Basin Sink",      '⊔', |k| k == ComponentKind::BasinSink),
        ("Water Heater",    '▲', |k| k == ComponentKind::WaterHeater),
        ("Solid Block",     '█', |k| k == ComponentKind::SolidBlock),
        ("Elbow 90°",      '╚', |k| matches!(k,
            ComponentKind::ElbowNE | ComponentKind::ElbowNW |
            ComponentKind::ElbowSE | ComponentKind::ElbowSW)),
        ("Tee 3-way",      '╠', |k| matches!(k,
            ComponentKind::TeeNSE | ComponentKind::TeeNSW |
            ComponentKind::TeeNEW | ComponentKind::TeeSEW)),
        ("Reducer Tee",   '╟', |k| matches!(k,
            ComponentKind::ReducerTeeNSE | ComponentKind::ReducerTeeNSW |
            ComponentKind::ReducerTeeNEW | ComponentKind::ReducerTeeSEW)),
        ("Cross 4-way",    '╬', |k| k == ComponentKind::Cross),
        ("Ball Valve",     '●', |k| matches!(k,
            ComponentKind::BallValveH | ComponentKind::BallValveV)),
        ("Check Valve",    '→', |k| matches!(k,
            ComponentKind::CheckValveH | ComponentKind::CheckValveV)),
        ("End Cap",        '■', |k| k == ComponentKind::EndCap),
        ("Reducer",        '◄', |k| k == ComponentKind::Reducer),
        ("Pressure Gauge", '⊙', |k| k == ComponentKind::PressureGauge),
    ];
    let mut group_counts = vec![0usize; group_defs.len()];
    let mut total_comps = 0usize;

    for r in 0..app.grid.height {
        for c in 0..app.grid.width {
            if let Some(comp) = app.grid.get(r, c) {
                total_comps += 1;
                match comp.kind {
                    ComponentKind::PipeH | ComponentKind::PipeV => {
                        let g = app.glyph_registry
                            .resolve(comp.kind, comp.material, comp.diameter);
                        let key = (
                            comp.material.label().to_string(),
                            comp.diameter.label().to_string(),
                        );
                        let entry = pipe_map.entry(key).or_insert((g.symbol, 0, 0.0));
                        entry.1 += 1;
                        entry.2 += comp.pipe_length;
                    }
                    kind => {
                        for (gi, (_, _, pred)) in group_defs.iter().enumerate() {
                            if pred(kind) {
                                group_counts[gi] += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let total_pipe_ft: f32 = pipe_map.values().map(|(_, _, ft)| ft).sum();
    let total_pipe_segs: usize = pipe_map.values().map(|(_, c, _)| c).sum();

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::raw("")));

    if !pipe_map.is_empty() {
        lines.push(Line::from(Span::styled("  PIPE SEGMENTS", h1)));
        for ((mat, diam), (sym, count, ft)) in &pipe_map {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<14}  {:<7} ", mat, diam), body),
                Span::styled(sym.to_string(), sym_s),
                Span::styled(
                    format!("   {:>3} seg{}   {}",
                        count, if *count == 1 { "" } else { "s" }, format_pipe_length(*ft)),
                    body,
                ),
            ]));
        }
        lines.push(Line::from(Span::raw("")));
    }

    let has_comps = group_counts.iter().any(|&c| c > 0);
    if has_comps {
        lines.push(Line::from(Span::styled("  FITTINGS & COMPONENTS", h1)));
        for (gi, &(name, sym, _)) in group_defs.iter().enumerate() {
            if group_counts[gi] == 0 { continue; }
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<24} ", name), body),
                Span::styled(sym.to_string(), sym_s),
                Span::styled(
                    format!("   {:>3} pc{}", group_counts[gi],
                        if group_counts[gi] == 1 { "" } else { "s" }),
                    body,
                ),
            ]));
        }
        lines.push(Line::from(Span::raw("")));
    }

    lines.push(Line::from(Span::styled("  TOTALS", h1)));
    if total_comps == 0 {
        lines.push(Line::from(Span::styled("    No components placed yet.", dim)));
    } else {
        if total_pipe_segs > 0 {
            lines.push(Line::from(Span::styled(
                format!("    {} pipe segment{}   {}",
                    total_pipe_segs,
                    if total_pipe_segs == 1 { "" } else { "s" },
                    format_pipe_length(total_pipe_ft)),
                body,
            )));
        }
        lines.push(Line::from(Span::styled(
            format!("    {} total component{} placed",
                total_comps, if total_comps == 1 { "" } else { "s" }),
            body,
        )));
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "  [B] or [Q] to close",
        Style::default().fg(Color::Yellow),
    )));

    f.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn render_assembly_browser(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(68, 82, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 180, 120)))
        .title(" Assembly Library  [Enter] stamp  [Del] delete  [Y/Q] close ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let libs = &app.assembly_lib.assemblies;

    if libs.is_empty() {
        let lines = vec![
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "  No assemblies saved yet.",
                Style::default().fg(Color::Rgb(110, 110, 110)),
            )),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "  Use [R] on the canvas to draw a selection rectangle,",
                Style::default().fg(Color::Rgb(80, 80, 80)),
            )),
            Line::from(Span::styled(
                "  then [Enter] to name and save it as an assembly.",
                Style::default().fg(Color::Rgb(80, 80, 80)),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let h1   = Style::default().fg(Color::Rgb(80, 200, 130)).add_modifier(Modifier::BOLD);
    let body = Style::default().fg(Color::White);
    let dim  = Style::default().fg(Color::Rgb(100, 100, 100));

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::raw("")));

    // Track which line the selected entry starts on (for scroll calculation)
    let mut selected_line: usize = 1;

    for (i, asm) in libs.iter().enumerate() {
        let selected = i == app.assembly_idx;
        if selected {
            selected_line = lines.len();
        }
        let prefix = if selected { ">" } else { " " };
        let name_style = if selected { h1 } else { body };
        let count = asm.component_count();
        lines.push(Line::from(vec![
            Span::styled(format!("{prefix} {:<28}", asm.name), name_style),
            Span::styled(
                format!("  {}×{}  {} comp{}", asm.width, asm.height, count,
                    if count == 1 { "" } else { "s" }),
                if selected { Style::default().fg(Color::Rgb(150, 200, 160)) } else { dim },
            ),
        ]));
        if selected && !asm.description.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", asm.description),
                Style::default().fg(Color::Rgb(120, 160, 130)),
            )));
        }
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(vec![
        Span::styled("[↑↓] navigate  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Enter] stamp on canvas  ", Style::default().fg(Color::LightGreen)),
        Span::styled("[Del] delete  ", Style::default().fg(Color::Red)),
        Span::styled("[Y/Q] close", Style::default().fg(Color::Yellow)),
    ]));

    let base_lines = lines.len();

    if app.assembly_idx < libs.len() {
        let asm = &libs[app.assembly_idx];
        let preview_h = asm.height.min(10) as u16;

        if inner.height > base_lines as u16 + preview_h + 2 {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                "  Preview:",
                Style::default().fg(Color::Rgb(80, 80, 80)),
            )));

            for r in 0..asm.height.min(10) {
                let mut spans: Vec<Span> = vec![Span::raw("  ")];
                for c in 0..asm.width.min(30) {
                    let span = if let Some(comp) = asm.get(r, c) {
                        let g = app.glyph_registry
                            .resolve(comp.kind, comp.material, comp.diameter);
                        Span::styled(
                            g.symbol.to_string(),
                            Style::default().fg(Color::Rgb(g.fg[0], g.fg[1], g.fg[2])),
                        )
                    } else {
                        Span::styled("·", Style::default().fg(Color::Rgb(30, 30, 30)))
                    };
                    spans.push(span);
                }
                if asm.width > 30 {
                    spans.push(Span::styled("…", Style::default().fg(Color::DarkGray)));
                }
                lines.push(Line::from(spans));
            }
            if asm.height > 10 {
                lines.push(Line::from(Span::styled(
                    "  … (preview truncated)",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    let total_lines = lines.len();
    let available_h = inner.height as usize;
    let scroll_y: u16 = if total_lines > available_h && selected_line + 1 > available_h {
        ((selected_line + 1).saturating_sub(available_h))
            .min(total_lines.saturating_sub(available_h)) as u16
    } else {
        0
    };

    f.render_widget(Paragraph::new(lines).scroll((scroll_y, 0)), inner);

    // Scrollbar on the right edge
    if total_lines > available_h && available_h > 0 {
        let bar_len = available_h;
        let bar_h = ((bar_len * bar_len) / total_lines).max(1).min(bar_len);
        let max_scroll = total_lines.saturating_sub(available_h);
        let bar_y = if max_scroll == 0 { 0 } else {
            scroll_y as usize * (bar_len - bar_h) / max_scroll
        };
        let bar_col = inner.x + inner.width.saturating_sub(1);
        for i in 0..bar_len {
            let in_bar = i >= bar_y && i < bar_y + bar_h;
            let (ch, col) = if in_bar {
                ('█', Color::Rgb(70, 140, 100))
            } else {
                ('░', Color::Rgb(25, 35, 25))
            };
            f.render_widget(
                Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(col))),
                Rect::new(bar_col, inner.y + i as u16, 1, 1),
            );
        }
    }
}

pub(super) fn render_component_detail(f: &mut Frame, app: &App, area: Rect) {
    let kind = app.detail_kind;
    let active = app.detail_active_ports();
    let arm_lengths = app.detail_arm_lengths();
    let port_count = active.len();

    let (disp_mat, disp_diam) = if app.detail_for_palette {
        (app.selected_material, app.selected_diameter)
    } else {
        app.component_at_cursor()
            .map(|c| (c.material, c.diameter))
            .unwrap_or((app.selected_material, app.selected_diameter))
    };
    let g = app.glyph_registry.resolve(kind, disp_mat, disp_diam);
    let [sr, sg, sb] = g.fg;

    let overlay_h = (5 + port_count as u16).max(8);
    let overlay_w = 60u16;
    let overlay = centered_rect_abs(overlay_w, overlay_h, area);
    f.render_widget(Clear, overlay);

    let title = if app.detail_for_palette {
        format!(" Default stubs: {} {} ", g.symbol, kind.label())
    } else {
        format!(" Placed: {} {}  stubs ", g.symbol, kind.label())
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title);
    let inner = block.inner(overlay);
    f.render_widget(block, overlay);

    let mut lines: Vec<Line> = Vec::new();

    let (mr, mg, mb) = GlyphRegistry::material_color(disp_mat);
    lines.push(Line::from(vec![
        Span::styled(disp_diam.label(), Style::default().fg(Color::Rgb(sr, sg, sb)).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(disp_mat.label(), Style::default().fg(Color::Rgb(mr, mg, mb))),
        Span::styled(
            format!("    equiv. friction: {:.0}D", kind.equiv_length_diameters()),
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "─ Stub length at each port ────────────────────────────────",
        Style::default().fg(Color::Rgb(50, 50, 50)),
    )));

    if port_count == 0 {
        lines.push(Line::from(Span::styled(
            "  No connectable ports.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    for (list_idx, &(raw_port, dir_name)) in active.iter().enumerate() {
        let selected = list_idx == app.detail_port_cursor;
        let arm_in = arm_lengths[raw_port] * 12.0;
        let prefix = if selected { "▶" } else { " " };

        if selected && app.input_mode == InputMode::EditingLength {
            let preview_in = app.input_buffer.parse::<f32>().unwrap_or(0.0);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{prefix} {dir_name:<6} "),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}|", app.input_buffer),
                    Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 80)).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" in  ({:.2} ft)   [Enter] [Esc]", preview_in / 12.0),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        } else {
            let len_display = if arm_in < 0.05 { "  --".to_string() } else { format!("{:>4.1}", arm_in) };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{prefix} {dir_name:<6} "),
                    if selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
                    else        { Style::default().fg(Color::Rgb(110, 110, 110)) },
                ),
                Span::styled(
                    format!("{} in", len_display),
                    if selected { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) }
                    else        { Style::default().fg(Color::Rgb(160, 160, 160)) },
                ),
                if selected {
                    Span::styled("   [Enter] to edit", Style::default().fg(Color::Rgb(80, 80, 50)))
                } else { Span::raw("") },
            ]));
        }
    }

    lines.push(Line::from(Span::styled(
        "──────────────────────────────────────────────────────────",
        Style::default().fg(Color::Rgb(50, 50, 50)),
    )));
    lines.push(Line::from(vec![
        Span::styled("[↑↓]", Style::default().fg(Color::Cyan)),
        Span::raw(" port   "),
        Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
        Span::raw(" edit   "),
        Span::styled("[Esc/Q]", Style::default().fg(Color::Rgb(180, 80, 80))),
        Span::raw(" close"),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}
