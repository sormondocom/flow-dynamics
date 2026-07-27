use super::*;

impl App {
    /// Rebuild the component palette: all static kinds (excluding the generic Custom
    /// placeholder) followed by one entry per custom component in the registry.
    /// If no custom components exist, a single generic Custom placeholder is appended.
    pub fn rebuild_palette(&mut self) {
        let prev_len = self.pal.palette.len();

        self.pal.palette = ComponentKind::all_palette()
            .iter()
            .filter(|&&k| k != ComponentKind::Custom)
            .copied()
            .collect();
        self.pal.palette_custom_indices = vec![None; self.pal.palette.len()];

        let customs = self.glyph_registry.custom_components();
        if customs.is_empty() {
            self.pal.palette.push(ComponentKind::Custom);
            self.pal.palette_custom_indices.push(None);
        } else {
            for i in 0..customs.len() {
                self.pal.palette.push(ComponentKind::Custom);
                self.pal.palette_custom_indices.push(Some(i));
            }
        }

        // Clamp cursor in case the palette shrank
        if self.pal.palette_idx >= self.pal.palette.len() {
            self.pal.palette_idx = self.pal.palette.len().saturating_sub(1);
        }
        let _ = prev_len;
        self.rebuild_display_rows();
        self.rebuild_editor_display_rows();
    }

    /// Returns the custom-component registry index for the currently selected palette
    /// slot, or None if the selected slot is not a custom component.
    pub fn selected_custom_idx(&self) -> Option<usize> {
        self.pal.palette_custom_indices.get(self.pal.palette_idx).copied().flatten()
    }

    // ── Group / display-row infrastructure ───────────────────────────────────

    /// Returns the stable string key used to look up a palette flat-list item's group assignment.
    pub fn component_key_for(&self, flat_idx: usize) -> String {
        if flat_idx >= self.pal.palette.len() { return "unknown".into(); }
        let kind = self.pal.palette[flat_idx];
        if kind == crate::components::ComponentKind::Custom {
            if let Some(ci) = self.pal.palette_custom_indices.get(flat_idx).copied().flatten() {
                let customs = self.glyph_registry.custom_components();
                if ci < customs.len() {
                    return customs[ci].id.clone();
                }
            }
            return format!("custom_{flat_idx}");
        }
        crate::glyphs::kind_key(kind).to_string()
    }

    /// Returns the group name for a palette flat index (defaults to "General").
    pub fn group_for_flat_idx(&self, flat_idx: usize) -> &str {
        let key = self.component_key_for(flat_idx);
        self.config.component_groups.get(&key).map(|s| s.as_str()).unwrap_or("General")
    }

    /// Builds the palette display list (group headers + component rows) from config.
    pub fn rebuild_display_rows(&mut self) {
        // Ensure "General" always exists as the first group.
        if self.config.groups.is_empty() {
            self.config.groups.push(crate::config::GroupConfig { name: "General".into(), collapsed: false });
        }
        let mut rows = Vec::new();
        for (gi, group) in self.config.groups.iter().enumerate() {
            rows.push(crate::palette_state::PaletteDisplayRow::GroupHeader { group_idx: gi });
            if !group.collapsed {
                for flat_idx in 0..self.pal.palette.len() {
                    let in_this_group = if group.name == "General" {
                        let assigned = self.config.component_groups.get(&self.component_key_for(flat_idx));
                        match assigned {
                            None => true,
                            Some(name) => !self.config.groups.iter().any(|g| &g.name == name) || name == "General",
                        }
                    } else {
                        let comp_group = self.group_for_flat_idx(flat_idx);
                        comp_group == group.name
                    };
                    if in_this_group {
                        rows.push(crate::palette_state::PaletteDisplayRow::Component { flat_idx });
                    }
                }
            }
        }
        self.pal.display_rows = rows;
        // Keep display_idx in bounds.
        if self.pal.display_idx >= self.pal.display_rows.len() {
            self.pal.display_idx = self.pal.display_rows.len().saturating_sub(1);
        }
        // If cursor is on a component row, sync palette_idx.
        self.sync_palette_idx_from_display();
    }

