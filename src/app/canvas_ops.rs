use super::*;

// ── Smart orientation ─────────────────────────────────────────────────────────
//
// When placing a tee, elbow, or H/V component on or next to existing pipes,
// automatically pick the orientation that fits the live neighbor connections.

pub(super) fn smart_orient(selected: ComponentKind, r: usize, c: usize, grid: &Grid) -> ComponentKind {
    use ComponentKind::*;

    let is_tee         = matches!(selected, TeeNSE | TeeNSW | TeeNEW | TeeSEW);
    let is_reducer_tee = matches!(selected, ReducerTeeNSE | ReducerTeeNSW | ReducerTeeNEW | ReducerTeeSEW);
    let is_elbow       = matches!(selected, ElbowNE | ElbowNW | ElbowSE | ElbowSW);

    // Pipes and valves have explicit H/V palette entries — respect the user's
    // choice and never auto-flip them.
    if !is_tee && !is_reducer_tee && !is_elbow {
        return selected;
    }

    // Detect which directions a neighbor is offering a port toward this cell.
    let height = grid.height;
    let width  = grid.width;

    let n = r > 0          && grid.get(r - 1, c).map(|co| co.connections().1).unwrap_or(false);
    let s = r + 1 < height && grid.get(r + 1, c).map(|co| co.connections().0).unwrap_or(false);
    let e = c + 1 < width  && grid.get(r, c + 1).map(|co| co.connections().3).unwrap_or(false);
    let w = c > 0          && grid.get(r, c - 1).map(|co| co.connections().2).unwrap_or(false);

    let conn_count = [n, s, e, w].iter().filter(|&&x| x).count();

    if is_tee && conn_count == 3 {
        return match (n, s, e, w) {
            (true, true, true, _) => TeeNSE,
            (true, true, _, true) => TeeNSW,
            (true, _, true, true) => TeeNEW,
            (_, true, true, true) => TeeSEW,
            _ => selected,
        };
    }

    if is_reducer_tee && conn_count == 3 {
        return match (n, s, e, w) {
            (true, true, true, _) => ReducerTeeNSE,
            (true, true, _, true) => ReducerTeeNSW,
            (true, _, true, true) => ReducerTeeNEW,
            (_, true, true, true) => ReducerTeeSEW,
            _ => selected,
        };
    }

    if is_elbow && conn_count == 2 {
        return match (n, s, e, w) {
            (true, _, true, _) => ElbowNE,
            (true, _, _, true) => ElbowNW,
            (_, true, true, _) => ElbowSE,
            (_, true, _, true) => ElbowSW,
            _ => selected,
        };
    }

    selected
}

// ── impl App ──────────────────────────────────────────────────────────────────

