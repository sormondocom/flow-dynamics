use super::*;

impl App {
    // ── File dialog ───────────────────────────────────────────────────────────

    pub fn open_file_dialog(&mut self, mode: FileDialogMode, purpose: FileDialogPurpose) {
        let start_dir = self.layout_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let initial_name = if mode == FileDialogMode::Save {
            self.layout_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "layout.json".to_string())
        } else {
            String::new()
        };

        self.dialog.pre_dialog_mode = self.mode;
        self.dialog.file_dialog = Some(FileDialogState::new(mode, purpose, start_dir, &initial_name));
        self.mode = AppMode::FileDialog;
    }

    pub fn cancel_file_dialog(&mut self) {
        self.dialog.file_dialog = None;
        self.mode = self.dialog.pre_dialog_mode;
    }

    pub fn file_dialog_nav(&mut self, delta: i32) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if delta < 0 { fd.nav_up(); } else { fd.nav_down(); }
    }

    pub fn file_dialog_page_up(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        fd.selected = fd.selected.saturating_sub(10);
    }

    pub fn file_dialog_page_down(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if !fd.entries.is_empty() {
            fd.selected = (fd.selected + 10).min(fd.entries.len() - 1);
        }
    }

    pub fn file_dialog_home(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        fd.selected = 0;
    }

    pub fn file_dialog_end(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if !fd.entries.is_empty() {
            fd.selected = fd.entries.len() - 1;
        }
    }

    pub fn file_dialog_toggle_focus(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.mode == FileDialogMode::Save {
            fd.focus_input = !fd.focus_input;
        }
    }

    pub fn file_dialog_backspace(&mut self) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.focus_input {
            fd.filename_input.pop();
        } else {
            fd.go_parent();
        }
    }

    pub fn file_dialog_type_char(&mut self, ch: char) {
        let Some(fd) = self.dialog.file_dialog.as_mut() else { return };
        if fd.mode == FileDialogMode::Save {
            if !fd.focus_input { fd.focus_input = true; }
            fd.filename_input.push(ch);
        }
    }

    pub fn file_dialog_confirm(&mut self) {
        use crate::file_dialog::EnterResult;
        let Some(mut fd) = self.dialog.file_dialog.take() else { return };
        fd.error_msg = None;

        match fd.mode {
            FileDialogMode::Open => {
                match fd.enter_selected() {
                    EnterResult::EnteredDir => {
                        self.dialog.file_dialog = Some(fd);
                    }
                    EnterResult::SelectedFile(path) => {
                        match self.load_layout_from(&path) {
                            Ok(()) => {
                                self.mode = AppMode::Build;
                                if fd.purpose == FileDialogPurpose::LoadLayout {
                                    // done — Build mode set above
                                }
                            }
                            Err(e) => {
                                fd.error_msg = Some(e);
                                self.dialog.file_dialog = Some(fd);
                            }
                        }
                    }
                    EnterResult::None => {
                        self.dialog.file_dialog = Some(fd);
                    }
                }
            }
            FileDialogMode::Save => {
                // If focus is on the list, entering a dir navigates; entering a file populates.
                if !fd.focus_input {
                    match fd.enter_selected() {
                        EnterResult::EnteredDir => {
                            self.dialog.file_dialog = Some(fd);
                            return;
                        }
                        EnterResult::SelectedFile(_) => {
                            fd.populate_filename_from_selection();
                            fd.focus_input = true;
                            self.dialog.file_dialog = Some(fd);
                            return;
                        }
                        EnterResult::None => {}
                    }
                }

                // Confirm save
                let Some(path) = fd.save_path() else {
                    fd.error_msg = Some("Enter a filename first.".into());
                    self.dialog.file_dialog = Some(fd);
                    return;
                };
                let purpose = fd.purpose.clone();
                let result = match &purpose {
                    FileDialogPurpose::ExportText => self.export_text_to(&path),
                    _ => self.save_layout_to(&path),
                };
                match result {
                    Ok(()) => {
                        self.mode = self.dialog.pre_dialog_mode;
                        if purpose == FileDialogPurpose::ExportText {
                            self.status_msg = format!("Exported text to '{}'.", path.display());
                        }
                        match purpose {
                            FileDialogPurpose::SaveThenNew  => self.do_new_diagram(),
                            FileDialogPurpose::SaveThenQuit => self.should_quit = true,
                            FileDialogPurpose::SaveThenFollowLink => self.do_follow_link(),
                            _ => {}
                        }
                    }
                    Err(e) => {
                        fd.error_msg = Some(e);
                        self.dialog.file_dialog = Some(fd);
                    }
                }
            }
        }
    }

    // ── Layout I/O ────────────────────────────────────────────────────────────

    pub fn save_layout_to(&mut self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.canvas.grid)
            .map_err(|e| format!("Serialise error: {e}"))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("Save failed: {e}"))?;
        self.layout_path = Some(path.to_path_buf());
        self.status_msg = format!("Saved to '{}'.", path.display());
        Ok(())
    }

    pub fn load_layout_from(&mut self, path: &Path) -> Result<(), String> {
        let txt = std::fs::read_to_string(path)
            .map_err(|e| format!("Load failed: {e}"))?;
        let mut grid: Grid = serde_json::from_str(&txt)
            .map_err(|e| format!("Parse error: {e}"))?;
        grid.rebuild_satellites();
        self.canvas.grid = grid;
        self.layout_path = Some(path.to_path_buf());
        self.sim.sim_result = None;
        self.status_msg = format!("Loaded '{}'.", path.display());
        Ok(())
    }

    // ── New diagram ───────────────────────────────────────────────────────────

    pub fn new_diagram(&mut self) {
        if self.grid_has_content() {
            self.dialog.confirm_new_choice = 0;
            self.mode = AppMode::ConfirmNew;
        } else {
            self.do_new_diagram();
        }
    }

    pub fn do_new_diagram(&mut self) {
        self.push_undo();
        let (w, h) = (self.canvas.grid.width, self.canvas.grid.height);
        self.canvas.grid = Grid::new(w, h);
        self.sim.sim_result = None;
        self.layout_path = None;
        self.mode = AppMode::Build;
        self.status_msg = "New diagram.".into();
    }

    pub(super) fn grid_has_content(&self) -> bool {
        self.canvas.grid.cells.iter().any(|row| row.iter().any(|c| c.is_some()))
    }

    // ── Export ────────────────────────────────────────────────────────────────

    pub fn export_text_to(&self, path: &std::path::Path) -> Result<(), String> {
        use crate::components::ComponentKind;

        // Find content bounds (include satellite cells)
        let mut min_r = self.canvas.grid.height;
        let mut max_r = 0usize;
        let mut min_c = self.canvas.grid.width;
        let mut max_c = 0usize;
        for r in 0..self.canvas.grid.height {
            for c in 0..self.canvas.grid.width {
                if self.canvas.grid.get(r, c).is_some() || self.canvas.grid.satellite_anchor(r, c).is_some() {
                    if r < min_r { min_r = r; }
                    if r > max_r { max_r = r; }
                    if c < min_c { min_c = c; }
                    if c > max_c { max_c = c; }
                }
            }
        }
        if min_r > max_r {
            return Err("Canvas is empty.".into());
        }

        // Extend bounds for labels ([text]) and notes (multi-line [text])
        for r in 0..self.canvas.grid.height {
            for c in 0..self.canvas.grid.width {
                if let Some(comp) = self.canvas.grid.get(r, c) {
                    match comp.kind {
                        ComponentKind::Label => {
                            if let Some(text) = &comp.text {
                                let end_c = c + text.chars().count() + 1;
                                if end_c > max_c { max_c = end_c; }
                            }
                        }
                        ComponentKind::Note => {
                            if let Some(text) = &comp.text {
                                let segs: Vec<&str> = text.split('\n').collect();
                                let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                                let end_c = c + max_w + 3;
                                let end_r = r + segs.len() + 3;
                                if end_c > max_c { max_c = end_c; }
                                if end_r > max_r { max_r = end_r; }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let cell_free = |gr: usize, gc: usize| {
            self.canvas.grid.get(gr, gc).is_none() && self.canvas.grid.satellite_anchor(gr, gc).is_none()
        };

        // Precompute note box overlay
        let mut note_export: std::collections::HashMap<(usize, usize), char> =
            std::collections::HashMap::new();
        for nr in 0..self.canvas.grid.height {
            for nc in 0..self.canvas.grid.width {
                if let Some(comp) = self.canvas.grid.get(nr, nc) {
                    if comp.kind == ComponentKind::Note {
                        if let Some(text) = &comp.text {
                            let segs: Vec<&str> = text.split('\n').collect();
                            let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                            let inner_w = max_w + 2;
                            let right_c = nc + inner_w + 1;
                            for ci in 1..=inner_w { note_export.insert((nr, nc + ci), '═'); }
                            note_export.insert((nr, right_c), '╗');
                            note_export.insert((nr + 1, nc), '║');
                            for ci in 1..=inner_w { note_export.insert((nr + 1, nc + ci), ' '); }
                            note_export.insert((nr + 1, right_c), '║');
                            for (li, seg) in segs.iter().enumerate() {
                                let row = nr + li + 2;
                                note_export.insert((row, nc), '║');
                                note_export.insert((row, nc + 1), ' ');
                                let chars: Vec<char> = seg.chars().collect();
                                for ci in 0..max_w {
                                    note_export.insert((row, nc + 2 + ci), chars.get(ci).copied().unwrap_or(' '));
                                }
                                note_export.insert((row, nc + max_w + 2), ' ');
                                note_export.insert((row, right_c), '║');
                            }
                            let bpad = nr + segs.len() + 2;
                            note_export.insert((bpad, nc), '║');
                            for ci in 1..=inner_w { note_export.insert((bpad, nc + ci), ' '); }
                            note_export.insert((bpad, right_c), '║');
                            let bot = nr + segs.len() + 3;
                            note_export.insert((bot, nc), '╚');
                            for ci in 1..=inner_w { note_export.insert((bot, nc + ci), '═'); }
                            note_export.insert((bot, right_c), '╝');
                        }
                    }
                }
            }
        }

        let mut output = String::new();
        for r in min_r..=max_r {
            let mut row_chars: Vec<char> = vec![' '; max_c - min_c + 1];

            for c in min_c..=max_c {
                let ch = self.export_cell_char(r, c, min_c);
                row_chars[c - min_c] = ch;
                if ch == ' ' {
                    if let Some(&nch) = note_export.get(&(r, c)) {
                        row_chars[c - min_c] = nch;
                    }
                }
            }

            // Spread label text: [text]
            for c in min_c..=max_c {
                if let Some(comp) = self.canvas.grid.get(r, c) {
                    if comp.kind == ComponentKind::Label {
                        if let Some(text) = &comp.text {
                            let mut ok = true;
                            for (i, ch) in text.chars().enumerate() {
                                let tc = c + i + 1;
                                if tc > max_c { ok = false; break; }
                                if cell_free(r, tc) { row_chars[tc - min_c] = ch; } else { ok = false; break; }
                            }
                            if ok {
                                let close_c = c + text.chars().count() + 1;
                                if close_c <= max_c && cell_free(r, close_c) {
                                    row_chars[close_c - min_c] = ']';
                                }
                            }
                        }
                    }
                }
            }

            let row_str: String = row_chars.iter().collect();
            let trimmed = row_str.trim_end();
            output.push_str(trimmed);
            output.push('\n');
        }

        std::fs::write(path, output).map_err(|e| format!("Export failed: {e}"))
    }

    fn export_cell_char(&self, r: usize, c: usize, _min_c: usize) -> char {
        use crate::components::ComponentKind;

        if let Some((ar, ac)) = self.canvas.grid.satellite_anchor(r, c) {
            if let Some(comp) = self.canvas.grid.get(ar, ac) {
                let pr = comp.effective_port_row();
                let (fw, fh) = comp.effective_footprint();
                let dr = r.wrapping_add(pr).wrapping_sub(ar);
                let dc = c.wrapping_sub(ac);
                return self.export_composite_char(comp, fw, fh, pr, dr, dc);
            }
        }

        let Some(comp) = self.canvas.grid.get(r, c) else { return ' '; };

        match comp.kind {
            ComponentKind::Label => '[',
            ComponentKind::Note => '*',
            _ if comp.effective_is_composite() => {
                let (fw, fh) = comp.effective_footprint();
                let pr = comp.effective_port_row();
                self.export_composite_char(comp, fw, fh, pr, pr, 0)
            }
            _ => {
                let g = self.glyph_registry.resolve(comp.kind, comp.material, comp.diameter);
                g.symbol
            }
        }
    }

    fn export_composite_char(
        &self,
        comp: &crate::components::Component,
        fw: usize, fh: usize, pr: usize,
        dr: usize, dc: usize,
    ) -> char {
        let label = comp.effective_composite_label();
        if comp.kind == crate::components::ComponentKind::Custom {
            if let Some(id) = &comp.custom_id {
                let customs = self.glyph_registry.custom_components();
                if let Some(def) = customs.iter().find(|d| &d.id == id) {
                    if let Some(ch) = def.get_cell(dr, dc) { return ch; }
                    if dr == 0 || dr + 1 == fh || dc == 0 || dc + 1 == fw { return ' '; }
                    return crate::ui::composite_box_char(
                        fw - 2, fh - 2, pr.saturating_sub(1), dr - 1, dc - 1, label, None, true,
                    );
                }
            }
            return '#';
        }
        let (_, _, ae, aw) = comp.kind.connections();
        let north_dc = comp.kind.composite_north_inlet_dc(fw);
        crate::ui::composite_box_char(fw, fh, pr, dr, dc, label, north_dc, ae || aw)
    }

    // ── Settings screen ──────────────────────────────────────────────────────

    pub fn open_settings(&mut self) {
        self.dialog.settings_status.clear();
        self.mode = AppMode::Settings;
    }

    pub fn close_settings(&mut self) {
        self.mode = AppMode::Build;
    }

    pub fn cycle_grid_scale(&mut self) {
        self.config.cycle_grid_scale();
        self.config.save();
        self.status_msg = format!("Scale: {}", self.config.grid_scale_label());
    }

    pub fn settings_nav(&mut self, delta: isize) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        self.dialog.settings_idx = (self.dialog.settings_idx as isize + delta)
            .clamp(0, n as isize - 1) as usize;
    }

    pub fn settings_home(&mut self) {
        self.dialog.settings_idx = 0;
    }

    pub fn settings_end(&mut self) {
        let n = self.config.glyph_files.len();
        if n > 0 { self.dialog.settings_idx = n - 1; }
    }

    pub fn settings_begin_add(&mut self) {
        self.text_input.input_buffer.clear();
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::AddGlyphFile);
        self.status_msg = "Glyph file path:".into();
    }

    pub fn settings_remove(&mut self) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        let i = self.dialog.settings_idx.min(n - 1);
        self.config.glyph_files.remove(i);
        self.config.save();
        self.dialog.settings_idx = self.dialog.settings_idx.min(self.config.glyph_files.len().saturating_sub(1));
        self.dialog.settings_status = "Removed.".into();
    }

    pub fn settings_load_now(&mut self) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        let path = self.config.glyph_files[self.dialog.settings_idx.min(n - 1)].clone();
        match self.try_load_glyph_file(&path) {
            Ok(()) => self.dialog.settings_status = format!("OK — loaded {}", path.display()),
            Err(e) => self.dialog.settings_status = format!("Error: {e}"),
        }
    }

    // ── Cost Estimator ───────────────────────────────────────────────────────

    pub fn open_cost_estimator(&mut self) {
        self.cost_cursor = 0;
        self.mode = AppMode::CostEstimator;
    }

    pub fn close_cost_estimator(&mut self) {
        self.text_input.input_mode = InputMode::Normal;
        self.config.save();
        self.mode = AppMode::Build;
    }

    pub fn cost_nav(&mut self, delta: isize) {
        use crate::cost_config::FITTING_GROUPS;
        use crate::glyphs::{ALL_DIAMETERS, ALL_MATERIALS};
        let total = ALL_MATERIALS.len() * ALL_DIAMETERS.len() + FITTING_GROUPS.len();
        if total == 0 { return; }
        self.cost_cursor = (self.cost_cursor as isize + delta)
            .rem_euclid(total as isize) as usize;
    }

    /// Begin editing the price at the current cursor row.
    pub fn begin_cost_price_edit(&mut self) {
        use crate::cost_config::FITTING_GROUPS;
        use crate::glyphs::{ALL_DIAMETERS, ALL_MATERIALS};
        let pipe_count = ALL_MATERIALS.len() * ALL_DIAMETERS.len();
        let price = if self.cost_cursor < pipe_count {
            let mat = ALL_MATERIALS[self.cost_cursor / ALL_DIAMETERS.len()];
            let dia = ALL_DIAMETERS[self.cost_cursor % ALL_DIAMETERS.len()];
            self.config.costs.pipe_price(mat, dia)
        } else {
            let fi = self.cost_cursor - pipe_count;
            if fi < FITTING_GROUPS.len() {
                let key = FITTING_GROUPS[fi].0;
                *self.config.costs.fitting_per_unit.get(key).unwrap_or(&0.0)
            } else {
                0.0
            }
        };
        self.text_input.input_buffer = format!("{:.2}", price);
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::CostPrice);
    }

    pub fn confirm_cost_price_edit(&mut self) {
        use crate::cost_config::FITTING_GROUPS;
        use crate::glyphs::{ALL_DIAMETERS, ALL_MATERIALS};
        if let Ok(price) = self.text_input.input_buffer.trim().parse::<f32>() {
            let pipe_count = ALL_MATERIALS.len() * ALL_DIAMETERS.len();
            if self.cost_cursor < pipe_count {
                let mat = ALL_MATERIALS[self.cost_cursor / ALL_DIAMETERS.len()];
                let dia = ALL_DIAMETERS[self.cost_cursor % ALL_DIAMETERS.len()];
                self.config.costs.set_pipe_price(mat, dia, price.max(0.0));
            } else {
                let fi = self.cost_cursor - pipe_count;
                if fi < FITTING_GROUPS.len() {
                    let key = FITTING_GROUPS[fi].0.to_string();
                    self.config.costs.fitting_per_unit.insert(key, price.max(0.0));
                }
            }
            self.config.save();
        }
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
    }

    pub fn cancel_cost_price_edit(&mut self) {
        self.text_input.input_mode = InputMode::Normal;
        self.text_input.input_buffer.clear();
    }
}
