use std::collections::HashMap;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, InputMode, TextEditTarget};
use crate::components::ComponentKind;
use crate::cost_config::{CostConfig, FITTING_GROUPS};
use crate::glyphs::{ALL_DIAMETERS, ALL_MATERIALS};

pub(super) fn render_cost_estimator(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Cost Estimator  [$] close  [↑↓] navigate  [Enter/E] edit price ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(inner);

    render_price_list(f, app, hchunks[0]);
    render_bom_totals(f, app, hchunks[1]);
}

fn render_price_list(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let editing = matches!(
        app.text_input.input_mode,
        InputMode::EditingText(TextEditTarget::CostPrice)
    );

    let (list_area, edit_area) = if area.height >= 4 {
        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);
        (v[0], Some(v[1]))
    } else {
        (area, None)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 100)))
        .title(" Prices ");
    let inner = block.inner(list_area);
    f.render_widget(block, list_area);

    let pipe_count = ALL_MATERIALS.len() * ALL_DIAMETERS.len();
    let visible_h = inner.height as usize;

    let scroll: usize = if app.cost_cursor >= visible_h {
        app.cost_cursor.saturating_sub(visible_h / 2)
    } else {
        0
    };

    let h1   = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let dim  = Style::default().fg(Color::Rgb(90, 90, 90));
    let body = Style::default().fg(Color::Rgb(200, 200, 200));
    let pr   = Style::default().fg(Color::Rgb(100, 220, 130));
    let sel  = Style::default().bg(Color::Rgb(40, 60, 90)).fg(Color::White);
    let selp = Style::default().bg(Color::Rgb(40, 60, 90)).fg(Color::Rgb(100, 255, 160)).add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled("  PIPE  (per linear foot)", h1)));

    for mat_i in 0..ALL_MATERIALS.len() {
        let mat = ALL_MATERIALS[mat_i];
        for dia_i in 0..ALL_DIAMETERS.len() {
            let dia = ALL_DIAMETERS[dia_i];
            let row_idx = mat_i * ALL_DIAMETERS.len() + dia_i;
            let p = app.config.costs.pipe_price(mat, dia);
            let is_sel = row_idx == app.cost_cursor;

            let mat_str = format!("  {:<16}", mat.label());
            let dia_str = format!("{:<8}", dia.label());
            let p_str   = format!("${:.2}/ft", p);

            if is_sel {
                lines.push(Line::from(vec![
                    Span::styled(mat_str, sel),
                    Span::styled(dia_str, sel),
                    Span::styled(p_str, selp),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(mat_str, body),
                    Span::styled(dia_str, dim),
                    Span::styled(p_str, pr),
                ]));
            }
        }
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled("  COMPONENTS  (per unit)", h1)));

    for (fi, &(key, label, _)) in FITTING_GROUPS.iter().enumerate() {
        let row_idx = pipe_count + fi;
        let p = *app.config.costs.fitting_per_unit.get(key).unwrap_or(&0.0);
        let is_sel = row_idx == app.cost_cursor;

        let label_str = format!("  {:<28}", label);
        let p_str     = format!("${:.2}/ea", p);

        if is_sel {
            lines.push(Line::from(vec![
                Span::styled(label_str, sel),
                Span::styled(p_str, selp),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(label_str, body),
                Span::styled(p_str, pr),
            ]));
        }
    }

    let display: Vec<Line> = lines.into_iter().skip(scroll).take(visible_h).collect();
    f.render_widget(Paragraph::new(display), inner);

    if let Some(ea) = edit_area {
        let edit_block = Block::default()
            .borders(Borders::ALL)
            .border_style(if editing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Rgb(50, 50, 70))
            })
            .title(if editing {
                " $ value — [Enter] confirm  [Esc] cancel "
            } else {
                " [Enter/E] edit selected price "
            });
        let edit_inner = edit_block.inner(ea);
        f.render_widget(edit_block, ea);

        if editing {
            let txt = Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(app.text_input.input_buffer.clone(), Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK)),
            ]);
            f.render_widget(Paragraph::new(txt), edit_inner);
        }
    }
}