impl App {
    pub fn move_cursor(&mut self, dr: isize, dc: isize, viewport_h: usize, viewport_w: usize) {
        let (r, c) = self.canvas.cursor;
        let new_r = (r as isize + dr).max(0) as usize;
        let new_c = (c as isize + dc).max(0) as usize;
        const GROW: usize = 20;
        if new_r >= self.canvas.grid.height {
            self.canvas.grid.ensure_size(self.canvas.grid.width, new_r + GROW);
        }
        if new_c >= self.canvas.grid.width {
            self.canvas.grid.ensure_size(new_c + GROW, self.canvas.grid.height);
        }
        self.canvas.cursor = (new_r, new_c);
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    pub(super) fn scroll_viewport_to_cursor(&mut self, viewport_h: usize, viewport_w: usize) {
        let (r, c) = self.canvas.cursor;
        let (vr, vc) = self.canvas.viewport;
        let new_vr = if r < vr {
            r
        } else if r >= vr + viewport_h {
            r + 1 - viewport_h
        } else {
            vr
        };
        let new_vc = if c < vc {
            c
        } else if c >= vc + viewport_w {
            c + 1 - viewport_w
        } else {
            vc
        };
        self.canvas.viewport = (new_vr, new_vc);
    }

    /// True if a composite footprint (fw×fh, port at port_row) can be placed at anchor.
    /// Cells already owned by the existing anchor component are counted as free.
    fn composite_fits_at_fp(&self, fw: usize, fh: usize, pr: usize, anchor_r: usize, anchor_c: usize) -> bool {
        if anchor_r < pr { return false; }
        let top_r = anchor_r - pr;
        if top_r + fh > self.canvas.grid.height { return false; }
        if anchor_c + fw > self.canvas.grid.width { return false; }
        for dr in 0..fh {
            let r = top_r + dr;
            for dc in 0..fw {
                let c = anchor_c + dc;
                if dr == pr && dc == 0 { continue; } // anchor cell — always replaceable
                if let Some(sat_owner) = self.canvas.grid.satellite_anchor(r, c) {
                    if sat_owner != (anchor_r, anchor_c) { return false; }
                } else if self.canvas.grid.get(r, c).is_some() {
                    return false;
                }
            }
        }
        true
    }

    pub fn place_component(&mut self) {
        // Annotations are placed via begin_label/note_placement, not here.
        if self.selected_component_kind().is_annotation() { return; }
        // Redirect satellite cursor to its anchor (allows replacing a composite)
        let (r, c) = self.canvas.grid.effective_pos(self.canvas.cursor.0, self.canvas.cursor.1);
        let kind = smart_orient(self.selected_component_kind(), r, c, &self.canvas.grid);
        let old_kind = self.canvas.grid.get(r, c).map(|co| co.kind);

        // Resolve custom def (if any) up front.
        #[allow(clippy::type_complexity)]
        let custom_info: Option<(String, [bool; 4], Option<(usize, usize)>, String)> =
            if kind == ComponentKind::Custom {
                if let Some(ci) = self.selected_custom_idx() {
                    let customs = self.glyph_registry.custom_components();
                    if ci < customs.len() {
                        let def = &customs[ci];
                        let fp = def.composite_size;
                        Some((def.id.clone(), def.connections_nsew, fp, def.label.clone()))
                    } else { None }
                } else { None }
            } else { None };

        let is_composite = kind.is_composite()
            || custom_info.as_ref().map(|(_, _, fp, _)| fp.is_some()).unwrap_or(false);

        if is_composite {
            let (fw, fh, pr) = if let Some((_, _, Some(fp), _)) = &custom_info {
                // composite_size = canvas dims directly (no buffer ring)
                (fp.0, fp.1, fp.1 / 2)
            } else {
                let fp = kind.footprint();
                (fp.0, fp.1, kind.port_row())
            };
            if !self.composite_fits_at_fp(fw, fh, pr, r, c) {
                self.status_msg = "Cannot place: not enough space for this component.".into();
                return;
            }
            self.push_undo();
            self.canvas.grid.clear_at(r, c);
            let mut comp = Component::new(kind, self.pal.selected_diameter, self.pal.selected_material);
            if let Some((id, conns, fp, label)) = custom_info {
                comp.custom_id = Some(id);
                comp.custom_connections = Some(conns);
                comp.custom_footprint = fp;
                comp.custom_label = Some(label);
            }
            if kind.supports_color_override() {
                comp.color_override = Some(self.selected_build_color());
            }
            self.canvas.grid.place_composite(r, c, comp);
        } else {
            self.push_undo();
            self.canvas.grid.clear_at(r, c);
            let mut comp = Component::new(kind, self.pal.selected_diameter, self.pal.selected_material);
            if matches!(comp.kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let scale_default = self.config.grid_scale_inches as f32 / 12.0;
                comp.pipe_length = self.pal.default_lengths.get(&comp.kind).copied().unwrap_or(scale_default);
            } else if let Some(&defaults) = self.pal.default_arm_lengths.get(&comp.kind) {
                comp.arm_lengths = defaults;
            }
            if let Some((id, conns, _, label)) = custom_info {
                comp.custom_id = Some(id);
                comp.custom_connections = Some(conns);
                comp.custom_label = Some(label);
            }
            if kind.supports_color_override() {
                comp.color_override = Some(self.selected_build_color());
            }
            self.canvas.grid.set(r, c, Some(comp));
        }

        self.status_msg = match old_kind {
            Some(ok) if ok != kind => format!("Replaced {} → {}", ok.label(), kind.label()),
            Some(_)                => format!("Replaced with {}", kind.label()),
            None                   => format!("Placed {}", kind.label()),
        };
        self.refresh_sim();
    }

    pub fn delete_component(&mut self) {
        let (r, c) = self.canvas.grid.effective_pos(self.canvas.cursor.0, self.canvas.cursor.1);
        if self.canvas.grid.get(r, c).is_none() { return; }
        self.push_undo();
        self.canvas.grid.clear_at(r, c);
        self.refresh_sim();
    }

    pub fn toggle_valve_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        if self.canvas.grid.get(r, c).map(|co| co.kind.is_valve()).unwrap_or(false) {
            self.push_undo();
        }
        if let Some(comp) = self.canvas.grid.get_mut(r, c) {
            comp.toggle_valve();
            let open = comp.valve_state == Some(crate::components::ValveState::Open);
            self.status_msg = if open { "Valve opened." } else { "Valve closed." }.into();
            self.refresh_sim();
        }
    }

