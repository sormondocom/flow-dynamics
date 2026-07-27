use super::*;

impl App {
    // ── Assembly: selection ──────────────────────────────────────────────────

    pub fn start_selecting(&mut self) {
        self.selection.select_start = Some(self.canvas.cursor);
        self.mode = AppMode::Selecting;
        self.status_msg = "Selection: arrows resize rect  [C] copy  [X] move  [Enter]/[R] save assembly  [Esc] cancel".into();
    }

    pub fn confirm_selection(&mut self) {
        if self.selection.select_start.is_some() {
            self.text_input.input_buffer.clear();
            self.text_input.input_mode = InputMode::EditingText(TextEditTarget::AssemblyName);
            self.status_msg = "Assembly name:".into();
        }
    }

    pub fn cancel_selection(&mut self) {
        self.selection.select_start = None;
        self.mode = AppMode::Build;
        self.status_msg = "Selection cancelled.".into();
    }

    /// Copy the selection rectangle to the stamp clipboard and enter Stamping mode.
    /// Original content is preserved regardless of where the paste lands.
    pub fn copy_selection(&mut self) {
        self.enter_stamp_mode(false);
    }

    /// Cut the selection rectangle: enter Stamping mode and clear the source rect on paste.
    pub fn move_selection(&mut self) {
        self.enter_stamp_mode(true);
    }

    fn enter_stamp_mode(&mut self, is_cut: bool) {
        let Some(start) = self.selection.select_start else { return };
        let end = self.canvas.cursor;
        let r0 = start.0.min(end.0);
        let r1 = start.0.max(end.0);
        let c0 = start.1.min(end.1);
        let c1 = start.1.max(end.1);
        let asm = Assembly::from_selection(
            &self.canvas.grid, r0, c0, r1, c1,
            "clipboard".into(), String::new(),
        );
        let count = asm.component_count();
        let w = c1 - c0 + 1;
        let h = r1 - r0 + 1;
        self.selection.pending_stamp = Some(asm);
        self.selection.stamp_cut_rect = if is_cut { Some((r0, c0, r1, c1)) } else { None };
        self.selection.select_start = None;
        self.canvas.cursor = (r0, c0); // ghost starts aligned over original
        self.mode = AppMode::Stamping;
        let action = if is_cut { "move" } else { "copy" };
        self.status_msg = format!(
            "{}×{} ({} components) to {} — arrows to position, [Enter] paste, [Esc] cancel",
            w, h, count, action
        );
    }

    pub fn save_assembly_named(&mut self, name: String) {
        let Some(start) = self.selection.select_start else { return };
        let end = self.canvas.cursor;
        let r0 = start.0.min(end.0);
        let r1 = start.0.max(end.0);
        let c0 = start.1.min(end.1);
        let c1 = start.1.max(end.1);

        let assembly = Assembly::from_selection(&self.canvas.grid, r0, c0, r1, c1, name, String::new());
        let comp_count = assembly.component_count();
        self.selection.assembly_lib.assemblies.push(assembly);
        self.selection.select_start = None;
        self.mode = AppMode::Build;

        let save_msg = if let Some(path) = &self.selection.assembly_path.clone() {
            match self.selection.assembly_lib.save(path) {
                Ok(()) => format!("Assembly saved ({} components, {}×{}).", comp_count, c1 - c0 + 1, r1 - r0 + 1),
                Err(e) => format!("Assembly saved in memory; write failed: {e}"),
            }
        } else {
            "Assembly saved in memory (no file path).".into()
        };
        self.status_msg = save_msg;
    }

    // ── Assembly: browser ────────────────────────────────────────────────────

    pub fn enter_assembly_browser(&mut self) {
        self.selection.pre_assembly_mode = self.mode;
        self.selection.assembly_idx = self.selection.assembly_idx.min(self.selection.assembly_lib.assemblies.len().saturating_sub(1));
        self.mode = AppMode::AssemblyBrowser;
    }

    pub fn exit_assembly_browser(&mut self) {
        self.mode = self.selection.pre_assembly_mode;
    }

    pub fn assembly_browser_up(&mut self) {
        if self.selection.assembly_idx > 0 {
            self.selection.assembly_idx -= 1;
        }
    }

    pub fn assembly_browser_down(&mut self) {
        if self.selection.assembly_idx + 1 < self.selection.assembly_lib.assemblies.len() {
            self.selection.assembly_idx += 1;
        }
    }

