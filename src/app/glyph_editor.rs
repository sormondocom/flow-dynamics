use super::*;

// ── Free functions ────────────────────────────────────────────────────────────

/// Build a `CustomCompDef` that mirrors a built-in standard component.
///
/// For composite standards the returned def has the same canvas footprint
/// (composite_size = (fw-2, fh-2)) and ports derived from the kind's
/// connections.  For fh==3 composites (inner_h==1) the single interior row
/// is pre-filled with the new label text so it renders legibly instead of
/// showing top-border box chars.
pub(super) fn snapshot_standard_as_custom(
    kind: ComponentKind,
    new_id: String,
    new_label: String,
    glyph: crate::glyphs::GlyphDef,
) -> crate::glyphs::CustomCompDef {
    use crate::glyphs::{CustomCompDef, CustomPort, PortKind};

    let mut def = CustomCompDef::new(new_id, new_label.clone(), glyph);
    def.equiv_length_d = kind.equiv_length_diameters();

    let (cn, cs, ce, cw) = kind.connections();

    if !kind.is_composite() {
        def.connections_nsew = [cn, cs, ce, cw];
        return def;
    }

    let (fw, fh) = kind.footprint();
    // composite_size = canvas dims directly (same as standard footprint, no extra buffer)
    def.composite_size = Some((fw, fh));
    let port_row = fh / 2;

    // East/West ports at canvas edges (dc=0 west, dc=fw-1 east)
    if cw {
        def.ports.push(CustomPort { name: "inlet_w".into(), kind: PortKind::Inlet,  row: port_row, col: 0 });
    }
    if ce {
        def.ports.push(CustomPort { name: "outlet_e".into(), kind: PortKind::Outlet, row: port_row, col: fw - 1 });
    }

    // BasinSink: north inlet + south drain (standard E/W connections are false)
    if kind == ComponentKind::BasinSink {
        def.ports.clear();
        let mid = fw / 2;
        def.ports.push(CustomPort { name: "inlet_n".into(), kind: PortKind::Inlet, row: 0,      col: mid });
        def.ports.push(CustomPort { name: "drain_s".into(), kind: PortKind::Drain, row: fh - 1, col: mid });
    }

    // For fh==3 (a single interior row at dr=1) pre-fill with label text so the copy
    // shows something legible instead of the default box-char '═'.
    if fh == 3 {
        let avail = fw.saturating_sub(2); // cols between west and east borders
        let padded: String = new_label.chars().chain(std::iter::repeat(' ')).take(avail).collect();
        for (i, ch) in padded.chars().enumerate() {
            def.set_cell(port_row, i + 1, ch); // dc=1 is the first interior cell
        }
    }

    def
}

pub(super) fn parse_composite_size(s: &str) -> (usize, usize) {
    let s = s.trim();
    if let Some((wstr, hstr)) = s.split_once(['x', 'X']) {
        let w = wstr.trim().parse::<usize>().unwrap_or(0);
        let h = hstr.trim().parse::<usize>().unwrap_or(0);
        (w, h)
    } else {
        let w = s.parse::<usize>().unwrap_or(0);
        (w, 3) // default height
    }
}

pub(super) fn parse_override_key(key: &str) -> Option<(usize, usize)> {
    let (r, c) = key.split_once(',')?;
    Some((r.parse().ok()?, c.parse().ok()?))
}

pub(super) fn shift_composite_content(
    def: &mut crate::glyphs::CustomCompDef,
    dr_offset: isize,
    dc_offset: isize,
) {
    use crate::glyphs::CustomCompDef;
    let old_overrides = std::mem::take(&mut def.cell_overrides);
    for (key, val) in old_overrides {
        if let Some((r, c)) = parse_override_key(&key) {
            let nr = (r as isize + dr_offset) as usize;
            let nc = (c as isize + dc_offset) as usize;
            def.cell_overrides.insert(CustomCompDef::override_key(nr, nc), val);
        }
    }
    let old_colors = std::mem::take(&mut def.cell_color_overrides);
    for (key, val) in old_colors {
        if let Some((r, c)) = parse_override_key(&key) {
            let nr = (r as isize + dr_offset) as usize;
            let nc = (c as isize + dc_offset) as usize;
            def.cell_color_overrides.insert(CustomCompDef::override_key(nr, nc), val);
        }
    }
    for port in &mut def.ports {
        port.row = (port.row as isize + dr_offset) as usize;
        port.col = (port.col as isize + dc_offset) as usize;
    }
}