    pub(super) fn sync_palette_idx_from_display(&mut self) {
        if let Some(crate::palette_state::PaletteDisplayRow::Component { flat_idx }) =
            self.pal.display_rows.get(self.pal.display_idx)
        {
            self.pal.palette_idx = *flat_idx;
        }
    }

    /// Builds the glyph editor display list (group headers + component rows).
    pub fn rebuild_editor_display_rows(&mut self) {
        if self.config.groups.is_empty() {
            self.config.groups.push(crate::config::GroupConfig { name: "General".into(), collapsed: false });
        }
        let static_len = crate::components::ComponentKind::all_palette().len();
        let customs = self.glyph_registry.custom_components();
        let total = static_len + customs.len();
        let customs_len = customs.len();
        let mut rows = Vec::new();
        for (gi, group) in self.config.groups.iter().enumerate() {
            rows.push(EditorDisplayRow::GroupHeader { group_idx: gi });
            if !group.collapsed {
                for kind_idx in 0..total {
                    let key = if kind_idx < static_len {
                        crate::glyphs::kind_key(crate::components::ComponentKind::all_palette()[kind_idx]).to_string()
                    } else {
                        let ci = kind_idx - static_len;
                        if ci < customs_len {
                            // Re-borrow customs inside the loop
                            self.glyph_registry.custom_components()[ci].id.clone()
                        } else {
                            continue;
                        }
                    };
                    let comp_group = self.config.component_groups.get(&key).map(|s| s.as_str()).unwrap_or("General");
                    let in_this_group = if group.name == "General" {
                        let assigned = self.config.component_groups.get(&key);
                        match assigned {
                            None => true,
                            Some(name) => !self.config.groups.iter().any(|g| &g.name == name) || name == "General",
                        }
                    } else {
                        comp_group == group.name
                    };
                    if in_this_group {
                        rows.push(EditorDisplayRow::Component { kind_idx });
                    }
                }
            }
        }
        self.editor.display_rows = rows;
        if self.editor.display_idx >= self.editor.display_rows.len() {
            self.editor.display_idx = self.editor.display_rows.len().saturating_sub(1);
        }
        self.sync_editor_kind_idx_from_display();
    }

    pub(super) fn sync_editor_kind_idx_from_display(&mut self) {
        if let Some(EditorDisplayRow::Component { kind_idx }) =
            self.editor.display_rows.get(self.editor.display_idx)
        {
            self.editor.kind_idx = *kind_idx;
        }
    }

    // ── Palette navigation using display rows ─────────────────────────────────

    pub fn palette_display_up(&mut self) {
        if self.pal.display_idx > 0 {
            self.pal.display_idx -= 1;
            self.sync_palette_idx_from_display();
        }
    }

    pub fn palette_display_down(&mut self) {
        if self.pal.display_idx + 1 < self.pal.display_rows.len() {
            self.pal.display_idx += 1;
            self.sync_palette_idx_from_display();
        }
    }

    pub fn palette_display_home(&mut self) {
        self.pal.display_idx = 0;
        self.sync_palette_idx_from_display();
    }

    pub fn palette_display_end(&mut self) {
        if !self.pal.display_rows.is_empty() {
            self.pal.display_idx = self.pal.display_rows.len() - 1;
            self.sync_palette_idx_from_display();
        }
    }

    pub fn palette_display_page_up(&mut self) {
        self.pal.display_idx = self.pal.display_idx.saturating_sub(10);
        self.sync_palette_idx_from_display();
    }

    pub fn palette_display_page_down(&mut self) {
        if !self.pal.display_rows.is_empty() {
            self.pal.display_idx = (self.pal.display_idx + 10).min(self.pal.display_rows.len() - 1);
            self.sync_palette_idx_from_display();
        }
    }

    // ── Group management (palette) ────────────────────────────────────────────

    /// True if the display cursor is currently on a group header.
    pub fn palette_cursor_on_header(&self) -> bool {
        matches!(self.pal.display_rows.get(self.pal.display_idx),
            Some(crate::palette_state::PaletteDisplayRow::GroupHeader { .. }))
    }