    pub fn delete_assembly(&mut self) {
        let libs = &mut self.selection.assembly_lib.assemblies;
        if self.selection.assembly_idx < libs.len() {
            let name = libs[self.selection.assembly_idx].name.clone();
            libs.remove(self.selection.assembly_idx);
            if self.selection.assembly_idx >= libs.len() && self.selection.assembly_idx > 0 {
                self.selection.assembly_idx -= 1;
            }
            if let Some(path) = &self.selection.assembly_path.clone() {
                let _ = self.selection.assembly_lib.save(path);
            }
            self.status_msg = format!("Deleted assembly '{name}'.");
        }
    }

    // ── Assembly: stamp ──────────────────────────────────────────────────────

    pub fn begin_stamp(&mut self) {
        let idx = self.selection.assembly_idx;
        if idx < self.selection.assembly_lib.assemblies.len() {
            self.selection.pending_stamp = Some(self.selection.assembly_lib.assemblies[idx].clone());
            self.mode = AppMode::Stamping;
            self.status_msg = "Move cursor to top-left corner, then [Enter] to stamp. [Esc] to cancel.".into();
        }
    }

    pub fn confirm_stamp(&mut self) {
        if let Some(asm) = self.selection.pending_stamp.take() {
            let (r, c) = self.canvas.cursor;
            self.push_undo();
            // For moves: clear the source region before stamping so overlapping areas work correctly.
            if let Some((r0, c0, r1, c1)) = self.selection.stamp_cut_rect.take() {
                for gr in r0..=r1 {
                    for gc in c0..=c1 {
                        self.canvas.grid.set(gr, gc, None);
                    }
                }
            }
            asm.stamp_onto(&mut self.canvas.grid, r, c);
            self.canvas.grid.rebuild_satellites();
            self.mode = AppMode::Build;
            self.status_msg = format!("Pasted {}×{} at ({},{}).", asm.width, asm.height, r, c);
            self.refresh_sim();
        }
    }

    pub fn cancel_stamp(&mut self) {
        self.selection.pending_stamp = None;
        self.selection.stamp_cut_rect = None;
        self.mode = AppMode::Build;
        self.status_msg = "Cancelled.".into();
    }

    // ── Component detail ─────────────────────────────────────────────────────

    /// Open detail overlay for the PLACED component at the cursor.
    pub fn enter_component_detail(&mut self) {
        let (r, c) = self.canvas.cursor;
        if let Some(comp) = self.canvas.grid.get(r, c) {
            self.dialog.pre_detail_mode = self.mode;
            self.detail.detail_kind = comp.kind;
            self.detail.detail_for_palette = false;
            self.detail.detail_port_cursor = 0;
            self.mode = AppMode::ComponentDetail;
        }
    }

    /// Open detail overlay to set DEFAULT arm lengths for the currently selected palette kind.
    pub fn enter_palette_component_detail(&mut self) {
        let kind = self.selected_component_kind();
        if !kind.has_arm_stubs() {
            return;
        }
        self.dialog.pre_detail_mode = self.mode;
        self.detail.detail_kind = kind;
        self.detail.detail_for_palette = true;
        self.detail.detail_port_cursor = 0;
        self.mode = AppMode::ComponentDetail;
    }

    pub fn exit_component_detail(&mut self) {
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
        self.mode = self.dialog.pre_detail_mode;
    }

    /// Returns the active ports for whatever is being edited (placed or palette default).
    pub fn detail_active_ports(&self) -> Vec<(usize, &'static str)> {
        if !self.detail.detail_kind.has_arm_stubs() {
            return Vec::new();
        }
        const NAMES: [&str; 4] = ["North", "South", "East", "West"];
        let (n, s, e, w) = if self.detail.detail_for_palette {
            self.detail.detail_kind.connections()
        } else {
            let (r, c) = self.canvas.cursor;
            self.canvas.grid.get(r, c)
                .map(|co| co.connections())
                .unwrap_or((false, false, false, false))
        };
        [n, s, e, w].iter().enumerate()
            .filter(|(_, &v)| v)
            .map(|(i, _)| (i, NAMES[i]))
            .collect()
    }

    /// Returns the current arm lengths for whatever is being edited.
    pub fn detail_arm_lengths(&self) -> [f32; 4] {
        if self.detail.detail_for_palette {
            self.pal.default_arm_lengths.get(&self.detail.detail_kind).copied().unwrap_or([0.0; 4])
        } else {
            let (r, c) = self.canvas.cursor;
            self.canvas.grid.get(r, c).map(|co| co.arm_lengths).unwrap_or([0.0; 4])
        }
    }