pub(super) fn trim_composite(def: &mut crate::glyphs::CustomCompDef) {
    let (canvas_w, canvas_h) = match def.composite_size {
        Some(s) => s,
        None => return,
    };
    let fw = canvas_w;
    let fh = canvas_h;

    let mut min_r = usize::MAX;
    let mut max_r = 0usize;
    let mut min_c = usize::MAX;
    let mut max_c = 0usize;
    let mut has_content = false;

    for key in def.cell_overrides.keys() {
        if let Some((r, c)) = parse_override_key(key) {
            min_r = min_r.min(r); max_r = max_r.max(r);
            min_c = min_c.min(c); max_c = max_c.max(c);
            has_content = true;
        }
    }
    for port in &def.ports {
        min_r = min_r.min(port.row); max_r = max_r.max(port.row);
        min_c = min_c.min(port.col); max_c = max_c.max(port.col);
        has_content = true;
    }

    if !has_content {
        def.composite_size = Some((3, 3));
        return;
    }

    // Content at the border edge (dc==0 or dc==fw-1) needs 0 extra padding;
    // content in the interior needs 1 cell of padding (a border cell).
    let left_pad  = if min_c == 0       { 0usize } else { 1 };
    let right_pad = if max_c + 1 == fw  { 0usize } else { 1 };
    let top_pad   = if min_r == 0       { 0usize } else { 1 };
    let bot_pad   = if max_r + 1 == fh  { 0usize } else { 1 };

    let new_fw = left_pad + (max_c - min_c + 1) + right_pad;
    let new_fh = top_pad  + (max_r - min_r + 1) + bot_pad;
    let new_canvas_w = new_fw.max(3);
    let new_canvas_h = new_fh.max(3);

    let dc_offset = left_pad as isize - min_c as isize;
    let dr_offset = top_pad  as isize - min_r as isize;

    if dc_offset != 0 || dr_offset != 0 {
        shift_composite_content(def, dr_offset, dc_offset);
    }
    def.composite_size = Some((new_canvas_w, new_canvas_h));
}

// ── impl App ──────────────────────────────────────────────────────────────────

impl App {
    pub fn enter_glyph_editor(&mut self) {
        self.mode = AppMode::GlyphEditor;
        self.editor.status =
            "  [Tab] switch panel  [Enter] apply  [M] mat scope  [D] diam scope  \
             [N] new  [R] rename  [C] copy  [W] composite  [S] save  [L] load  [G/Q] exit"
                .into();
        self.rebuild_editor_display_rows();
    }

    pub fn exit_glyph_editor(&mut self) {
        self.mode = AppMode::Build;
        self.status_msg = "Glyph editor closed.".into();
    }

    /// Apply the currently selected char + color as an override (or composite cell placement).
    pub fn editor_apply_glyph(&mut self) {
        let static_len = ComponentKind::all_palette().len();

        // CompositeGrid focus: place selected char into the tile under the cursor.
        // Cursor is in display space (with +1 visual buffer offset); data = cursor - 1.
        if self.editor.focus == GlyphEditorFocus::CompositeGrid {
            let ci = self.editor.kind_idx.saturating_sub(static_len);
            if ci < self.glyph_registry.library.custom_components.len() {
                let (display_r, display_c) = self.editor.composite_cursor;
                if display_r == 0 || display_c == 0 { return; } // on visual buffer
                let data_r = display_r - 1;
                let data_c = display_c - 1;
                let ch = self.editor.current_symbol();
                let color = self.editor.current_color();
                self.glyph_registry.library.custom_components[ci].set_cell(data_r, data_c, ch);
                self.glyph_registry.library.custom_components[ci].set_cell_color(data_r, data_c, color);
                self.editor.status = format!("Placed '{ch}' at ({data_r},{data_c}).");
            }
            return;
        }

        let glyph = GlyphDef {
            symbol: self.editor.current_symbol(),
            fg: self.editor.current_color(),
        };
        if self.editor.kind_idx < static_len {
            let kind = ComponentKind::all_palette()[self.editor.kind_idx];
            let mat_opt = self.editor.mat_scope.map(|i| ALL_MATERIALS[i]);
            let diam_opt = self.editor.diam_scope.map(|i| ALL_DIAMETERS[i]);
            self.glyph_registry.set_override(kind, diam_opt, mat_opt, glyph);
            self.editor.status = format!(
                "Applied '{}' to {}  [{}  {}]",
                self.editor.current_symbol(),
                kind.label(),
                self.editor.mat_label(),
                self.editor.diam_label(),
            );
        } else {
            let ci = self.editor.kind_idx - static_len;
            if ci < self.glyph_registry.library.custom_components.len() {
                let label = self.glyph_registry.library.custom_components[ci].label.clone();
                self.glyph_registry.library.custom_components[ci].glyph = glyph;
                self.rebuild_palette();
                self.editor.status = format!("Updated glyph for custom component '{label}'.");
            }
        }
    }