    /// Returns the group index the cursor is on (header or the header above a component).
    #[allow(dead_code)]
    pub fn palette_cursor_group_idx(&self) -> Option<usize> {
        match self.pal.display_rows.get(self.pal.display_idx) {
            Some(crate::palette_state::PaletteDisplayRow::GroupHeader { group_idx }) => Some(*group_idx),
            Some(crate::palette_state::PaletteDisplayRow::Component { .. }) => {
                for i in (0..self.pal.display_idx).rev() {
                    if let Some(crate::palette_state::PaletteDisplayRow::GroupHeader { group_idx }) =
                        self.pal.display_rows.get(i)
                    {
                        return Some(*group_idx);
                    }
                }
                None
            }
            None => None,
        }
    }

    /// Toggle collapse/expand of the group at the current cursor.
    pub fn palette_toggle_group(&mut self) {
        let gi = match self.pal.display_rows.get(self.pal.display_idx) {
            Some(crate::palette_state::PaletteDisplayRow::GroupHeader { group_idx }) => *group_idx,
            _ => return,
        };
        if gi < self.config.groups.len() {
            self.config.groups[gi].collapsed = !self.config.groups[gi].collapsed;
            self.config.save();
            self.rebuild_display_rows();
        }
    }

    /// Open the group picker for the selected component (palette context).
    pub fn palette_open_group_picker(&mut self) {
        if let Some(crate::palette_state::PaletteDisplayRow::Component { flat_idx }) =
            self.pal.display_rows.get(self.pal.display_idx)
        {
            self.pal.group_picker_for_flat = Some(*flat_idx);
            self.pal.group_picker_idx = 0;
            self.pal.group_picker_active = true;
        }
    }

    /// Confirm group assignment from the picker.
    pub fn palette_confirm_group_pick(&mut self) {
        let Some(flat_idx) = self.pal.group_picker_for_flat else { return; };
        let num_groups = self.config.groups.len();
        let pick = self.pal.group_picker_idx;
        if pick >= num_groups {
            // "New Group" option selected — open text input for name.
            self.pal.group_picker_active = false;
            self.pal.group_picker_for_flat = Some(flat_idx);
            self.text_input.input_buffer.clear();
            self.text_input.input_mode = InputMode::EditingText(TextEditTarget::GroupAssign);
        } else {
            let group_name = self.config.groups[pick].name.clone();
            self.assign_component_to_group(flat_idx, group_name);
            self.pal.group_picker_active = false;
            self.pal.group_picker_for_flat = None;
        }
    }

    /// Cancel group picker.
    pub fn palette_cancel_group_picker(&mut self) {
        self.pal.group_picker_active = false;
        self.pal.group_picker_for_flat = None;
    }

    /// Move a component (by palette flat index) to a named group.
    /// Creates the group if it doesn't exist.
    pub fn assign_component_to_group(&mut self, flat_idx: usize, group_name: String) {
        if !self.config.groups.iter().any(|g| g.name == group_name) {
            self.config.groups.push(crate::config::GroupConfig { name: group_name.clone(), collapsed: false });
        }
        let key = self.component_key_for(flat_idx);
        if group_name == "General" {
            self.config.component_groups.remove(&key);
        } else {
            self.config.component_groups.insert(key, group_name);
        }
        self.config.save();
        self.rebuild_display_rows();
    }

    /// Assign editor component (by kind_idx) to a named group.
    pub fn assign_editor_component_to_group(&mut self, kind_idx: usize, group_name: String) {
        let static_len = crate::components::ComponentKind::all_palette().len();
        let key = if kind_idx < static_len {
            crate::glyphs::kind_key(crate::components::ComponentKind::all_palette()[kind_idx]).to_string()
        } else {
            let ci = kind_idx - static_len;
            let customs = self.glyph_registry.custom_components();
            if ci < customs.len() { customs[ci].id.clone() } else { return; }
        };
        if !self.config.groups.iter().any(|g| g.name == group_name) {
            self.config.groups.push(crate::config::GroupConfig { name: group_name.clone(), collapsed: false });
        }
        if group_name == "General" {
            self.config.component_groups.remove(&key);
        } else {
            self.config.component_groups.insert(key, group_name);
        }
        self.config.save();
        self.rebuild_editor_display_rows();
    }