    pub fn component_detail_nav(&mut self, delta: isize) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail.detail_port_cursor = (self.detail.detail_port_cursor as isize + delta)
                .rem_euclid(count as isize) as usize;
        }
    }

    pub fn component_detail_page_up(&mut self) {
        self.detail.detail_port_cursor = self.detail.detail_port_cursor.saturating_sub(10);
    }

    pub fn component_detail_page_down(&mut self) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail.detail_port_cursor = (self.detail.detail_port_cursor + 10).min(count - 1);
        }
    }

    pub fn component_detail_home(&mut self) {
        self.detail.detail_port_cursor = 0;
    }

    pub fn component_detail_end(&mut self) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail.detail_port_cursor = count - 1;
        }
    }

    pub fn begin_port_length_edit(&mut self) {
        let ports = self.detail_active_ports();
        if let Some(&(raw_port, dir)) = ports.get(self.detail.detail_port_cursor) {
            let arm_lengths = self.detail_arm_lengths();
            let cur_in = (arm_lengths[raw_port] * 12.0).round() as i32;
            self.text_input.input_buffer = if cur_in > 0 { cur_in.to_string() } else { String::new() };
            self.text_input.input_mode = InputMode::EditingLength;
            self.status_msg = format!("Enter {} stub length (inches):", dir);
        }
    }

    // ── Fluid type ───────────────────────────────────────────────────────────

    pub fn cycle_fluid_type(&mut self) {
        self.sim.fluid_type = self.sim.fluid_type.cycle();
        self.status_msg = format!("Fluid: {}", self.sim.fluid_type.label());
        self.refresh_sim();
    }

    // ── Annotation placement ─────────────────────────────────────────────────

    pub fn begin_label_placement(&mut self) {
        let (r, c) = self.canvas.cursor;
        let is_existing = self.canvas.grid.get(r, c).map(|co| co.kind == ComponentKind::Label).unwrap_or(false);
        let existing = self.canvas.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Label)
            .and_then(|co| co.text.as_deref())
            .unwrap_or("")
            .to_string();
        self.text_input.input_buffer = existing;
        self.text_input.note_cursor_pos = self.text_input.input_buffer.len();
        self.text_input.note_scroll_col = 0;
        self.note_update_scroll();
        self.text_input.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::LabelText);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn begin_note_placement(&mut self) {
        let (r, c) = self.canvas.cursor;
        let is_existing = self.canvas.grid.get(r, c).map(|co| co.kind == ComponentKind::Note).unwrap_or(false);
        let existing = self.canvas.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Note)
            .and_then(|co| co.text.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_default();
        self.text_input.input_buffer = existing;
        self.text_input.note_cursor_pos = self.text_input.input_buffer.len();
        self.text_input.note_scroll_row = 0;
        self.text_input.note_scroll_col = 0;
        self.note_update_scroll();
        self.text_input.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::NoteText);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn begin_link_placement(&mut self) {
        let (r, c) = self.canvas.cursor;
        let is_existing = self.canvas.grid.get(r, c).map(|co| co.kind == ComponentKind::Link).unwrap_or(false);
        let existing = self.canvas.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Link)
            .and_then(|co| co.text.as_deref())
            .unwrap_or("")
            .to_string();
        self.text_input.input_buffer = existing;
        self.text_input.note_cursor_pos = self.text_input.input_buffer.len();
        self.text_input.note_scroll_col = 0;
        self.note_update_scroll();
        self.text_input.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::LinkPath);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn follow_link_at_cursor(&mut self) {
        let (r, c) = self.canvas.cursor;
        let Some(comp) = self.canvas.grid.get(r, c) else { return };
        if comp.kind != ComponentKind::Link { return }
        let path_str = comp.text.clone().unwrap_or_default();
        if path_str.is_empty() {
            self.status_msg = "Link has no target — press [E] to set path.".into();
            return;
        }
        self.text_input.pending_link_path = Some(path_str);
        if self.grid_has_content() {
            self.dialog.confirm_new_choice = 0;
            self.mode = AppMode::ConfirmNew;
        } else {
            self.do_follow_link();
        }
    }

    pub fn do_follow_link(&mut self) {
        if let Some(path_str) = self.text_input.pending_link_path.take() {
            let path = std::path::PathBuf::from(&path_str);
            match self.load_layout_from(&path) {
                Ok(()) => {}
                Err(e) => self.status_msg = format!("Link failed: {e}"),
            }
            self.mode = AppMode::Build;
        }
    }

    pub fn place_pending_annotation(&mut self) {
        if let Some((kind, text)) = self.text_input.pending_annotation.take() {
            let (r, c) = self.canvas.cursor;
            self.push_undo();
            let mut comp = crate::components::Component::new(kind, self.pal.selected_diameter, self.pal.selected_material);
            comp.text = Some(text);
            self.canvas.grid.set(r, c, Some(comp));
            self.refresh_sim();
            self.status_msg = "Annotation placed.".into();
        }
    }

    pub fn cancel_pending_annotation(&mut self) {
        self.text_input.pending_annotation = None;
        self.status_msg = "Placement cancelled.".into();
    }
}