    pub fn editor_clear_composite_cell(&mut self) {
        if self.editor.focus != GlyphEditorFocus::CompositeGrid { return; }
        let static_len = ComponentKind::all_palette().len();
        let ci = self.editor.kind_idx.saturating_sub(static_len);
        if ci < self.glyph_registry.library.custom_components.len() {
            let (display_r, display_c) = self.editor.composite_cursor;
            if display_r == 0 || display_c == 0 { return; }
            let data_r = display_r - 1;
            let data_c = display_c - 1;
            self.glyph_registry.library.custom_components[ci].clear_cell(data_r, data_c);
            self.glyph_registry.library.custom_components[ci].clear_cell_color(data_r, data_c);
            self.editor.status = format!("Cleared cell ({data_r},{data_c}) — reverted to default.");
        }
    }

    pub fn editor_set_port(&mut self, kind: crate::glyphs::PortKind) {
        if self.editor.focus != GlyphEditorFocus::CompositeGrid { return; }
        let static_len = ComponentKind::all_palette().len();
        let ci = self.editor.kind_idx.saturating_sub(static_len);
        if ci >= self.glyph_registry.library.custom_components.len() { return; }
        let (display_r, display_c) = self.editor.composite_cursor;
        if display_r == 0 || display_c == 0 { return; }
        let data_r = display_r - 1;
        let data_c = display_c - 1;
        let (canvas_w, canvas_h) = match self.glyph_registry.library.custom_components[ci].composite_size {
            Some(s) => s,
            None => {
                self.editor.status = "Not a composite component.".into();
                return;
            }
        };
        let def = &mut self.glyph_registry.library.custom_components[ci];
        let msg = def.set_port(data_r, data_c, canvas_w, canvas_h, kind);
        self.editor.status = format!("({data_r},{data_c}): {msg}");
    }

