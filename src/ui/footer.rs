use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, AppMode, InputMode, TextEditTarget};
use crate::components::{ComponentKind, LineTemp, ValveState};
use crate::simulation::FlowState;

use super::key;

pub(super) fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 60)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(comp) = app.component_at_cursor() {
        // Line 1 – type, diameter, material, valve state
        let valve_tag = match comp.valve_state {
            Some(ValveState::Open)   => "  VALVE:OPEN",
            Some(ValveState::Closed) => "  VALVE:CLOSED",
            None => "",
        };
        let [mr, mg, mb] = app
            .glyph_registry
            .resolve(comp.kind, comp.material, comp.diameter)
            .fg;
        let (temp_tag, temp_color) = match comp.line_temp {
            LineTemp::Cold   => ("  ❄ COLD",   Color::Rgb(80, 160, 255)),
            LineTemp::Hot    => ("  🔥 HOT",   Color::Rgb(255, 90, 70)),
            LineTemp::Recirc => ("  ↺ RECIRC", Color::Rgb(255, 165, 40)),
            LineTemp::Unset  => ("", Color::DarkGray),
        };
        let mut row1 = vec![
            Span::styled(
                comp.kind.label(),
                Style::default().fg(Color::Rgb(mr, mg, mb)).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}  {}{}", comp.diameter.label(), comp.material.label(), valve_tag),
                Style::default().fg(Color::White),
            ),
        ];
        if !temp_tag.is_empty() {
            row1.push(Span::styled(temp_tag, Style::default().fg(temp_color).add_modifier(Modifier::BOLD)));
        }
        lines.push(Line::from(row1));

        // Line 2 – properties / length edit overlay
        let prop_line = match comp.kind {
            ComponentKind::Source => format!(
                "Inlet pressure: {:.1} PSI   [i] +1  [I] -1  [P] enter exact",
                comp.source_pressure_psi
            ),
            ComponentKind::PressureReducingValve => format!(
                "PRV setpoint: {:.1} PSI   [i] +1  [I] -1  [P] enter exact",
                comp.prv_setpoint_psi
            ),
            ComponentKind::ExpansionTank => "Expansion tank — dead-end branch. No setpoint needed.".to_string(),
            ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet
            | ComponentKind::BasinSink => format!(
                "Fixture: {}   [T] cycle type",
                comp.drain_type.label()
            ),
            ComponentKind::SolidBlock => "Structural element — no plumbing connections.".to_string(),
            ComponentKind::Link => {
                let path = comp.text.as_deref().unwrap_or("(no path set)");
                format!("⇒ {path}   [Enter] follow  [E] edit path")
            }
            ComponentKind::PipeH | ComponentKind::PipeV => {
                let in_total = comp.pipe_length * 12.0;
                let whole_in = in_total.floor() as i32;
                format!(
                    "Length: {} in ({:.2} ft)   [+/-] 1 in  [Shift] 6 in  [L] manual entry",
                    whole_in, comp.pipe_length
                )
            }
            ComponentKind::DrainH | ComponentKind::DrainV => {
                let in_total = comp.pipe_length * 12.0;
                let whole_in = in_total.floor() as i32;
                format!(
                    "DWV drain {}  Length: {} in ({:.2} ft)   [+/-] 1 in  [D] diameter",
                    comp.drain_diameter.label(), whole_in, comp.pipe_length
                )
            }
            ComponentKind::PTrap | ComponentKind::Vent | ComponentKind::DrainWye | ComponentKind::Cleanout => {
                format!("DWV {} — diameter {}   [D] cycle", comp.kind.label(), comp.drain_diameter.label())
            }
            _ => format!(
                "Equiv. length: {:.1} in ({:.2} ft)   [M] material   [D] diameter",
                comp.equiv_length_ft() * 12.0,
                comp.equiv_length_ft()
            ),
        };

        if app.text_input.input_mode == InputMode::EditingLength {
            let preview_in = app.text_input.input_buffer.parse::<f32>().unwrap_or(0.0);
            lines.push(Line::from(vec![
                Span::styled(
                    "Length (in): ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}|", app.text_input.input_buffer),
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Rgb(40, 40, 80))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  = {:.2} ft   [Enter] confirm  [Esc] cancel", preview_in / 12.0),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(prop_line, Style::default().fg(Color::Gray))));
        }

        // Line 3 – flow data or description
        if let Some(fd) = app.flow_data_at_cursor() {
            let flow_state_label = app
                .flow_state_at_cursor()
                .map(|fs| match fs {
                    FlowState::Flowing     => "FLOWING",
                    FlowState::Pressurized => "PRESSURIZED",
                    FlowState::Static      => "STATIC",
                })
                .unwrap_or("--");

            if comp.kind == ComponentKind::PressureGauge {
                lines.push(Line::from(vec![
                    Span::styled("⊙ GAUGE  ", Style::default().fg(Color::Rgb(220, 200, 60)).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{:.1} PSI", fd.pressure_psi),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  [{flow_state_label}]"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            } else if matches!(comp.kind, ComponentKind::FlowMeterH | ComponentKind::FlowMeterV) {
                lines.push(Line::from(vec![
                    Span::styled("⊗ METER  ", Style::default().fg(Color::Rgb(60, 200, 180)).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{:.2} GPM", fd.flow_gpm),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("   {:.1} PSI", fd.pressure_psi),
                        Style::default().fg(Color::Rgb(160, 175, 185)),
                    ),
                    Span::styled(
                        format!("  [{flow_state_label}]"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            } else {
                let vel_limit = comp.material.max_velocity_fps();
                let vel_exceeded = fd.velocity_fps > vel_limit;
                let vel_color = if vel_exceeded { Color::Red } else { Color::Gray };
                let vel_suffix = if vel_exceeded { " !" } else { "" };

                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[{}]  ", flow_state_label),
                        Style::default()
                            .fg(match flow_state_label {
                                "FLOWING"     => Color::LightCyan,
                                "PRESSURIZED" => Color::Yellow,
                                _             => Color::DarkGray,
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("Flow: {:.2} GPM  ", fd.flow_gpm), Style::default().fg(Color::White)),
                    Span::styled(format!("Pressure: {:.1} PSI  ", fd.pressure_psi), Style::default().fg(Color::White)),
                    Span::styled(
                        format!("Velocity: {:.2} ft/s{vel_suffix}", fd.velocity_fps),
                        Style::default().fg(vel_color),
                    ),
                ]));
            }
        } else {
            lines.push(Line::from(Span::styled(
                comp.kind.description(),
                Style::default().fg(Color::Rgb(100, 100, 100)),
            )));
        }
    } else {
        let sel = app.selected_component_kind();
        lines.push(Line::from(Span::styled(
            format!(
                "Ready: {}  {}  {}",
                sel.label(),
                app.pal.selected_diameter.label(),
                app.pal.selected_material.label()
            ),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            sel.description(),
            Style::default().fg(Color::Rgb(70, 70, 70)),
        )));
        if app.text_input.input_mode == InputMode::EditingLength {
            let preview_in = app.text_input.input_buffer.parse::<f32>().unwrap_or(0.0);
            lines.push(Line::from(vec![
                Span::styled(
                    "Length (in): ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}|", app.text_input.input_buffer),
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Rgb(40, 40, 80))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  = {:.2} ft   [Enter] confirm  [Esc] cancel", preview_in / 12.0),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        } else {
            let kind = app.selected_component_kind();
            if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let def_ft = app.pal.default_lengths.get(&kind).copied().unwrap_or(1.0);
                let def_in = (def_ft * 12.0).round() as i32;
                lines.push(Line::from(vec![
                    Span::styled(format!("{} default: ", kind.label()), Style::default().fg(Color::Rgb(70, 70, 70))),
                    Span::styled(
                        format!("{} in ({:.2} ft)  [+/-] [L]", def_in, def_ft),
                        Style::default().fg(Color::Rgb(120, 120, 120)),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    "Select PipeH/PipeV in palette to set default length",
                    Style::default().fg(Color::Rgb(70, 70, 70)),
                )));
            }
        }
    }

    // Warning / status line
    let warn = if app.dwv_mode {
        if let Some(dwv) = &app.dwv_result {
            if dwv.warnings.is_empty() && dwv.total_dfu > 0 {
                let ok_trap = if dwv.all_trapped { "✓ All fixtures trapped" } else { "✗ P-trap missing" };
                let ok_vent = if dwv.has_vent { "✓ Vent present" } else { "✗ No vent!" };
                Line::from(vec![
                    Span::styled("DWV ", Style::default().fg(Color::Rgb(160, 110, 60)).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{total_dfu} DFU  ", total_dfu=dwv.total_dfu), Style::default().fg(Color::White)),
                    Span::styled(ok_trap, Style::default().fg(if dwv.all_trapped { Color::LightGreen } else { Color::Red })),
                    Span::styled("  ", Style::default()),
                    Span::styled(ok_vent, Style::default().fg(if dwv.has_vent { Color::LightGreen } else { Color::Yellow })),
                ])
            } else if !dwv.warnings.is_empty() {
                Line::from(vec![
                    Span::styled("DWV! ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(dwv.warnings[0].as_str(), Style::default().fg(Color::Yellow)),
                ])
            } else if dwv.total_dfu == 0 {
                Line::from(Span::styled("DWV mode active — place drain pipes, P-traps, and vents. [W] exit.", Style::default().fg(Color::Rgb(120, 90, 50))))
            } else {
                Line::from(Span::styled("DWV OK", Style::default().fg(Color::LightGreen)))
            }
        } else {
            Line::from(Span::styled("DWV mode active. [W] exit.", Style::default().fg(Color::Rgb(120, 90, 50))))
        }
    } else if let Some(sim) = &app.sim.sim_result {
        if sim.warnings.is_empty() {
            if sim.reached_sink {
                Line::from(Span::styled(
                    "OK  Flow reaches all connected drains.",
                    Style::default().fg(Color::LightGreen),
                ))
            } else {
                Line::from(Span::styled("--", Style::default().fg(Color::DarkGray)))
            }
        } else {
            Line::from(Span::styled(
                format!("! {}", sim.warnings[0]),
                Style::default().fg(Color::Yellow),
            ))
        }
    } else if !app.status_msg.is_empty() {
        Line::from(Span::styled(app.status_msg.as_str(), Style::default().fg(Color::Gray)))
    } else {
        Line::from(Span::styled(
            "Press [P] to run simulation.",
            Style::default().fg(Color::Rgb(70, 70, 70)),
        ))
    };

    if app.text_input.pending_annotation.is_some() {
        let (label, color) = if matches!(app.text_input.pending_annotation, Some((ComponentKind::Note, _))) {
            ("Note", Color::Rgb(80, 220, 230))
        } else {
            ("Label", Color::Rgb(255, 230, 60))
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{label} placement: "), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled("move cursor to target position, ", Style::default().fg(Color::White)),
            Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" place  "),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]));
    } else if let InputMode::EditingText(target) = app.text_input.input_mode {
        let prompt = match target {
            TextEditTarget::AssemblyName  => "Assembly name: ",
            TextEditTarget::AddGlyphFile  => "Glyph file path: ",
            TextEditTarget::CustomRgb     => "Custom RGB (R,G,B): ",
            TextEditTarget::BuildCustomRgb => "Custom color R,G,B: ",
            _                             => "",
        };
        if !prompt.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(prompt, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{}|", app.text_input.input_buffer),
                    Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 80)).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  [Enter] confirm  [Esc] cancel", Style::default().fg(Color::Gray)),
            ]));
        } else {
            lines.push(warn);
        }
    } else {
        let u = app.undo.undo_count();
        let r = app.undo.redo_count();
        let undo_fg = if u > 0 { Color::Rgb(80, 140, 230) } else { Color::Rgb(45, 45, 65) };
        let redo_fg = if r > 0 { Color::Rgb(70, 200, 130) } else { Color::Rgb(45, 45, 65) };
        let mut w = warn;
        w.spans.push(Span::styled("   ↩ ", Style::default().fg(undo_fg)));
        w.spans.push(Span::styled(
            u.to_string(),
            Style::default().fg(undo_fg).add_modifier(Modifier::BOLD),
        ));
        w.spans.push(Span::styled("  ↪ ", Style::default().fg(redo_fg)));
        w.spans.push(Span::styled(
            r.to_string(),
            Style::default().fg(redo_fg).add_modifier(Modifier::BOLD),
        ));
        lines.push(w);
    }

    // Key bindings — two rows, context-sensitive
    match app.mode {
        AppMode::Splash | AppMode::Build | AppMode::GlyphEditor | AppMode::BomView
        | AppMode::Selecting | AppMode::AssemblyBrowser | AppMode::Stamping => {
            lines.push(Line::from(vec![
                key("[Enter]"), Span::raw("Place "),
                key("[Del]"), Span::raw("Delete "),
                key("[V]"), Span::raw("Valve "),
                key("[1-6]"), Span::raw("Material "),
                key("[D]"), Span::raw("Diameter "),
                key("[F]"), Span::raw("Fluid "),
                key("[+/-]"), Span::raw("Len±1in "),
                key("[L]"), Span::raw("Len=? "),
                key("[T]"), Span::raw("Drain "),
                key("[I]"), Span::raw("Pressure "),
                key("[H]"), Span::raw("Hot/Cold "),
                key("[G]"), Span::raw("Glyphs"),
            ]));
            lines.push(Line::from(vec![
                key("[Tab]"), Span::raw("Focus "),
                key("[Home/End]"), Span::raw("Jump "),
                key("[^S]"), Span::raw("Save "),
                key("[^O]"), Span::raw("Load "),
                key("[X]"), Span::raw("Export "),
                key("[^Z]"), Span::raw("Undo "),
                key("[^Y]"), Span::raw("Redo "),
                key("[N]"), Span::raw("New "),
                key("[A]"), Span::raw("Ann "),
                key("[B]"), Span::raw("BOM "),
                key("[$]"), Span::raw("Cost "),
                key("[W]"), Span::raw("DWV "),
                key("[R]"), Span::raw("Select "),
                key("[Y]"), Span::raw("Assem "),
                key("[C]"), Span::raw("Settings "),
                key("[P]"), Span::styled("Sim  ", Style::default().fg(Color::LightGreen)),
                key("[?]"), Span::raw("Help "),
                key("[Q]"), Span::styled("Quit", Style::default().fg(Color::Red)),
            ]));
        }
        AppMode::Settings => {
            lines.push(Line::from(vec![
                key("[↑↓]"), Span::raw("Select "),
                key("[A]"), Span::raw("Add file "),
                key("[D/Del]"), Span::raw("Remove "),
                key("[L]"), Span::raw("Load now "),
            ]));
            lines.push(Line::from(vec![
                key("[C]"), Span::raw(" / "),
                key("[Q]"), Span::raw(" / "),
                key("[Esc]"), Span::styled("  Close Settings", Style::default().fg(Color::Red)),
            ]));
        }
        AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit => {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::raw("")));
        }
        AppMode::ExportDialog => {
            lines.push(Line::from(vec![
                key("[T]"), Span::raw("Text (.txt)  "),
                key("[J]"), Span::raw("JSON (.json)  "),
                key("[Esc]"), Span::styled("  Cancel", Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(Span::raw("")));
        }
        AppMode::AnnotationDialog => {
            lines.push(Line::from(vec![
                key("[Enter]"), Span::raw("Confirm  "),
                key("[Esc]"), Span::styled("  Cancel", Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(Span::raw("")));
        }
        AppMode::Simulating => {
            lines.push(Line::from(vec![
                key("[↑↓←→]"), Span::raw("Move "),
                key("[V]"), Span::raw("Valve "),
                key("[F]"), Span::raw("Fluid "),
                key("[I]"), Span::raw("Pressure±10 "),
                key("[G]"), Span::raw("Glyphs "),
                key("[^S]"), Span::raw("Save "),
                key("[^O]"), Span::raw("Load "),
                key("[A]"), Span::raw("Ann "),
                key("[B]"), Span::raw("BOM "),
                key("[Tab]"), Span::raw("Focus"),
            ]));
            lines.push(Line::from(vec![
                key("[Spc]"), Span::styled("Pause  ", Style::default().fg(Color::Yellow)),
                key("[S]"), Span::styled("Stop  ", Style::default().fg(Color::Red)),
                key("[P]"), Span::styled("Restart Sim  ", Style::default().fg(Color::LightGreen)),
                key("[?]"), Span::raw("Help "),
                key("[Q]"), Span::styled("Quit", Style::default().fg(Color::Red)),
            ]));
        }
        AppMode::Paused => {
            lines.push(Line::from(vec![
                key("[↑↓←→]"), Span::raw("Move "),
                key("[V]"), Span::raw("Valve "),
                key("[F]"), Span::raw("Fluid "),
                key("[I]"), Span::raw("Pressure±10 "),
                key("[G]"), Span::raw("Glyphs "),
                key("[^S]"), Span::raw("Save "),
                key("[^O]"), Span::raw("Load "),
                key("[A]"), Span::raw("Ann "),
                key("[B]"), Span::raw("BOM "),
                key("[Tab]"), Span::raw("Focus"),
            ]));
            lines.push(Line::from(vec![
                key("[Spc]"), Span::styled("Resume  ", Style::default().fg(Color::LightGreen)),
                key("[S]"), Span::styled("Stop  ", Style::default().fg(Color::Red)),
                key("[P]"), Span::styled("Restart Sim  ", Style::default().fg(Color::LightGreen)),
                key("[?]"), Span::raw("Help "),
                key("[Q]"), Span::styled("Quit", Style::default().fg(Color::Red)),
            ]));
        }
        AppMode::ComponentDetail => {
            lines.push(Line::from(vec![
                key("[↑↓]"), Span::raw("Select port   "),
                key("[Enter]"), Span::raw("Edit stub length   "),
                key("[Esc/Q]"), Span::styled("Close", Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(Span::raw("")));
        }
        AppMode::Help => {
            lines.push(Line::from(vec![
                key("[↑↓]"), Span::raw("Scroll   "),
                key("[PgUp/PgDn]"), Span::raw("Page   "),
                key("[Home/End]"), Span::raw("Top / Bottom   "),
                key("[?]"), Span::raw(" / "),
                key("[Esc]"), Span::styled("  Close Help", Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(Span::raw("")));
        }
        AppMode::CostEstimator => {
            lines.push(Line::from(vec![
                key("[↑↓]"), Span::raw("Navigate "),
                key("[Enter/E]"), Span::raw("Edit price "),
                key("[$]"), Span::raw(" / "),
                key("[Q]"), Span::raw(" / "),
                key("[Esc]"), Span::styled("  Close", Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(Span::raw("")));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);

    // Coffee link — bottom-right corner of footer
    use ratatui::layout::Alignment;
    if inner.height >= 1 {
        let link_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
        f.render_widget(
            Paragraph::new(Span::styled(
                "Did this help? https://buymeacoffee.com/sormondocom",
                Style::default().fg(Color::Rgb(210, 140, 40)),
            ))
            .alignment(Alignment::Right),
            link_area,
        );
    }
}