fn render_bom_totals(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 100)))
        .title(" Estimated Cost ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let h1    = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let body  = Style::default().fg(Color::Rgb(200, 200, 200));
    let dim   = Style::default().fg(Color::Rgb(90, 90, 90));
    let money = Style::default().fg(Color::Rgb(100, 220, 130));
    let tot   = Style::default().fg(Color::Rgb(255, 220, 60)).add_modifier(Modifier::BOLD);

    // Tally canvas: pipe uses CostConfig::pipe_key() as key, fittings use FITTING_GROUPS key
    let mut pipe_ft: HashMap<String, (String, String, f32)> = HashMap::new(); // key → (mat_label, dia_label, ft)
    let mut comp_counts: HashMap<String, usize> = HashMap::new();

    for r in 0..app.canvas.grid.height {
        for c in 0..app.canvas.grid.width {
            if let Some(comp) = app.canvas.grid.get(r, c) {
                match comp.kind {
                    ComponentKind::PipeH | ComponentKind::PipeV => {
                        let key = CostConfig::pipe_key(comp.material, comp.diameter);
                        let e = pipe_ft.entry(key).or_insert_with(|| {
                            (comp.material.label().to_string(), comp.diameter.label().to_string(), 0.0)
                        });
                        e.2 += comp.pipe_length;
                    }
                    kind => {
                        if let Some(gkey) = fitting_group_key(kind) {
                            *comp_counts.entry(gkey.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }

    let mut grand_total = 0.0f32;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::raw("")));

    if !pipe_ft.is_empty() {
        lines.push(Line::from(Span::styled("  PIPE", h1)));
        let mut pipe_entries: Vec<_> = pipe_ft.iter().collect();
        pipe_entries.sort_by_key(|(k, _)| (*k).clone());
        for (cfg_key, (mat_label, dia_label, ft)) in &pipe_entries {
            let price_per_ft = *app.config.costs.pipe_per_ft.get(*cfg_key).unwrap_or(&0.0);
            let line_cost = ft * price_per_ft;
            grand_total += line_cost;

            lines.push(Line::from(vec![
                Span::styled(format!("  {} {}", mat_label, dia_label), body),
                Span::styled(format!("  {:.1}ft", ft), dim),
                Span::styled(format!("  ${:.2}", line_cost), money),
            ]));
        }
        lines.push(Line::from(Span::raw("")));
    }

    if !comp_counts.is_empty() {
        lines.push(Line::from(Span::styled("  FITTINGS", h1)));
        for &(key, label, _) in FITTING_GROUPS {
            if let Some(&count) = comp_counts.get(key) {
                let unit_price = *app.config.costs.fitting_per_unit.get(key).unwrap_or(&0.0);
                let line_cost = count as f32 * unit_price;
                grand_total += line_cost;

                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<20}", label), body),
                    Span::styled(format!(" {:>2}×", count), dim),
                    Span::styled(format!("  ${:.2}", line_cost), money),
                ]));
            }
        }
        lines.push(Line::from(Span::raw("")));
    }

    if pipe_ft.is_empty() && comp_counts.is_empty() {
        lines.push(Line::from(Span::styled("  (no components on canvas)", dim)));
        lines.push(Line::from(Span::raw("")));
    }

    let sep = "─".repeat(inner.width.saturating_sub(4) as usize);
    lines.push(Line::from(Span::styled(format!("  {}", sep), dim)));
    lines.push(Line::from(vec![
        Span::styled("  TOTAL  ", tot),
        Span::styled(format!("${:.2}", grand_total), tot),
    ]));

    let visible_h = inner.height as usize;
    let display: Vec<Line> = lines.into_iter().take(visible_h).collect();
    f.render_widget(Paragraph::new(display), inner);
}

/// Returns the FITTING_GROUPS canonical key for a component kind, or None if not tracked.
fn fitting_group_key(kind: ComponentKind) -> Option<&'static str> {
    for &(key, _, members) in FITTING_GROUPS {
        if members.contains(&kind) {
            return Some(key);
        }
    }
    None
}