    pub fn editor_nav(&mut self, dr: isize, dc: isize) {
        match self.editor.focus {
            GlyphEditorFocus::ComponentList => {
                // Use display-row navigation (handles group headers).
                self.editor_display_nav(dr);
            }
            GlyphEditorFocus::CompositeGrid => {
                let static_len = ComponentKind::all_palette().len();
                let ci = self.editor.kind_idx.saturating_sub(static_len);
                if ci >= self.glyph_registry.library.custom_components.len() { return; }
                let (canvas_w, canvas_h) = match self.glyph_registry.library.custom_components[ci].composite_size {
                    Some(s) => s,
                    None => return,
                };
                // Display adds +2 visual buffer ring around the canvas area.
                // Display range: dr=0..display_fh-1, dc=0..display_fw-1.
                // Valid edit range: dr=1..canvas_h, dc=1..canvas_w (= canvas dc 0..canvas_w-1).
                let display_fw = canvas_w + 2;
                let display_fh = canvas_h + 2;
                let (cur_r, cur_c) = self.editor.composite_cursor;
                let new_r = cur_r as isize + dr;
                let new_c = cur_c as isize + dc;
                const MAX_CANVAS: usize = 60; // max canvas_w/canvas_h

                if new_c >= display_fw as isize - 1 && canvas_w < MAX_CANVAS {
                    // Expand east: cursor to new east display border
                    self.glyph_registry.library.custom_components[ci].composite_size = Some((canvas_w + 1, canvas_h));
                    self.editor.composite_cursor = (cur_r, display_fw - 1);
                } else if new_c <= 0 && canvas_w < MAX_CANVAS {
                    // Expand west: shift data right by 1, cursor stays at display dc=1
                    shift_composite_content(&mut self.glyph_registry.library.custom_components[ci], 0, 1);
                    self.glyph_registry.library.custom_components[ci].composite_size = Some((canvas_w + 1, canvas_h));
                    self.editor.composite_cursor = (cur_r, 1);
                } else if new_r >= display_fh as isize - 1 && canvas_h < MAX_CANVAS {
                    // Expand south
                    self.glyph_registry.library.custom_components[ci].composite_size = Some((canvas_w, canvas_h + 1));
                    self.editor.composite_cursor = (display_fh - 1, cur_c);
                } else if new_r <= 0 && canvas_h < MAX_CANVAS {
                    // Expand north: shift data down by 1, cursor stays at display dr=1
                    shift_composite_content(&mut self.glyph_registry.library.custom_components[ci], 1, 0);
                    self.glyph_registry.library.custom_components[ci].composite_size = Some((canvas_w, canvas_h + 1));
                    self.editor.composite_cursor = (1, cur_c);
                } else {
                    // Clamp to valid display edit range [1, canvas_w] × [1, canvas_h]
                    let clamped_r = new_r.max(1).min(canvas_h as isize) as usize;
                    let clamped_c = new_c.max(1).min(canvas_w as isize) as usize;
                    self.editor.composite_cursor = (clamped_r, clamped_c);
                }

                // Scroll viewport to keep cursor visible (rough 20×40 assumed visible area)
                let (cr, cc) = self.editor.composite_cursor;
                let (vr, vc) = &mut self.editor.composite_viewport;
                const VH: usize = 20;
                const VW: usize = 40;
                if cr < *vr { *vr = cr; }
                else if cr >= *vr + VH { *vr = cr + 1 - VH; }
                if cc < *vc { *vc = cc; }
                else if cc >= *vc + VW { *vc = cc + 1 - VW; }
            }
            GlyphEditorFocus::CharGrid    => self.editor.nav_char(dr, dc),
            GlyphEditorFocus::ColorPicker => self.editor.nav_color(dr, dc),
        }
    }

    pub fn editor_nav_home(&mut self) {
        if self.editor.focus == GlyphEditorFocus::ComponentList {
            self.editor_display_home();
        }
    }

    pub fn editor_nav_end(&mut self) {
        if self.editor.focus == GlyphEditorFocus::ComponentList {
            self.editor_display_end();
        }
    }

    pub fn editor_cycle_focus(&mut self) {
        let is_composite = self.editor_selected_is_composite();
        self.editor.focus = match (&self.editor.focus, is_composite) {
            (GlyphEditorFocus::ComponentList, true)  => GlyphEditorFocus::CompositeGrid,
            (GlyphEditorFocus::ComponentList, false) => GlyphEditorFocus::CharGrid,
            (GlyphEditorFocus::CompositeGrid, _)     => GlyphEditorFocus::CharGrid,
            (GlyphEditorFocus::CharGrid, _)          => GlyphEditorFocus::ColorPicker,
            (GlyphEditorFocus::ColorPicker, _)       => GlyphEditorFocus::ComponentList,
        };
    }

    pub(super) fn editor_selected_is_composite(&self) -> bool {
        let static_len = ComponentKind::all_palette().len();
        if self.editor.kind_idx < static_len { return false; }
        let ci = self.editor.kind_idx - static_len;
        let customs = self.glyph_registry.custom_components();
        ci < customs.len() && customs[ci].composite_size.is_some()
    }