    /// Begin creating a new group (opens text input).
    pub fn begin_new_group(&mut self) {
        self.text_input.input_buffer.clear();
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::NewGroupName);
    }

    /// Delete the group the cursor is on (General cannot be deleted; members move to General).
    pub fn palette_delete_group(&mut self) {
        let gi = match self.pal.display_rows.get(self.pal.display_idx) {
            Some(crate::palette_state::PaletteDisplayRow::GroupHeader { group_idx }) => *group_idx,
            _ => return,
        };
        if gi == 0 || gi >= self.config.groups.len() { return; }
        let name = self.config.groups[gi].name.clone();
        self.config.component_groups.retain(|_, v| v != &name);
        self.config.groups.remove(gi);
        self.config.save();
        self.rebuild_display_rows();
    }

    /// Delete the group the editor cursor is on.
    pub fn editor_delete_group(&mut self) {
        let gi = match self.editor.display_rows.get(self.editor.display_idx) {
            Some(EditorDisplayRow::GroupHeader { group_idx }) => *group_idx,
            _ => return,
        };
        if gi == 0 || gi >= self.config.groups.len() { return; }
        let name = self.config.groups[gi].name.clone();
        self.config.component_groups.retain(|_, v| v != &name);
        self.config.groups.remove(gi);
        self.config.save();
        self.rebuild_editor_display_rows();
    }

    /// Toggle collapse of the group the editor cursor is on.
    pub fn editor_toggle_group(&mut self) {
        let gi = match self.editor.display_rows.get(self.editor.display_idx) {
            Some(EditorDisplayRow::GroupHeader { group_idx }) => *group_idx,
            _ => return,
        };
        if gi < self.config.groups.len() {
            self.config.groups[gi].collapsed = !self.config.groups[gi].collapsed;
            self.config.save();
            self.rebuild_editor_display_rows();
        }
    }

    /// Open group picker for glyph editor component.
    pub fn editor_open_group_picker(&mut self) {
        if let Some(EditorDisplayRow::Component { kind_idx }) =
            self.editor.display_rows.get(self.editor.display_idx)
        {
            self.editor.group_picker_for_kind = Some(*kind_idx);
            self.editor.group_picker_idx = 0;
            self.editor.group_picker_active = true;
        }
    }

    pub fn editor_confirm_group_pick(&mut self) {
        let Some(kind_idx) = self.editor.group_picker_for_kind else { return; };
        let num_groups = self.config.groups.len();
        let pick = self.editor.group_picker_idx;
        if pick >= num_groups {
            self.editor.group_picker_active = false;
            self.text_input.input_buffer.clear();
            self.text_input.input_mode = InputMode::EditingText(TextEditTarget::GroupAssign);
            self.editor.group_picker_for_kind = Some(kind_idx);
        } else {
            let group_name = self.config.groups[pick].name.clone();
            self.assign_editor_component_to_group(kind_idx, group_name);
            self.editor.group_picker_active = false;
            self.editor.group_picker_for_kind = None;
        }
    }

    pub fn editor_cancel_group_picker(&mut self) {
        self.editor.group_picker_active = false;
        self.editor.group_picker_for_kind = None;
    }

    // ── Glyph editor display navigation ──────────────────────────────────────

    pub fn editor_display_nav(&mut self, delta: isize) {
        if self.editor.focus != GlyphEditorFocus::ComponentList { return; }
        let new_idx = self.editor.display_idx as isize + delta;
        if new_idx < 0 { return; }
        let new_idx = new_idx as usize;
        if new_idx < self.editor.display_rows.len() {
            let prev_kind = self.editor.kind_idx;
            self.editor.display_idx = new_idx;
            self.sync_editor_kind_idx_from_display();
            if !self.editor_selected_is_composite()
                && self.editor.focus == GlyphEditorFocus::CompositeGrid
            {
                self.editor.focus = GlyphEditorFocus::CharGrid;
            }
            if self.editor.kind_idx != prev_kind {
                self.editor.composite_cursor = (1, 1);
                self.editor.composite_viewport = (0, 0);
            }
        }
    }

    pub fn editor_display_home(&mut self) {
        if self.editor.focus != GlyphEditorFocus::ComponentList { return; }
        self.editor.display_idx = 0;
        self.sync_editor_kind_idx_from_display();
    }

    pub fn editor_display_end(&mut self) {
        if self.editor.focus != GlyphEditorFocus::ComponentList { return; }
        if !self.editor.display_rows.is_empty() {
            self.editor.display_idx = self.editor.display_rows.len() - 1;
            self.sync_editor_kind_idx_from_display();
        }
    }

    pub fn editor_display_page_up(&mut self) {
        if self.editor.focus != GlyphEditorFocus::ComponentList { return; }
        self.editor.display_idx = self.editor.display_idx.saturating_sub(10);
        self.sync_editor_kind_idx_from_display();
    }

    pub fn editor_display_page_down(&mut self) {
        if self.editor.focus != GlyphEditorFocus::ComponentList { return; }
        if !self.editor.display_rows.is_empty() {
            self.editor.display_idx =
                (self.editor.display_idx + 10).min(self.editor.display_rows.len() - 1);
            self.sync_editor_kind_idx_from_display();
        }
    }

    // ── Old palette navigation (kept for backward-compat callers) ────────────

    pub fn palette_up(&mut self) {
        if self.pal.palette_idx > 0 {
            self.pal.palette_idx -= 1;
        }
    }

    pub fn palette_down(&mut self) {
        if self.pal.palette_idx + 1 < self.pal.palette.len() {
            self.pal.palette_idx += 1;
        }
    }

    #[allow(dead_code)]
    pub fn palette_home(&mut self) {
        self.pal.palette_idx = 0;
    }

    #[allow(dead_code)]
    pub fn palette_end(&mut self) {
        if !self.pal.palette.is_empty() {
            self.pal.palette_idx = self.pal.palette.len() - 1;
        }
    }

    #[allow(dead_code)]
    pub fn palette_page_up(&mut self) {
        self.pal.palette_idx = self.pal.palette_idx.saturating_sub(10);
    }

    #[allow(dead_code)]
    pub fn palette_page_down(&mut self) {
        if !self.pal.palette.is_empty() {
            self.pal.palette_idx = (self.pal.palette_idx + 10).min(self.pal.palette.len() - 1);
        }
    }

    // ── Palette search ────────────────────────────────────────────────────────

    pub fn palette_item_matches(&self, idx: usize, query: &str) -> bool {
        let Some(kind) = self.pal.palette.get(idx) else { return false };
        if *kind == ComponentKind::Custom {
            let customs = self.glyph_registry.custom_components();
            let ci = self.pal.palette_custom_indices.get(idx).copied().flatten();
            if let Some(ci) = ci.filter(|&ci| ci < customs.len()) {
                return customs[ci].label.to_lowercase().contains(query);
            }
            return "custom comp".contains(query);
        }
        kind.label().to_lowercase().contains(query)
    }

    /// Jump palette_idx to the first palette item that matches the current search query.
    pub fn palette_search_jump_first(&mut self) {
        let query = self.pal.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.pal.palette.len();
        for i in 0..len {
            if self.palette_item_matches(i, &query) {
                self.pal.palette_idx = i;
                return;
            }
        }
    }

    /// Move palette_idx to the next matching item (wraps around).
    pub fn palette_search_next(&mut self) {
        let query = self.pal.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.pal.palette.len();
        for offset in 1..=len {
            let i = (self.pal.palette_idx + offset) % len;
            if self.palette_item_matches(i, &query) {
                self.pal.palette_idx = i;
                return;
            }
        }
    }

    /// Move palette_idx to the previous matching item (wraps around).
    pub fn palette_search_prev(&mut self) {
        let query = self.pal.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.pal.palette.len();
        for offset in 1..=len {
            let i = (self.pal.palette_idx + len - offset) % len;
            if self.palette_item_matches(i, &query) {
                self.pal.palette_idx = i;
                return;
            }
        }
    }

    // ── Help search ──────────────────────────────────────────────────────────

    /// Scroll help to the first line matching the current search query.
    pub fn help_search_jump_first(&mut self) {
        let query = self.help_search.to_lowercase();
        if query.is_empty() { return; }
        for (i, line) in self.help_lines.iter().enumerate() {
            if line.to_lowercase().contains(&query) {
                self.help_scroll = i;
                return;
            }
        }
    }

    /// Scroll help to the next matching line after help_scroll (wraps).
    pub fn help_search_next(&mut self) {
        let query = self.help_search.to_lowercase();
        if query.is_empty() { return; }
        let total = self.help_lines.len();
        for offset in 1..=total {
            let i = (self.help_scroll + offset) % total;
            if self.help_lines.get(i).map(|l| l.to_lowercase().contains(&query)).unwrap_or(false) {
                self.help_scroll = i;
                return;
            }
        }
    }

    /// Scroll help to the previous matching line before help_scroll (wraps).
    pub fn help_search_prev(&mut self) {
        let query = self.help_search.to_lowercase();
        if query.is_empty() { return; }
        let total = self.help_lines.len();
        for offset in 1..=total {
            let i = (self.help_scroll + total - offset) % total;
            if self.help_lines.get(i).map(|l| l.to_lowercase().contains(&query)).unwrap_or(false) {
                self.help_scroll = i;
                return;
            }
        }
    }

    // ── Color/material ────────────────────────────────────────────────────────

    /// Returns the active build color: custom_rgb if set, else palette selection.
    pub fn selected_build_color(&self) -> [u8; 3] {
        if let Some(rgb) = self.pal.build_custom_rgb {
            return rgb;
        }
        let (r, g, b, _) = COLOR_PALETTE[self.pal.build_color_cursor.min(COLOR_PALETTE.len() - 1)];
        [r, g, b]
    }

    /// Navigate the build-mode color palette grid.
    pub fn palette_color_nav(&mut self, dr: isize, dc: isize) {
        self.pal.build_custom_rgb = None;
        let total = COLOR_PALETTE.len();
        let cols  = COLOR_PALETTE_COLS as isize;
        let rows  = total.div_ceil(COLOR_PALETTE_COLS) as isize;
        let row = (self.pal.build_color_cursor as isize / cols + dr).rem_euclid(rows);
        let col = (self.pal.build_color_cursor as isize % cols + dc).rem_euclid(cols);
        self.pal.build_color_cursor = ((row * cols + col) as usize).min(total - 1);
    }

    pub fn palette_begin_custom_rgb(&mut self) {
        let [r, g, b] = self.selected_build_color();
        self.text_input.input_buffer = format!("{r},{g},{b}");
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::BuildCustomRgb);
        self.status_msg = "Custom RGB (R,G,B):".into();
    }

    pub fn cycle_diameter(&mut self) {
        self.pal.selected_diameter = self.pal.selected_diameter.cycle();
        self.status_msg = format!("Diameter: {}", self.pal.selected_diameter.label());
    }

    pub fn nav_material(&mut self, delta: isize) {
        use crate::components::PipeMaterial::*;
        let mats = [Copper, PEX, PE, GalvanizedIron, BlackPlastic, CastIron];
        let cur = mats.iter().position(|&m| m == self.pal.selected_material).unwrap_or(0);
        let next = (cur as isize + delta).rem_euclid(mats.len() as isize) as usize;
        self.set_material_by_index(next);
    }

    pub fn nav_material_home(&mut self) {
        self.set_material_by_index(0);
    }

    pub fn nav_material_end(&mut self) {
        use crate::components::PipeMaterial::*;
        let mats = [Copper, PEX, PE, GalvanizedIron, BlackPlastic, CastIron];
        self.set_material_by_index(mats.len() - 1);
    }

    pub fn set_material_by_index(&mut self, idx: usize) {
        use crate::components::PipeMaterial::*;
        let mats = [Copper, PEX, PE, GalvanizedIron, BlackPlastic, CastIron];
        if let Some(&mat) = mats.get(idx) {
            self.pal.selected_material = mat;
            self.status_msg = format!("Material: {}", mat.label());
            let (r, c) = self.canvas.cursor;
            let (ar, ac) = self.canvas.grid.effective_pos(r, c);
            if let Some(comp) = self.canvas.grid.get_mut(ar, ac) {
                comp.material = mat;
                self.refresh_sim();
            }
        }
    }

    pub fn adjust_palette_kind_length(&mut self, delta_in: f32) {
        let kind = self.selected_component_kind();
        if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
            let current_in = self.pal.default_lengths.get(&kind).copied().unwrap_or(1.0) * 12.0;
            let new_in = (current_in + delta_in).max(1.0);
            self.pal.default_lengths.insert(kind, new_in / 12.0);
            self.status_msg = format!(
                "Default {} length: {} in ({:.2} ft)",
                kind.label(),
                new_in.round() as i32,
                new_in / 12.0,
            );
        }
    }
}