    pub fn cycle_material_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        let (ar, ac) = self.canvas.grid.effective_pos(r, c);
        if self.canvas.grid.get(ar, ac).is_some() { self.push_undo(); }
        if let Some(comp) = self.canvas.grid.get_mut(ar, ac) {
            comp.material = comp.material.cycle();
            self.pal.selected_material = comp.material;
            self.status_msg = format!("Material: {}", comp.material.label());
            self.refresh_sim();
        } else {
            self.pal.selected_material = self.pal.selected_material.cycle();
            self.status_msg = format!("Default material: {}", self.pal.selected_material.label());
        }
    }

    pub fn adjust_length_at_cursor(&mut self, delta_in: f32) {
        let (r, c) = self.canvas.cursor;
        if self.canvas.grid.get(r, c).is_some() { self.push_undo(); }
        if let Some(comp) = self.canvas.grid.get_mut(r, c) {
            let new_in = (comp.pipe_length * 12.0 + delta_in).max(1.0);
            comp.pipe_length = new_in / 12.0;
            self.status_msg = format!(
                "Pipe length: {} in ({:.2} ft)",
                new_in.round() as i32,
                comp.pipe_length
            );
            self.refresh_sim();
        }
    }

    pub fn begin_length_edit(&mut self) {
        let (r, c) = self.canvas.cursor;
        if let Some(comp) = self.canvas.grid.get(r, c) {
            if matches!(comp.kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let inches = (comp.pipe_length * 12.0).round() as i32;
                self.text_input.input_buffer = inches.to_string();
                self.text_input.input_mode = InputMode::EditingLength;
            } else if comp.kind.has_arm_stubs() {
                self.enter_component_detail();
            }
            return;
        }
        // No component at cursor — edit the default length for the selected pipe kind.
        let kind = self.selected_component_kind();
        if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
            let inches = (self.pal.default_lengths.get(&kind).copied().unwrap_or(1.0) * 12.0).round() as i32;
            self.text_input.input_buffer = inches.to_string();
            self.text_input.input_mode = InputMode::EditingLength;
            self.status_msg = format!("Enter default {} length (inches):", kind.label());
        } else {
            self.status_msg = "Select PipeH or PipeV in palette to set default length.".into();
        }
    }

    pub fn cycle_line_temp_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        let (ar, ac) = self.canvas.grid.effective_pos(r, c);
        if self.canvas.grid.get(ar, ac).is_some() {
            self.push_undo();
            if let Some(comp) = self.canvas.grid.get_mut(ar, ac) {
                comp.line_temp = comp.line_temp.cycle();
                let label = comp.line_temp.label();
                self.status_msg = if label.is_empty() {
                    "Line temp: unset".into()
                } else {
                    format!("Line temp: {label}")
                };
            }
        }
    }

    pub fn cycle_drain_type_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        let has_drain = self.canvas.grid.get(r, c)
            .map(|co| matches!(co.kind, ComponentKind::Sink | ComponentKind::Toilet
                | ComponentKind::Faucet | ComponentKind::BasinSink))
            .unwrap_or(false);
        if has_drain { self.push_undo(); }
        if let Some(comp) = self.canvas.grid.get_mut(r, c) {
            if matches!(comp.kind, ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet | ComponentKind::BasinSink) {
                comp.drain_type = comp.drain_type.cycle();
                self.status_msg = format!("Fixture type: {}", comp.drain_type.label());
                self.refresh_sim();
            }
        }
    }

    pub fn adjust_source_pressure_at_cursor(&mut self, delta: f32) {
        let (r, c) = self.canvas.cursor;
        let kind = self.canvas.grid.get(r, c).map(|co| co.kind);
        if matches!(kind, Some(ComponentKind::Source) | Some(ComponentKind::PressureReducingValve)) {
            self.push_undo();
        }
        if let Some(comp) = self.canvas.grid.get_mut(r, c) {
            match comp.kind {
                ComponentKind::Source => {
                    comp.source_pressure_psi = (comp.source_pressure_psi + delta).clamp(10.0, 200.0);
                    self.status_msg = format!("Inlet pressure: {:.0} PSI", comp.source_pressure_psi);
                    self.refresh_sim();
                }
                ComponentKind::PressureReducingValve => {
                    comp.prv_setpoint_psi = (comp.prv_setpoint_psi + delta).clamp(10.0, 200.0);
                    self.status_msg = format!("PRV setpoint: {:.0} PSI", comp.prv_setpoint_psi);
                    self.refresh_sim();
                }
                _ => {}
            }
        }
    }

    pub fn begin_source_pressure_dialog(&mut self) {
        let (r, c) = self.canvas.cursor;
        let Some(comp) = self.canvas.grid.get(r, c) else { return };
        let (psi, is_prv) = match comp.kind {
            ComponentKind::Source => (comp.source_pressure_psi, false),
            ComponentKind::PressureReducingValve => (comp.prv_setpoint_psi, true),
            _ => return,
        };
        self.text_input.input_buffer = format!("{:.1}", psi);
        self.text_input.note_cursor_pos = self.text_input.input_buffer.len();
        self.text_input.note_scroll_col = 0;
        self.text_input.input_mode = InputMode::EditingText(
            if is_prv { TextEditTarget::PrvSetpoint } else { TextEditTarget::SourcePressure }
        );
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn toggle_annotations(&mut self) {
        self.show_annotations = !self.show_annotations;
        self.status_msg = if self.show_annotations {
            "Annotations ON".into()
        } else {
            "Annotations OFF".into()
        };
    }

    pub fn toggle_dwv_mode(&mut self) {
        self.dwv_mode = !self.dwv_mode;
        if self.dwv_mode {
            self.refresh_dwv();
            self.status_msg = "DWV mode ON — showing drain-waste-vent components. [W] to exit.".into();
        } else {
            self.dwv_result = None;
            self.status_msg = "DWV mode OFF.".into();
        }
    }

    pub fn refresh_dwv(&mut self) {
        if self.dwv_mode {
            self.dwv_result = Some(validate_dwv(&self.canvas.grid));
        }
    }

    /// Cycle drain diameter on DWV component at cursor.
    pub fn cycle_drain_diameter_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        let is_dwv = self.canvas.grid.get(r, c).map(|co| co.kind.is_dwv()).unwrap_or(false);
        if is_dwv {
            self.push_undo();
            if let Some(comp) = self.canvas.grid.get_mut(r, c) {
                comp.drain_diameter = comp.drain_diameter.cycle();
            }
            self.refresh_dwv();
        }
    }
}