    pub fn editor_begin_save(&mut self) {
        let path = self
            .glyph_registry
            .library_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("glyphs.json")
            .to_string();
        self.text_input.input_buffer = path;
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::SaveLibrary);
    }

    pub fn editor_begin_load(&mut self) {
        let path = self
            .glyph_registry
            .library_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("glyphs.json")
            .to_string();
        self.text_input.input_buffer = path;
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::LoadLibrary);
    }

    pub fn editor_begin_new_comp(&mut self) {
        self.text_input.input_buffer.clear();
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::NewCompName);
    }

    pub fn editor_begin_rename_comp(&mut self) {
        let static_len = ComponentKind::all_palette().len();
        if self.editor.kind_idx < static_len {
            self.editor.status = "Built-in components can't be renamed — use [C] to copy it as an editable custom component.".into();
            return;
        }
        let ci = self.editor.kind_idx - static_len;
        let customs = self.glyph_registry.custom_components();
        if ci >= customs.len() {
            self.editor.status = "No custom component selected.".into();
            return;
        }
        self.text_input.input_buffer = customs[ci].label.clone();
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::RenameComp);
    }

    pub fn editor_begin_copy_comp(&mut self) {
        let static_len = ComponentKind::all_palette().len();
        let source_label = if self.editor.kind_idx < static_len {
            ComponentKind::all_palette()[self.editor.kind_idx].label().to_string()
        } else {
            let ci = self.editor.kind_idx - static_len;
            let customs = self.glyph_registry.custom_components();
            if ci >= customs.len() {
                self.editor.status = "No component selected.".into();
                return;
            }
            customs[ci].label.clone()
        };
        self.text_input.input_buffer = format!("{source_label} Copy");
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::CopyComp);
    }

    /// First [Del] press — arms the pending-delete prompt.
    pub fn editor_delete_custom_comp(&mut self) {
        let static_len = ComponentKind::all_palette().len();
        if self.editor.kind_idx < static_len {
            self.editor.status = "Built-in components can't be deleted. Use [C] to copy as a custom component.".into();
            return;
        }
        let ci = self.editor.kind_idx - static_len;
        if ci >= self.glyph_registry.library.custom_components.len() {
            self.editor.status = "No custom component selected.".into();
            return;
        }
        let name = &self.glyph_registry.library.custom_components[ci].label;
        self.editor.status = format!("Delete '{name}'?  [Y] confirm  [N / any key] cancel");
        self.dialog.editor_pending_delete = Some(ci);
    }

    /// [Y] after the delete prompt — executes the deletion.
    pub fn editor_confirm_delete_comp(&mut self) {
        let Some(ci) = self.dialog.editor_pending_delete.take() else { return };
        let static_len = ComponentKind::all_palette().len();
        let customs = &mut self.glyph_registry.library.custom_components;
        if ci >= customs.len() {
            self.editor.status = "Component no longer exists.".into();
            return;
        }
        let name = customs[ci].label.clone();
        customs.remove(ci);
        let new_len = self.glyph_registry.library.custom_components.len();
        self.editor.kind_idx = if new_len == 0 {
            static_len.saturating_sub(1)
        } else {
            (static_len + ci).min(static_len + new_len - 1)
        };
        self.rebuild_palette();
        self.editor.status = format!("Deleted '{name}'. Press [S] to save the library.");
    }

    /// Any non-Y key during the delete prompt — cancels without deleting.
    pub fn editor_cancel_delete_comp(&mut self) {
        self.dialog.editor_pending_delete = None;
        self.editor.status = "Delete cancelled.".into();
    }

    pub fn editor_begin_set_composite_width(&mut self) {
        let static_len = ComponentKind::all_palette().len();
        if self.editor.kind_idx < static_len {
            self.editor.status = "Select a custom component first (navigate to it in the list).".into();
            return;
        }
        let ci = self.editor.kind_idx - static_len;
        let customs = self.glyph_registry.custom_components();
        if ci >= customs.len() {
            self.editor.status = "No custom component selected.".into();
            return;
        }
        self.text_input.input_buffer = match customs[ci].composite_size {
            Some((w, h)) => format!("{w}x{h}"),
            None         => String::new(),
        };
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::CompWidth);
    }

    pub fn editor_cycle_mat_scope(&mut self) {
        self.editor.cycle_mat_scope();
    }

    pub fn editor_cycle_diam_scope(&mut self) {
        self.editor.cycle_diam_scope();
    }

    pub fn editor_begin_custom_rgb(&mut self) {
        let [r, g, b] = self.editor.current_color();
        self.text_input.input_buffer = format!("{r},{g},{b}");
        self.text_input.input_mode = InputMode::EditingText(TextEditTarget::CustomRgb);
        self.status_msg = "Custom RGB (0-255 each):".into();
    }
}
