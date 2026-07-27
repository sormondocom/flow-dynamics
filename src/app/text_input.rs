use super::*;
use super::glyph_editor::{parse_composite_size, snapshot_standard_as_custom, trim_composite};

impl App {
    /// Commit a text-input prompt started from the glyph editor or other dialogs.
    pub fn commit_text_input(&mut self) {
        // resolve here so the borrow checker is happy inside the match arms
        let copy_kind_idx = self.editor.kind_idx;
        let buf = self.text_input.input_buffer.trim().to_string();
        match self.text_input.input_mode {
            InputMode::EditingText(TextEditTarget::SaveLibrary) => {
                let path = std::path::Path::new(&buf);
                // Trim all composite components to their used bounding box.
                for def in &mut self.glyph_registry.library.custom_components {
                    trim_composite(def);
                }
                self.glyph_registry.library.version = "2.0".into();
                // Reset editor state so cursor is valid after trim.
                self.editor.composite_cursor = (1, 1);
                self.editor.composite_viewport = (0, 0);
                match self.glyph_registry.save_library(path) {
                    Ok(()) => {
                        self.editor.status = format!("Saved to '{buf}'.");
                        self.glyph_registry.library_path = Some(path.to_path_buf());
                    }
                    Err(e) => self.editor.status = format!("Save failed: {e}"),
                }
            }
            InputMode::EditingText(TextEditTarget::LoadLibrary) => {
                let path = std::path::Path::new(&buf);
                match self.glyph_registry.load_library(path) {
                    Ok(()) => {
                        self.rebuild_palette();
                        self.editor.status = format!("Loaded '{buf}'.");
                    }
                    Err(e) => self.editor.status = format!("Load failed: {e}"),
                }
            }
            InputMode::EditingText(TextEditTarget::NewCompName) => {
                if !buf.is_empty() {
                    let id = buf.to_lowercase().replace(' ', "_");
                    let [r, g, b] = self.editor.current_color();
                    let def = CustomCompDef::new(
                        id.clone(),
                        buf.clone(),
                        GlyphDef { symbol: self.editor.current_symbol(), fg: [r, g, b] },
                    );
                    self.glyph_registry.add_custom_component(def);
                    self.rebuild_palette();
                    // Auto-select the new component in the editor list.
                    let ci = self.glyph_registry.custom_components().len() - 1;
                    let target_kind_idx = ComponentKind::all_palette().len() + ci;
                    self.editor.kind_idx = target_kind_idx;
                    // Sync display_idx to the new component's row in the grouped list.
                    if let Some(di) = self.editor.display_rows.iter().position(|r| {
                        matches!(r, EditorDisplayRow::Component { kind_idx } if *kind_idx == target_kind_idx)
                    }) {
                        self.editor.display_idx = di;
                    }
                    self.editor.status = format!(
                        "Added '{buf}'. Press Enter to change its glyph."
                    );
                } else {
                    self.editor.status = "Name cannot be empty.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::CompWidth) => {
                let static_len = ComponentKind::all_palette().len();
                let ci = self.editor.kind_idx.saturating_sub(static_len);
                if ci < self.glyph_registry.library.custom_components.len() {
                    // Parse "WxH", "W" (height defaults to 3), or "0" (revert to single-cell).
                    let (w, h) = parse_composite_size(&buf);
                    let def = &mut self.glyph_registry.library.custom_components[ci];
                    if w >= 3 && h >= 3 {
                        def.composite_size = Some((w, h));
                        def.connections_nsew = [false, false, true, true]; // E+W pass-through
                        self.editor.status = format!("'{}' → composite {w}×{h}.", def.label);
                    } else {
                        def.composite_size = None;
                        self.editor.status = format!("'{}' → single-cell glyph.", def.label);
                    }
                    self.rebuild_palette();
                }
            }
            InputMode::EditingText(TextEditTarget::AssemblyName) => {
                if buf.is_empty() {
                    self.status_msg = "Assembly name cannot be empty.".into();
                } else {
                    self.save_assembly_named(buf);
                }
            }
            InputMode::EditingText(TextEditTarget::AddGlyphFile) => {
                if !buf.is_empty() {
                    let path = PathBuf::from(&buf);
                    self.config.glyph_files.push(path.clone());
                    self.config.save();
                    match self.try_load_glyph_file(&path) {
                        Ok(()) => {
                            self.dialog.settings_status = format!("OK — loaded {}", path.display());
                        }
                        Err(e) => {
                            self.dialog.settings_status = format!("Added (not yet loaded): {e}");
                        }
                    }
                    self.dialog.settings_idx = self.config.glyph_files.len().saturating_sub(1);
                }
            }
            InputMode::EditingText(TextEditTarget::CustomRgb) => {
                let parts: Vec<&str> = buf.split(',').collect();
                if parts.len() == 3 {
                    let r = parts[0].trim().parse::<u8>().unwrap_or(128);
                    let g = parts[1].trim().parse::<u8>().unwrap_or(128);
                    let b = parts[2].trim().parse::<u8>().unwrap_or(128);
                    self.editor.set_custom_rgb(r, g, b);
                } else {
                    self.editor.status = "Format: R,G,B  e.g.  255,128,0".into();
                }
            }
            InputMode::EditingText(TextEditTarget::BuildCustomRgb) => {
                let parts: Vec<&str> = buf.split(',').collect();
                if parts.len() == 3 {
                    let r = parts[0].trim().parse::<u8>().unwrap_or(128);
                    let g = parts[1].trim().parse::<u8>().unwrap_or(128);
                    let b = parts[2].trim().parse::<u8>().unwrap_or(128);
                    self.pal.build_custom_rgb = Some([r, g, b]);
                } else {
                    self.status_msg = "Format: R,G,B  e.g.  255,128,0".into();
                }
            }
            InputMode::EditingText(TextEditTarget::LabelText) => {
                self.mode = AppMode::Build;
                if let Some((r, c)) = self.text_input.edit_annotation_pos.take() {
                    // Editing existing label in place
                    self.push_undo();
                    if buf.is_empty() {
                        self.canvas.grid.set(r, c, None);
                        self.status_msg = "Label removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Label, self.pal.selected_diameter, self.pal.selected_material);
                        comp.text = Some(buf);
                        self.canvas.grid.set(r, c, Some(comp));
                        self.status_msg = "Label updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    // New label — enter placement mode so the user can choose position
                    self.text_input.pending_annotation = Some((ComponentKind::Label, buf));
                    self.status_msg = "Move cursor to position, [Enter] to place, [Esc] to cancel.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::NoteText) => {
                self.mode = AppMode::Build;
                if let Some((r, c)) = self.text_input.edit_annotation_pos.take() {
                    // Editing existing note in place
                    self.push_undo();
                    if buf.is_empty() {
                        self.canvas.grid.set(r, c, None);
                        self.status_msg = "Note removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Note, self.pal.selected_diameter, self.pal.selected_material);
                        comp.text = Some(buf);
                        self.canvas.grid.set(r, c, Some(comp));
                        self.status_msg = "Note updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    self.text_input.pending_annotation = Some((ComponentKind::Note, buf));
                    self.status_msg = "Move cursor to position, [Enter] to place, [Esc] to cancel.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::RenameComp) => {
                if buf.is_empty() {
                    self.editor.status = "Name cannot be empty.".into();
                } else {
                    let static_len = ComponentKind::all_palette().len();
                    let ci = self.editor.kind_idx.saturating_sub(static_len);
                    if ci < self.glyph_registry.library.custom_components.len() {
                        let new_id = buf.to_lowercase().replace(' ', "_");
                        let def = &mut self.glyph_registry.library.custom_components[ci];
                        def.label = buf.clone();
                        def.id = new_id;
                        self.rebuild_palette();
                        self.editor.status = format!("Renamed to '{buf}'.");
                    }
                }
            }
            InputMode::EditingText(TextEditTarget::LinkPath) => {
                self.mode = AppMode::Build;
                if let Some((r, c)) = self.text_input.edit_annotation_pos.take() {
                    self.push_undo();
                    if buf.is_empty() {
                        self.canvas.grid.set(r, c, None);
                        self.status_msg = "Link removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Link, self.pal.selected_diameter, self.pal.selected_material);
                        comp.text = Some(buf);
                        self.canvas.grid.set(r, c, Some(comp));
                        self.status_msg = "Link updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    self.text_input.pending_annotation = Some((ComponentKind::Link, buf));
                    self.status_msg = "Move cursor to position, [Enter] to place, [Esc] to cancel.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::SourcePressure) => {
                self.text_input.input_mode = InputMode::Normal;
                self.mode = AppMode::Build;
                match buf.parse::<f32>() {
                    Ok(psi) => {
                        let psi = psi.clamp(10.0, 200.0);
                        let (r, c) = self.canvas.cursor;
                        if self.canvas.grid.get(r, c).map(|co| co.kind == ComponentKind::Source).unwrap_or(false) {
                            self.push_undo();
                            if let Some(comp) = self.canvas.grid.get_mut(r, c) {
                                comp.source_pressure_psi = psi;
                            }
                            self.refresh_sim();
                            self.status_msg = format!("Inlet pressure set to {psi:.1} PSI.");
                        }
                    }
                    Err(_) => {
                        self.status_msg = "Invalid pressure — enter a number between 10 and 200.".into();
                    }
                }
            }
            InputMode::EditingText(TextEditTarget::PrvSetpoint) => {
                self.text_input.input_mode = InputMode::Normal;
                self.mode = AppMode::Build;
                match buf.parse::<f32>() {
                    Ok(psi) => {
                        let psi = psi.clamp(10.0, 200.0);
                        let (r, c) = self.canvas.cursor;
                        if self.canvas.grid.get(r, c)
                            .map(|co| co.kind == ComponentKind::PressureReducingValve)
                            .unwrap_or(false)
                        {
                            self.push_undo();
                            if let Some(comp) = self.canvas.grid.get_mut(r, c) {
                                comp.prv_setpoint_psi = psi;
                            }
                            self.refresh_sim();
                            self.status_msg = format!("PRV setpoint set to {psi:.1} PSI.");
                        }
                    }
                    Err(_) => {
                        self.status_msg = "Invalid pressure — enter a number between 10 and 200.".into();
                    }
                }
            }
            InputMode::EditingText(TextEditTarget::CopyComp) => {
                if buf.is_empty() {
                    self.editor.status = "Name cannot be empty.".into();
                } else {
                    let static_len = ComponentKind::all_palette().len();
                    let new_id = buf.to_lowercase().replace(' ', "_");
                    if copy_kind_idx < static_len {
                        // Copy from a built-in standard component → snapshot as custom
                        let kind = ComponentKind::all_palette()[copy_kind_idx];
                        let glyph = self.glyph_registry.resolve(kind, self.pal.selected_material, self.pal.selected_diameter);
                        let def = snapshot_standard_as_custom(kind, new_id, buf.clone(), glyph);
                        self.glyph_registry.add_custom_component(def);
                    } else {
                        // Copy from an existing custom component → deep clone
                        let ci = copy_kind_idx - static_len;
                        if ci < self.glyph_registry.library.custom_components.len() {
                            let mut clone = self.glyph_registry.library.custom_components[ci].clone();
                            clone.id = new_id;
                            clone.label = buf.clone();
                            self.glyph_registry.add_custom_component(clone);
                        }
                    }
                    self.rebuild_palette();
                    let new_ci = self.glyph_registry.custom_components().len() - 1;
                    let target_kind_idx = static_len + new_ci;
                    self.editor.kind_idx = target_kind_idx;
                    if let Some(di) = self.editor.display_rows.iter().position(|r| {
                        matches!(r, EditorDisplayRow::Component { kind_idx } if *kind_idx == target_kind_idx)
                    }) {
                        self.editor.display_idx = di;
                    }
                    self.editor.status = format!("Copied to '{buf}'. Edit ports/cells as needed.");
                }
            }
            InputMode::EditingText(TextEditTarget::NewGroupName) => {
                if !buf.is_empty() {
                    if !self.config.groups.iter().any(|g| g.name == buf) {
                        self.config.groups.push(crate::config::GroupConfig { name: buf.clone(), collapsed: false });
                        self.config.save();
                    }
                    self.rebuild_display_rows();
                    self.rebuild_editor_display_rows();
                    self.editor.status = format!("Created group '{buf}'.");
                }
            }
            InputMode::EditingText(TextEditTarget::GroupAssign) => {
                if !buf.is_empty() {
                    if let Some(flat_idx) = self.pal.group_picker_for_flat.take() {
                        self.assign_component_to_group(flat_idx, buf.clone());
                    } else if let Some(kind_idx) = self.editor.group_picker_for_kind.take() {
                        self.assign_editor_component_to_group(kind_idx, buf.clone());
                    }
                    self.editor.status = format!("Assigned to group '{buf}'.");
                }
            }
            _ => {}
        }
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
        self.text_input.note_cursor_pos = 0;
        self.text_input.note_scroll_row = 0;
        self.text_input.note_scroll_col = 0;
    }

    pub fn push_input_char(&mut self, ch: char) {
        match self.text_input.input_mode {
            InputMode::EditingLength => {
                if (ch.is_ascii_digit() || (ch == '.' && !self.text_input.input_buffer.contains('.')))
                    && self.text_input.input_buffer.len() < 8
                {
                    self.text_input.input_buffer.push(ch);
                }
            }
            InputMode::EditingText(target) => {
                if ch.is_ascii_graphic() || ch == ' ' {
                    let limit = match target {
                        TextEditTarget::NoteText => 400,
                        TextEditTarget::LinkPath => 256,
                        _ => 120,
                    };
                    if self.text_input.input_buffer.len() < limit {
                        if matches!(target, TextEditTarget::NoteText | TextEditTarget::LabelText | TextEditTarget::LinkPath) {
                            let pos = self.text_input.note_cursor_pos.min(self.text_input.input_buffer.len());
                            self.text_input.input_buffer.insert(pos, ch);
                            self.text_input.note_cursor_pos = pos + ch.len_utf8();
                            self.note_update_scroll();
                        } else {
                            self.text_input.input_buffer.push(ch);
                        }
                    }
                }
            }
            InputMode::Normal => {}
        }
    }

    pub fn pop_input_char(&mut self) {
        let cursor_edit = matches!(
            self.text_input.input_mode,
            InputMode::EditingText(TextEditTarget::NoteText)
                | InputMode::EditingText(TextEditTarget::LabelText)
                | InputMode::EditingText(TextEditTarget::LinkPath)
        );
        if cursor_edit {
            if self.text_input.note_cursor_pos > 0 {
                let pos = self.text_input.note_cursor_pos;
                let prev_len = self.text_input.input_buffer[..pos]
                    .chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                if prev_len > 0 {
                    self.text_input.input_buffer.remove(pos - prev_len);
                    self.text_input.note_cursor_pos = pos - prev_len;
                    self.note_update_scroll();
                }
            }
        } else {
            self.text_input.input_buffer.pop();
        }
    }

    /// Insert a newline at the cursor (Shift+Enter). Only valid in NoteText mode.
    pub fn push_note_newline(&mut self) {
        if matches!(self.text_input.input_mode, InputMode::EditingText(TextEditTarget::NoteText))
            && self.text_input.input_buffer.len() < 400
        {
            let pos = self.text_input.note_cursor_pos.min(self.text_input.input_buffer.len());
            self.text_input.input_buffer.insert(pos, '\n');
            self.text_input.note_cursor_pos = pos + 1;
            self.note_update_scroll();
        }
    }

    /// Returns `(line, col)` — both 0-indexed byte offsets — for `note_cursor_pos`.
    fn note_cursor_lc(&self) -> (usize, usize) {
        let pos = self.text_input.note_cursor_pos.min(self.text_input.input_buffer.len());
        let before = &self.text_input.input_buffer[..pos];
        let line = before.chars().filter(|&c| c == '\n').count();
        let col = match before.rfind('\n') {
            Some(nl) => pos - nl - 1,
            None => pos,
        };
        (line, col)
    }

    /// Adjust `note_scroll_row`/`note_scroll_col` so the cursor stays in view.
    pub(super) fn note_update_scroll(&mut self) {
        const VIS_ROWS: usize = 3;
        const VIS_W: usize = 50; // conservative estimate of visible content width

        let (line, col) = self.note_cursor_lc();

        if line < self.text_input.note_scroll_row {
            self.text_input.note_scroll_row = line;
        } else if line >= self.text_input.note_scroll_row + VIS_ROWS {
            self.text_input.note_scroll_row = line + 1 - VIS_ROWS;
        }

        if col < self.text_input.note_scroll_col {
            self.text_input.note_scroll_col = col;
        } else if col >= self.text_input.note_scroll_col + VIS_W {
            self.text_input.note_scroll_col = col + 1 - VIS_W;
        }
    }

    /// Returns true when the active text-edit target is a note (for key routing).
    pub fn is_note_text_mode(&self) -> bool {
        matches!(self.text_input.input_mode, InputMode::EditingText(TextEditTarget::NoteText))
    }

    pub fn note_move_left(&mut self) {
        if self.text_input.note_cursor_pos > 0 {
            let prev_len = self.text_input.input_buffer[..self.text_input.note_cursor_pos]
                .chars().last().map(|c| c.len_utf8()).unwrap_or(0);
            self.text_input.note_cursor_pos -= prev_len;
            self.note_update_scroll();
        }
    }

    pub fn note_move_right(&mut self) {
        let pos = self.text_input.note_cursor_pos;
        if pos < self.text_input.input_buffer.len() {
            let next_len = self.text_input.input_buffer[pos..]
                .chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            self.text_input.note_cursor_pos += next_len;
            self.note_update_scroll();
        }
    }

    pub fn note_move_up(&mut self) {
        let (line, col) = self.note_cursor_lc();
        if line == 0 {
            self.text_input.note_cursor_pos = 0;
        } else {
            let lines: Vec<&str> = self.text_input.input_buffer.split('\n').collect();
            let new_col = col.min(lines[line - 1].len());
            let start: usize = lines[..line - 1].iter().map(|l| l.len() + 1).sum();
            self.text_input.note_cursor_pos = start + new_col;
        }
        self.note_update_scroll();
    }

    pub fn note_move_down(&mut self) {
        let (line, col) = self.note_cursor_lc();
        let lines: Vec<&str> = self.text_input.input_buffer.split('\n').collect();
        if line + 1 < lines.len() {
            let new_col = col.min(lines[line + 1].len());
            let start: usize = lines[..=line].iter().map(|l| l.len() + 1).sum();
            self.text_input.note_cursor_pos = start + new_col;
        } else {
            self.text_input.note_cursor_pos = self.text_input.input_buffer.len();
        }
        self.note_update_scroll();
    }

    pub fn is_label_text_mode(&self) -> bool {
        matches!(self.text_input.input_mode, InputMode::EditingText(TextEditTarget::LabelText))
    }

    pub fn label_move_left(&mut self) { self.note_move_left(); }
    pub fn label_move_right(&mut self) { self.note_move_right(); }

    pub fn is_link_path_mode(&self) -> bool {
        matches!(self.text_input.input_mode, InputMode::EditingText(TextEditTarget::LinkPath))
    }

    pub fn cancel_input(&mut self) {
        if self.mode == AppMode::AnnotationDialog {
            self.mode = AppMode::Build;
            self.text_input.edit_annotation_pos = None;
        }
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
        self.text_input.note_cursor_pos = 0;
        self.text_input.note_scroll_row = 0;
        self.text_input.note_scroll_col = 0;
        self.status_msg = "Cancelled.".into();
    }

    pub fn commit_length_input(&mut self) {
        let buf = self.text_input.input_buffer.trim().to_string();
        let (r, c) = self.canvas.cursor;

        if self.mode == AppMode::ComponentDetail {
            match buf.parse::<f32>() {
                Ok(inches) if inches >= 0.0 => {
                    let raw_port = self.detail_active_ports()
                        .get(self.detail.detail_port_cursor)
                        .map(|&(p, _)| p);
                    if let Some(port) = raw_port {
                        const DIRS: [&str; 4] = ["North", "South", "East", "West"];
                        if self.detail.detail_for_palette {
                            let entry = self.pal.default_arm_lengths
                                .entry(self.detail.detail_kind)
                                .or_insert([0.0; 4]);
                            entry[port] = inches / 12.0;
                        } else if let Some(comp) = self.canvas.grid.get_mut(r, c) {
                            comp.arm_lengths[port] = inches / 12.0;
                            self.refresh_sim();
                        }
                        self.status_msg = format!(
                            "{} stub: {} in ({:.2} ft)",
                            DIRS[port], inches.round() as i32, inches / 12.0,
                        );
                    }
                }
                Ok(_) => self.status_msg = "Length must be 0 or more inches.".into(),
                Err(_) => self.status_msg = "Invalid number.".into(),
            }
            self.text_input.input_mode = InputMode::Normal;
            self.text_input.input_buffer.clear();
            return;
        }

        match buf.parse::<f32>() {
            Ok(inches) if inches >= 1.0 => {
                let on_pipe = self.canvas.grid.get(r, c)
                    .map(|co| matches!(co.kind, ComponentKind::PipeH | ComponentKind::PipeV))
                    .unwrap_or(false);

                if on_pipe {
                    if let Some(comp) = self.canvas.grid.get_mut(r, c) {
                        comp.pipe_length = inches / 12.0;
                        self.status_msg = format!(
                            "Pipe length set to {} in ({:.2} ft)",
                            inches.round() as i32,
                            comp.pipe_length,
                        );
                        self.refresh_sim();
                    }
                } else {
                    let kind = self.selected_component_kind();
                    if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                        self.pal.default_lengths.insert(kind, inches / 12.0);
                        self.status_msg = format!(
                            "Default {} length: {} in ({:.2} ft)",
                            kind.label(),
                            inches.round() as i32,
                            inches / 12.0,
                        );
                    }
                }
            }
            Ok(_) => self.status_msg = "Length must be at least 1 inch.".into(),
            Err(_) => self.status_msg = "Invalid number — enter length in inches.".into(),
        }
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
    }
}
