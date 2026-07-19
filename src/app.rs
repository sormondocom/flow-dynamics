use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::assembly::{Assembly, AssemblyLibrary};
use crate::components::{Component, ComponentKind, PipeDiameter, PipeMaterial};
use crate::config::AppConfig;
use crate::file_dialog::{FileDialogMode, FileDialogPurpose, FileDialogState};
use crate::fluid::FluidType;
use crate::glyphs::{
    CustomCompDef, GlyphDef, GlyphEditorFocus, GlyphEditorState, GlyphRegistry,
    ALL_DIAMETERS, ALL_MATERIALS, COLOR_PALETTE, COLOR_PALETTE_COLS,
};
use crate::grid::Grid;
use crate::simulation::{simulate, NodeFlowData, SimResult};

pub const GRID_COLS_MIN: usize = 79;
pub const GRID_ROWS_MIN: usize = 40;
const UNDO_MAX: usize = 50;

/// Parse a composite size string: "WxH", "W" (height defaults to 3), or "0"/"" (single-cell).
/// Returns (w, h); caller should reject if either < 3.
/// Build a `CustomCompDef` that mirrors a built-in standard component.
///
/// For composite standards the returned def has the same canvas footprint
/// (composite_size = (fw-2, fh-2)) and ports derived from the kind's
/// connections.  For fh==3 composites (inner_h==1) the single interior row
/// is pre-filled with the new label text so it renders legibly instead of
/// showing top-border box chars.
fn snapshot_standard_as_custom(
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

fn parse_composite_size(s: &str) -> (usize, usize) {
    let s = s.trim();
    if let Some((wstr, hstr)) = s.split_once(|c| c == 'x' || c == 'X') {
        let w = wstr.trim().parse::<usize>().unwrap_or(0);
        let h = hstr.trim().parse::<usize>().unwrap_or(0);
        (w, h)
    } else {
        let w = s.parse::<usize>().unwrap_or(0);
        (w, 3) // default height
    }
}

fn parse_override_key(key: &str) -> Option<(usize, usize)> {
    let (r, c) = key.split_once(',')?;
    Some((r.parse().ok()?, c.parse().ok()?))
}

fn shift_composite_content(
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

fn trim_composite(def: &mut crate::glyphs::CustomCompDef) {
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

// ── App mode ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Splash,
    Build,
    Simulating,
    Paused,
    GlyphEditor,
    BomView,
    Selecting,
    AssemblyBrowser,
    Stamping,
    ComponentDetail,
    Help,
    Settings,
    FileDialog,
    ConfirmNew,
    ConfirmQuit,
    ExportDialog,
    /// Dedicated popup for entering Label or Note text.
    AnnotationDialog,
}

// ── Focus ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Canvas,
    Palette,       // component list
    PaletteColors, // material + color-swatch section
}

// ── Text input mode ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEditTarget {
    /// Filename for saving the current glyph library.
    SaveLibrary,
    /// Filename for loading a glyph library.
    LoadLibrary,
    /// Name for a new custom component.
    NewCompName,
    /// Composite width for a custom component in the glyph editor.
    CompWidth,
    /// Name for a new assembly being saved from selection.
    AssemblyName,
    /// File path being added to the glyph-file auto-load list in Settings.
    AddGlyphFile,
    /// Custom RGB value entered in the glyph editor color picker ("R,G,B").
    CustomRgb,
    /// Custom RGB value entered from the build-mode palette color picker.
    BuildCustomRgb,
    /// Text for a Label annotation being placed at the cursor.
    LabelText,
    /// Text for a Note annotation being placed at the cursor (Shift+Enter = line break).
    NoteText,
    /// New name for renaming an existing custom component.
    RenameComp,
    /// Name for a clone of the currently selected custom component.
    CopyComp,
    /// Exact PSI value typed directly into the source pressure dialog.
    SourcePressure,
    /// File path for a Link annotation being placed or edited.
    LinkPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EditingLength,
    EditingText(TextEditTarget),
}

// ── Application state ─────────────────────────────────────────────────────────

pub struct App {
    pub mode: AppMode,
    pub focus: Focus,
    pub grid: Grid,
    pub cursor: (usize, usize),
    pub viewport: (usize, usize),
    pub palette: Vec<ComponentKind>,
    pub palette_idx: usize,
    /// For each palette slot that is ComponentKind::Custom, stores the index into
    /// glyph_registry.custom_components(). None means the generic placeholder.
    pub palette_custom_indices: Vec<Option<usize>>,
    pub selected_diameter: PipeDiameter,
    pub selected_material: PipeMaterial,
    pub sim_result: Option<SimResult>,
    pub tick: u64,
    pub should_quit: bool,
    pub status_msg: String,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub glyph_registry: GlyphRegistry,
    pub editor: GlyphEditorState,
    pub layout_path: Option<PathBuf>,
    pub fluid_type: FluidType,
    pub show_annotations: bool,
    pub pre_bom_mode: AppMode,
    /// Per-kind default pipe length (ft) applied to newly placed PipeH/PipeV segments.
    pub default_lengths: HashMap<ComponentKind, f32>,
    /// Anchor corner of the active selection rectangle.
    pub select_start: Option<(usize, usize)>,
    /// Assembly library (loaded from / saved to assemblies.json).
    pub assembly_lib: AssemblyLibrary,
    pub assembly_path: Option<PathBuf>,
    /// Cursor index in the assembly browser list.
    pub assembly_idx: usize,
    /// Assembly currently being positioned for stamping.
    pub pending_stamp: Option<Assembly>,
    /// When Some, the pending stamp is a "move": clear this rect from the grid on confirm.
    pub stamp_cut_rect: Option<(usize, usize, usize, usize)>,
    /// Mode to restore when closing the browser or cancelling a stamp.
    pub pre_assembly_mode: AppMode,
    /// Default arm-stub lengths (ft) per kind and port; applied on component placement.
    pub default_arm_lengths: HashMap<ComponentKind, [f32; 4]>,
    /// Index into the active-ports list while the ComponentDetail overlay is open.
    pub detail_port_cursor: usize,
    /// Mode to restore when closing the ComponentDetail overlay.
    pub pre_detail_mode: AppMode,
    /// Which component kind the ComponentDetail overlay is currently editing.
    pub detail_kind: ComponentKind,
    /// True when the overlay is editing palette defaults; false when editing a placed component.
    pub detail_for_palette: bool,
    /// Layout loaded from splash.json, displayed on the splash screen.
    pub splash_grid: Option<Grid>,
    /// Simulation result for the splash grid — computed once at startup.
    pub splash_sim: Option<SimResult>,
    /// Lines loaded from help.txt (hot-reloaded each time help opens).
    pub help_lines: Vec<String>,
    /// Scroll offset within the help screen.
    pub help_scroll: usize,
    /// Mode to restore when closing the help screen.
    pub pre_help_mode: AppMode,
    /// Active file-browser dialog state.
    pub file_dialog: Option<FileDialogState>,
    /// Mode to restore when the file dialog is cancelled.
    pub pre_dialog_mode: AppMode,
    /// Custom-component index awaiting delete confirmation in the glyph editor (None = no pending delete).
    pub editor_pending_delete: Option<usize>,
    /// Selected option in the "unsaved changes" prompt (0=Save 1=Discard 2=Cancel).
    pub confirm_new_choice: usize,
    /// Selected option in the quit confirmation prompt (0=Save & Quit 1=Quit 2=Cancel).
    pub confirm_quit_choice: usize,
    /// Mode to restore if the quit confirmation is cancelled.
    pub pre_quit_mode: AppMode,
    /// Persistent application configuration (glyph auto-load list, etc.)
    pub config: AppConfig,
    /// Cursor index in the settings screen glyph-file list.
    pub settings_idx: usize,
    /// Status from the last attempted glyph-file load in settings.
    pub settings_status: String,
    /// Index into COLOR_PALETTE for the build-mode color picker (used by SolidBlock etc.)
    pub build_color_cursor: usize,
    /// Custom RGB override for the build-mode color picker (set via [E] in palette).
    pub build_custom_rgb: Option<[u8; 3]>,
    /// Undo history: grid snapshots before each mutation, oldest-first. Capped at UNDO_MAX.
    pub undo_stack: Vec<crate::grid::Grid>,
    /// Redo history: grid snapshots popped from undo_stack during undo.
    pub redo_stack: Vec<crate::grid::Grid>,
    /// Annotation waiting to be placed: (kind, text). Set after the dialog confirms.
    pub pending_annotation: Option<(ComponentKind, String)>,
    /// When editing an existing annotation, the grid position to update in place.
    pub edit_annotation_pos: Option<(usize, usize)>,
    /// Path stored when following a Link requires a save-before-switch confirmation.
    pub pending_link_path: Option<String>,
    /// Byte offset of the cursor within input_buffer during NoteText editing.
    pub note_cursor_pos: usize,
    /// First visible row in the 3-row note text area.
    pub note_scroll_row: usize,
    /// First visible column in the note text area (horizontal scroll).
    pub note_scroll_col: usize,
    /// Current search query typed with [/] in the component palette.
    pub palette_search: String,
    /// Whether the palette search bar is active.
    pub palette_search_active: bool,
    /// Current search query typed with [/] in the help screen.
    pub help_search: String,
    /// Whether the help search bar is active.
    pub help_search_active: bool,
}

impl App {
    pub fn new(grid_cols: usize, grid_rows: usize) -> Self {
        let splash_grid = Self::try_load_splash();
        let splash_sim  = splash_grid.as_ref().map(|g| simulate(g, FluidType::default(), &GlyphRegistry::new()));
        let mut app = Self {
            mode: AppMode::Splash,
            focus: Focus::Canvas,
            grid: Grid::new(grid_cols, grid_rows),
            cursor: (0, 0),
            viewport: (0, 0),
            palette: Vec::new(),
            palette_idx: 2, // Start with PipeH
            palette_custom_indices: Vec::new(),
            selected_diameter: PipeDiameter::ThreeQuarter,
            selected_material: PipeMaterial::Copper,
            sim_result: None,
            tick: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            should_quit: false,
            status_msg: String::new(),
            glyph_registry: GlyphRegistry::new(),
            editor: GlyphEditorState::default(),
            layout_path: None,
            fluid_type: FluidType::default(),
            show_annotations: false,
            pre_bom_mode: AppMode::Build,
            default_lengths: {
                let mut m = HashMap::new();
                m.insert(ComponentKind::PipeH, 1.0_f32);
                m.insert(ComponentKind::PipeV, 1.0_f32);
                m
            },
            select_start: None,
            assembly_lib: Self::try_load_assemblies(),
            assembly_path: Some(PathBuf::from("assemblies.json")),
            assembly_idx: 0,
            pending_stamp: None,
            stamp_cut_rect: None,
            pre_assembly_mode: AppMode::Build,
            default_arm_lengths: HashMap::new(),
            detail_port_cursor: 0,
            pre_detail_mode: AppMode::Build,
            detail_kind: ComponentKind::PipeH,
            detail_for_palette: false,
            splash_grid,
            splash_sim,
            help_lines: Self::try_load_help(),
            help_scroll: 0,
            pre_help_mode: AppMode::Build,
            file_dialog: None,
            pre_dialog_mode: AppMode::Build,
            editor_pending_delete: None,
            confirm_new_choice: 0,
            confirm_quit_choice: 0,
            pre_quit_mode: AppMode::Build,
            config: AppConfig::default(),
            settings_idx: 0,
            settings_status: String::new(),
            build_color_cursor: 11, // index of "Gray" in COLOR_PALETTE row 1
            build_custom_rgb: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_annotation: None,
            edit_annotation_pos: None,
            pending_link_path: None,
            note_cursor_pos: 0,
            note_scroll_row: 0,
            note_scroll_col: 0,
            palette_search: String::new(),
            palette_search_active: false,
            help_search: String::new(),
            help_search_active: false,
        };
        app.rebuild_palette();
        app
    }

    /// Load config and auto-load any configured glyph files.
    /// Called from main() after App::new(), before the event loop.
    pub fn load_config(&mut self) {
        self.config = AppConfig::load();
        for path in self.config.glyph_files.clone() {
            let _ = self.try_load_glyph_file(&path);
        }
    }

    /// Attempt to load a glyph library from `path` into the current registry.
    /// Returns Ok(()) on success, Err(message) on failure.
    pub fn try_load_glyph_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.glyph_registry.load_library(path)?;
        self.rebuild_palette();
        Ok(())
    }

    /// Rebuild the component palette: all static kinds (excluding the generic Custom
    /// placeholder) followed by one entry per custom component in the registry.
    /// If no custom components exist, a single generic Custom placeholder is appended.
    pub fn rebuild_palette(&mut self) {
        let prev_len = self.palette.len();

        self.palette = ComponentKind::all_palette()
            .iter()
            .filter(|&&k| k != ComponentKind::Custom)
            .copied()
            .collect();
        self.palette_custom_indices = vec![None; self.palette.len()];

        let customs = self.glyph_registry.custom_components();
        if customs.is_empty() {
            self.palette.push(ComponentKind::Custom);
            self.palette_custom_indices.push(None);
        } else {
            for i in 0..customs.len() {
                self.palette.push(ComponentKind::Custom);
                self.palette_custom_indices.push(Some(i));
            }
        }

        // Clamp cursor in case the palette shrank
        if self.palette_idx >= self.palette.len() {
            self.palette_idx = self.palette.len().saturating_sub(1);
        }
        let _ = prev_len;
    }

    /// Returns the custom-component registry index for the currently selected palette
    /// slot, or None if the selected slot is not a custom component.
    pub fn selected_custom_idx(&self) -> Option<usize> {
        self.palette_custom_indices.get(self.palette_idx).copied().flatten()
    }

    fn try_load_assemblies() -> AssemblyLibrary {
        AssemblyLibrary::load(std::path::Path::new("assemblies.json"))
            .unwrap_or_default()
    }

    fn try_load_splash() -> Option<Grid> {
        let text = std::fs::read_to_string("splash.json").ok()?;
        let mut grid: Grid = serde_json::from_str(&text).ok()?;
        grid.rebuild_satellites();
        Some(grid)
    }

    pub fn try_load_help() -> Vec<String> {
        std::fs::read_to_string("help.txt")
            .map(|s| s.lines().map(str::to_owned).collect())
            .unwrap_or_else(|_| vec![
                "# Help file not found".into(),
                "".into(),
                "Place  help.txt  in the working directory to populate this screen.".into(),
            ])
    }

    pub fn open_help(&mut self) {
        self.pre_help_mode = self.mode;
        self.help_lines = Self::try_load_help();
        self.help_scroll = 0;
        self.mode = AppMode::Help;
    }

    pub fn close_help(&mut self) {
        self.mode = self.pre_help_mode;
    }

    pub fn help_scroll_up(&mut self, n: usize) {
        self.help_scroll = self.help_scroll.saturating_sub(n);
    }

    pub fn help_scroll_down(&mut self, n: usize) {
        self.help_scroll = self.help_scroll.saturating_add(n);
    }

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

        self.pre_dialog_mode = self.mode;
        self.file_dialog = Some(FileDialogState::new(mode, purpose, start_dir, &initial_name));
        self.mode = AppMode::FileDialog;
    }

    pub fn cancel_file_dialog(&mut self) {
        self.file_dialog = None;
        self.mode = self.pre_dialog_mode;
    }

    pub fn file_dialog_nav(&mut self, delta: i32) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if delta < 0 { fd.nav_up(); } else { fd.nav_down(); }
    }

    pub fn file_dialog_page_up(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        fd.selected = fd.selected.saturating_sub(10);
    }

    pub fn file_dialog_page_down(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if !fd.entries.is_empty() {
            fd.selected = (fd.selected + 10).min(fd.entries.len() - 1);
        }
    }

    pub fn file_dialog_home(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        fd.selected = 0;
    }

    pub fn file_dialog_end(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input { return; }
        if !fd.entries.is_empty() {
            fd.selected = fd.entries.len() - 1;
        }
    }

    pub fn file_dialog_toggle_focus(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.mode == FileDialogMode::Save {
            fd.focus_input = !fd.focus_input;
        }
    }

    pub fn file_dialog_backspace(&mut self) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.focus_input {
            fd.filename_input.pop();
        } else {
            fd.go_parent();
        }
    }

    pub fn file_dialog_type_char(&mut self, ch: char) {
        let Some(fd) = self.file_dialog.as_mut() else { return };
        if fd.mode == FileDialogMode::Save {
            if !fd.focus_input { fd.focus_input = true; }
            fd.filename_input.push(ch);
        }
    }

    pub fn file_dialog_confirm(&mut self) {
        use crate::file_dialog::EnterResult;
        let Some(mut fd) = self.file_dialog.take() else { return };
        fd.error_msg = None;

        match fd.mode {
            FileDialogMode::Open => {
                match fd.enter_selected() {
                    EnterResult::EnteredDir => {
                        self.file_dialog = Some(fd);
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
                                self.file_dialog = Some(fd);
                            }
                        }
                    }
                    EnterResult::None => {
                        self.file_dialog = Some(fd);
                    }
                }
            }
            FileDialogMode::Save => {
                // If focus is on the list, entering a dir navigates; entering a file populates.
                if !fd.focus_input {
                    match fd.enter_selected() {
                        EnterResult::EnteredDir => {
                            self.file_dialog = Some(fd);
                            return;
                        }
                        EnterResult::SelectedFile(_) => {
                            fd.populate_filename_from_selection();
                            fd.focus_input = true;
                            self.file_dialog = Some(fd);
                            return;
                        }
                        EnterResult::None => {}
                    }
                }

                // Confirm save
                let Some(path) = fd.save_path() else {
                    fd.error_msg = Some("Enter a filename first.".into());
                    self.file_dialog = Some(fd);
                    return;
                };
                let purpose = fd.purpose.clone();
                let result = match &purpose {
                    FileDialogPurpose::ExportText => self.export_text_to(&path),
                    _ => self.save_layout_to(&path),
                };
                match result {
                    Ok(()) => {
                        self.mode = self.pre_dialog_mode;
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
                        self.file_dialog = Some(fd);
                    }
                }
            }
        }
    }

    // ── Layout I/O (path-based, extracted from commit_text_input) ─────────────

    pub fn save_layout_to(&mut self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.grid)
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
        self.grid = grid;
        self.layout_path = Some(path.to_path_buf());
        self.sim_result = None;
        self.status_msg = format!("Loaded '{}'.", path.display());
        Ok(())
    }

    // ── New diagram ───────────────────────────────────────────────────────────

    pub fn new_diagram(&mut self) {
        if self.grid_has_content() {
            self.confirm_new_choice = 0;
            self.mode = AppMode::ConfirmNew;
        } else {
            self.do_new_diagram();
        }
    }

    pub fn do_new_diagram(&mut self) {
        self.push_undo();
        let (w, h) = (self.grid.width, self.grid.height);
        self.grid = Grid::new(w, h);
        self.sim_result = None;
        self.layout_path = None;
        self.mode = AppMode::Build;
        self.status_msg = "New diagram.".into();
    }

    fn grid_has_content(&self) -> bool {
        self.grid.cells.iter().any(|row| row.iter().any(|c| c.is_some()))
    }

    pub fn selected_component_kind(&self) -> ComponentKind {
        self.palette[self.palette_idx]
    }

    // ── Simulation controls ──────────────────────────────────────────────────

    pub fn play(&mut self) {
        self.sim_result = Some(simulate(&self.grid, self.fluid_type, &self.glyph_registry));
        self.mode = AppMode::Simulating;
        self.status_msg = "Simulation running.".into();
    }

    pub fn stop(&mut self) {
        self.sim_result = None;
        self.mode = AppMode::Build;
        self.status_msg = "Simulation stopped.".into();
    }

    pub fn pause_toggle(&mut self) {
        match self.mode {
            AppMode::Simulating => {
                self.mode = AppMode::Paused;
                self.status_msg = "Paused.".into();
            }
            AppMode::Paused => {
                self.mode = AppMode::Simulating;
                self.status_msg = "Resumed.".into();
            }
            AppMode::Splash | AppMode::Build | AppMode::GlyphEditor | AppMode::BomView
            | AppMode::Selecting | AppMode::AssemblyBrowser | AppMode::Stamping
            | AppMode::ComponentDetail | AppMode::Help | AppMode::Settings
            | AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit
            | AppMode::ExportDialog | AppMode::AnnotationDialog => {}
        }
    }

    pub fn on_tick(&mut self) {
        // Advance the animation clock for splash and simulation.
        if matches!(self.mode, AppMode::Splash | AppMode::Simulating) {
            self.tick = self.tick.wrapping_add(1);
        }
        if self.mode == AppMode::Simulating && self.tick % 4 == 0 {
            self.sim_result = Some(simulate(&self.grid, self.fluid_type, &self.glyph_registry));
        }
    }

    // ── Cursor / viewport ────────────────────────────────────────────────────

    /// Jump to the topmost–leftmost cell that has content, or (0,0) if empty.
    pub fn jump_to_content_start(&mut self, viewport_h: usize, viewport_w: usize) {
        let target = (0..self.grid.height)
            .find_map(|r| {
                (0..self.grid.width)
                    .find(|&c| self.grid.get(r, c).is_some())
                    .map(|c| (r, c))
            })
            .unwrap_or((0, 0));
        self.cursor = target;
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    /// Jump to the bottommost–rightmost cell that has content, or grid origin if empty.
    pub fn jump_to_content_end(&mut self, viewport_h: usize, viewport_w: usize) {
        let target = (0..self.grid.height)
            .rev()
            .find_map(|r| {
                (0..self.grid.width)
                    .rev()
                    .find(|&c| self.grid.get(r, c).is_some())
                    .map(|c| (r, c))
            })
            .unwrap_or((0, 0));
        self.cursor = target;
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    pub fn move_cursor(&mut self, dr: isize, dc: isize, viewport_h: usize, viewport_w: usize) {
        let (r, c) = self.cursor;
        let new_r = (r as isize + dr).max(0) as usize;
        let new_c = (c as isize + dc).max(0) as usize;
        const GROW: usize = 20;
        if new_r >= self.grid.height {
            self.grid.ensure_size(self.grid.width, new_r + GROW);
        }
        if new_c >= self.grid.width {
            self.grid.ensure_size(new_c + GROW, self.grid.height);
        }
        self.cursor = (new_r, new_c);
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    fn scroll_viewport_to_cursor(&mut self, viewport_h: usize, viewport_w: usize) {
        let (r, c) = self.cursor;
        let (vr, vc) = self.viewport;
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
        self.viewport = (new_vr, new_vc);
    }

    // ── Build actions ────────────────────────────────────────────────────────

    /// True if a composite footprint (fw×fh, port at port_row) can be placed at anchor.
    /// Cells already owned by the existing anchor component are counted as free.
    fn composite_fits_at_fp(&self, fw: usize, fh: usize, pr: usize, anchor_r: usize, anchor_c: usize) -> bool {
        if anchor_r < pr { return false; }
        let top_r = anchor_r - pr;
        if top_r + fh > self.grid.height { return false; }
        if anchor_c + fw > self.grid.width { return false; }
        for dr in 0..fh {
            let r = top_r + dr;
            for dc in 0..fw {
                let c = anchor_c + dc;
                if dr == pr && dc == 0 { continue; } // anchor cell — always replaceable
                if let Some(sat_owner) = self.grid.satellite_anchor(r, c) {
                    if sat_owner != (anchor_r, anchor_c) { return false; }
                } else if self.grid.get(r, c).is_some() {
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
        let (r, c) = self.grid.effective_pos(self.cursor.0, self.cursor.1);
        let kind = smart_orient(self.selected_component_kind(), r, c, &self.grid);
        let old_kind = self.grid.get(r, c).map(|co| co.kind);

        // Resolve custom def (if any) up front.
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
            self.grid.clear_at(r, c);
            let mut comp = Component::new(kind, self.selected_diameter, self.selected_material);
            if let Some((id, conns, fp, label)) = custom_info {
                comp.custom_id = Some(id);
                comp.custom_connections = Some(conns);
                comp.custom_footprint = fp;
                comp.custom_label = Some(label);
            }
            if kind.supports_color_override() {
                comp.color_override = Some(self.selected_build_color());
            }
            self.grid.place_composite(r, c, comp);
        } else {
            self.push_undo();
            self.grid.clear_at(r, c);
            let mut comp = Component::new(kind, self.selected_diameter, self.selected_material);
            if matches!(comp.kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                comp.pipe_length = self.default_lengths.get(&comp.kind).copied().unwrap_or(1.0);
            } else if let Some(&defaults) = self.default_arm_lengths.get(&comp.kind) {
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
            self.grid.set(r, c, Some(comp));
        }

        self.status_msg = match old_kind {
            Some(ok) if ok != kind => format!("Replaced {} → {}", ok.label(), kind.label()),
            Some(_)                => format!("Replaced with {}", kind.label()),
            None                   => format!("Placed {}", kind.label()),
        };
        self.refresh_sim();
    }

    pub fn delete_component(&mut self) {
        let (r, c) = self.grid.effective_pos(self.cursor.0, self.cursor.1);
        if self.grid.get(r, c).is_none() { return; }
        self.push_undo();
        self.grid.clear_at(r, c);
        self.refresh_sim();
    }

    pub fn toggle_valve_at_cursor(&mut self) {
        let (r, c) = self.cursor;
        if self.grid.get(r, c).map(|co| co.kind.is_valve()).unwrap_or(false) {
            self.push_undo();
        }
        if let Some(comp) = self.grid.get_mut(r, c) {
            comp.toggle_valve();
            let open = comp.valve_state == Some(crate::components::ValveState::Open);
            self.status_msg = if open { "Valve opened." } else { "Valve closed." }.into();
            self.refresh_sim();
        }
    }

    // ── Property editing on placed components ────────────────────────────────

    pub fn cycle_material_at_cursor(&mut self) {
        let (r, c) = self.cursor;
        let (ar, ac) = self.grid.effective_pos(r, c);
        if self.grid.get(ar, ac).is_some() { self.push_undo(); }
        if let Some(comp) = self.grid.get_mut(ar, ac) {
            comp.material = comp.material.cycle();
            self.selected_material = comp.material;
            self.status_msg = format!("Material: {}", comp.material.label());
            self.refresh_sim();
        } else {
            self.selected_material = self.selected_material.cycle();
            self.status_msg = format!("Default material: {}", self.selected_material.label());
        }
    }

    pub fn adjust_length_at_cursor(&mut self, delta_in: f32) {
        let (r, c) = self.cursor;
        if self.grid.get(r, c).is_some() { self.push_undo(); }
        if let Some(comp) = self.grid.get_mut(r, c) {
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
        let (r, c) = self.cursor;
        if let Some(comp) = self.grid.get(r, c) {
            if matches!(comp.kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let inches = (comp.pipe_length * 12.0).round() as i32;
                self.input_buffer = inches.to_string();
                self.input_mode = InputMode::EditingLength;
            } else if comp.kind.has_arm_stubs() {
                self.enter_component_detail();
            }
            return;
        }
        // No component at cursor — edit the default length for the selected pipe kind.
        let kind = self.selected_component_kind();
        if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
            let inches = (self.default_lengths.get(&kind).copied().unwrap_or(1.0) * 12.0).round() as i32;
            self.input_buffer = inches.to_string();
            self.input_mode = InputMode::EditingLength;
            self.status_msg = format!("Enter default {} length (inches):", kind.label());
        } else {
            self.status_msg = "Select PipeH or PipeV in palette to set default length.".into();
        }
    }

    pub fn adjust_palette_kind_length(&mut self, delta_in: f32) {
        let kind = self.selected_component_kind();
        if matches!(kind, ComponentKind::PipeH | ComponentKind::PipeV) {
            let current_in = self.default_lengths.get(&kind).copied().unwrap_or(1.0) * 12.0;
            let new_in = (current_in + delta_in).max(1.0);
            self.default_lengths.insert(kind, new_in / 12.0);
            self.status_msg = format!(
                "Default {} length: {} in ({:.2} ft)",
                kind.label(),
                new_in.round() as i32,
                new_in / 12.0,
            );
        }
    }

    pub fn commit_length_input(&mut self) {
        let buf = self.input_buffer.trim().to_string();
        let (r, c) = self.cursor;

        if self.mode == AppMode::ComponentDetail {
            match buf.parse::<f32>() {
                Ok(inches) if inches >= 0.0 => {
                    let raw_port = self.detail_active_ports()
                        .get(self.detail_port_cursor)
                        .map(|&(p, _)| p);
                    if let Some(port) = raw_port {
                        const DIRS: [&str; 4] = ["North", "South", "East", "West"];
                        if self.detail_for_palette {
                            let entry = self.default_arm_lengths
                                .entry(self.detail_kind)
                                .or_insert([0.0; 4]);
                            entry[port] = inches / 12.0;
                        } else {
                            if let Some(comp) = self.grid.get_mut(r, c) {
                                comp.arm_lengths[port] = inches / 12.0;
                                self.refresh_sim();
                            }
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
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            return;
        }

        match buf.parse::<f32>() {
            Ok(inches) if inches >= 1.0 => {
                let on_pipe = self.grid.get(r, c)
                    .map(|co| matches!(co.kind, ComponentKind::PipeH | ComponentKind::PipeV))
                    .unwrap_or(false);

                if on_pipe {
                    if let Some(comp) = self.grid.get_mut(r, c) {
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
                        self.default_lengths.insert(kind, inches / 12.0);
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
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn cancel_input(&mut self) {
        if self.mode == AppMode::AnnotationDialog {
            self.mode = AppMode::Build;
            self.edit_annotation_pos = None;
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.note_cursor_pos = 0;
        self.note_scroll_row = 0;
        self.note_scroll_col = 0;
        self.status_msg = "Cancelled.".into();
    }

    pub fn push_input_char(&mut self, ch: char) {
        match self.input_mode {
            InputMode::EditingLength => {
                if ch.is_ascii_digit() || (ch == '.' && !self.input_buffer.contains('.')) {
                    if self.input_buffer.len() < 8 {
                        self.input_buffer.push(ch);
                    }
                }
            }
            InputMode::EditingText(target) => {
                if ch.is_ascii_graphic() || ch == ' ' {
                    let limit = match target {
                        TextEditTarget::NoteText => 400,
                        TextEditTarget::LinkPath => 256,
                        _ => 120,
                    };
                    if self.input_buffer.len() < limit {
                        if matches!(target, TextEditTarget::NoteText | TextEditTarget::LabelText | TextEditTarget::LinkPath) {
                            let pos = self.note_cursor_pos.min(self.input_buffer.len());
                            self.input_buffer.insert(pos, ch);
                            self.note_cursor_pos = pos + ch.len_utf8();
                            self.note_update_scroll();
                        } else {
                            self.input_buffer.push(ch);
                        }
                    }
                }
            }
            InputMode::Normal => {}
        }
    }

    pub fn pop_input_char(&mut self) {
        let cursor_edit = matches!(
            self.input_mode,
            InputMode::EditingText(TextEditTarget::NoteText)
                | InputMode::EditingText(TextEditTarget::LabelText)
                | InputMode::EditingText(TextEditTarget::LinkPath)
        );
        if cursor_edit {
            if self.note_cursor_pos > 0 {
                let pos = self.note_cursor_pos;
                let prev_len = self.input_buffer[..pos]
                    .chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                if prev_len > 0 {
                    self.input_buffer.remove(pos - prev_len);
                    self.note_cursor_pos = pos - prev_len;
                    self.note_update_scroll();
                }
            }
        } else {
            self.input_buffer.pop();
        }
    }

    /// Insert a newline at the cursor (Shift+Enter). Only valid in NoteText mode.
    pub fn push_note_newline(&mut self) {
        if matches!(self.input_mode, InputMode::EditingText(TextEditTarget::NoteText)) {
            if self.input_buffer.len() < 400 {
                let pos = self.note_cursor_pos.min(self.input_buffer.len());
                self.input_buffer.insert(pos, '\n');
                self.note_cursor_pos = pos + 1;
                self.note_update_scroll();
            }
        }
    }

    /// Returns `(line, col)` — both 0-indexed byte offsets — for `note_cursor_pos`.
    fn note_cursor_lc(&self) -> (usize, usize) {
        let pos = self.note_cursor_pos.min(self.input_buffer.len());
        let before = &self.input_buffer[..pos];
        let line = before.chars().filter(|&c| c == '\n').count();
        let col = match before.rfind('\n') {
            Some(nl) => pos - nl - 1,
            None => pos,
        };
        (line, col)
    }

    /// Adjust `note_scroll_row`/`note_scroll_col` so the cursor stays in view.
    fn note_update_scroll(&mut self) {
        const VIS_ROWS: usize = 3;
        const VIS_W: usize = 50; // conservative estimate of visible content width

        let (line, col) = self.note_cursor_lc();

        if line < self.note_scroll_row {
            self.note_scroll_row = line;
        } else if line >= self.note_scroll_row + VIS_ROWS {
            self.note_scroll_row = line + 1 - VIS_ROWS;
        }

        if col < self.note_scroll_col {
            self.note_scroll_col = col;
        } else if col >= self.note_scroll_col + VIS_W {
            self.note_scroll_col = col + 1 - VIS_W;
        }
    }

    /// Returns true when the active text-edit target is a note (for key routing).
    pub fn is_note_text_mode(&self) -> bool {
        matches!(self.input_mode, InputMode::EditingText(TextEditTarget::NoteText))
    }

    pub fn note_move_left(&mut self) {
        if self.note_cursor_pos > 0 {
            let prev_len = self.input_buffer[..self.note_cursor_pos]
                .chars().last().map(|c| c.len_utf8()).unwrap_or(0);
            self.note_cursor_pos -= prev_len;
            self.note_update_scroll();
        }
    }

    pub fn note_move_right(&mut self) {
        let pos = self.note_cursor_pos;
        if pos < self.input_buffer.len() {
            let next_len = self.input_buffer[pos..]
                .chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            self.note_cursor_pos += next_len;
            self.note_update_scroll();
        }
    }

    pub fn note_move_up(&mut self) {
        let (line, col) = self.note_cursor_lc();
        if line == 0 {
            self.note_cursor_pos = 0;
        } else {
            let lines: Vec<&str> = self.input_buffer.split('\n').collect();
            let new_col = col.min(lines[line - 1].len());
            let start: usize = lines[..line - 1].iter().map(|l| l.len() + 1).sum();
            self.note_cursor_pos = start + new_col;
        }
        self.note_update_scroll();
    }

    pub fn note_move_down(&mut self) {
        let (line, col) = self.note_cursor_lc();
        let lines: Vec<&str> = self.input_buffer.split('\n').collect();
        if line + 1 < lines.len() {
            let new_col = col.min(lines[line + 1].len());
            let start: usize = lines[..=line].iter().map(|l| l.len() + 1).sum();
            self.note_cursor_pos = start + new_col;
        } else {
            self.note_cursor_pos = self.input_buffer.len();
        }
        self.note_update_scroll();
    }

    pub fn is_label_text_mode(&self) -> bool {
        matches!(self.input_mode, InputMode::EditingText(TextEditTarget::LabelText))
    }

    pub fn label_move_left(&mut self) { self.note_move_left(); }
    pub fn label_move_right(&mut self) { self.note_move_right(); }

    pub fn is_link_path_mode(&self) -> bool {
        matches!(self.input_mode, InputMode::EditingText(TextEditTarget::LinkPath))
    }

    pub fn cycle_drain_type_at_cursor(&mut self) {
        let (r, c) = self.cursor;
        let has_drain = self.grid.get(r, c)
            .map(|co| matches!(co.kind, ComponentKind::Sink | ComponentKind::Toilet
                | ComponentKind::Faucet | ComponentKind::BasinSink))
            .unwrap_or(false);
        if has_drain { self.push_undo(); }
        if let Some(comp) = self.grid.get_mut(r, c) {
            if matches!(comp.kind, ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet | ComponentKind::BasinSink) {
                comp.drain_type = comp.drain_type.cycle();
                self.status_msg = format!("Fixture type: {}", comp.drain_type.label());
                self.refresh_sim();
            }
        }
    }

    pub fn adjust_source_pressure_at_cursor(&mut self, delta: f32) {
        let (r, c) = self.cursor;
        if self.grid.get(r, c).map(|co| co.kind == ComponentKind::Source).unwrap_or(false) {
            self.push_undo();
        }
        if let Some(comp) = self.grid.get_mut(r, c) {
            if comp.kind == ComponentKind::Source {
                comp.source_pressure_psi = (comp.source_pressure_psi + delta).clamp(10.0, 200.0);
                self.status_msg = format!("Inlet pressure: {:.0} PSI", comp.source_pressure_psi);
                self.refresh_sim();
            }
        }
    }

    pub fn begin_source_pressure_dialog(&mut self) {
        let (r, c) = self.cursor;
        let Some(comp) = self.grid.get(r, c) else { return };
        if comp.kind != ComponentKind::Source { return }
        self.input_buffer = format!("{:.1}", comp.source_pressure_psi);
        self.note_cursor_pos = self.input_buffer.len();
        self.note_scroll_col = 0;
        self.input_mode = InputMode::EditingText(TextEditTarget::SourcePressure);
        self.mode = AppMode::AnnotationDialog;
    }

    // ── Material selection ───────────────────────────────────────────────────

    pub fn set_material_by_index(&mut self, idx: usize) {
        use crate::components::PipeMaterial::*;
        let mats = [Copper, PEX, PE, GalvanizedIron, BlackPlastic, CastIron];
        if let Some(&mat) = mats.get(idx) {
            self.selected_material = mat;
            self.status_msg = format!("Material: {}", mat.label());
            let (r, c) = self.cursor;
            let (ar, ac) = self.grid.effective_pos(r, c);
            if let Some(comp) = self.grid.get_mut(ar, ac) {
                comp.material = mat;
                self.refresh_sim();
            }
        }
    }

    pub fn nav_material(&mut self, delta: isize) {
        use crate::components::PipeMaterial::*;
        let mats = [Copper, PEX, PE, GalvanizedIron, BlackPlastic, CastIron];
        let cur = mats.iter().position(|&m| m == self.selected_material).unwrap_or(0);
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

    // ── Palette navigation ───────────────────────────────────────────────────

    pub fn cycle_diameter(&mut self) {
        self.selected_diameter = self.selected_diameter.cycle();
        self.status_msg = format!("Diameter: {}", self.selected_diameter.label());
    }

    pub fn palette_up(&mut self) {
        if self.palette_idx > 0 {
            self.palette_idx -= 1;
        }
    }

    pub fn palette_down(&mut self) {
        if self.palette_idx + 1 < self.palette.len() {
            self.palette_idx += 1;
        }
    }

    pub fn palette_home(&mut self) {
        self.palette_idx = 0;
    }

    pub fn palette_end(&mut self) {
        if !self.palette.is_empty() {
            self.palette_idx = self.palette.len() - 1;
        }
    }

    pub fn palette_page_up(&mut self) {
        self.palette_idx = self.palette_idx.saturating_sub(10);
    }

    pub fn palette_page_down(&mut self) {
        if !self.palette.is_empty() {
            self.palette_idx = (self.palette_idx + 10).min(self.palette.len() - 1);
        }
    }

    // ── Glyph editor ─────────────────────────────────────────────────────────

    pub fn enter_glyph_editor(&mut self) {
        self.mode = AppMode::GlyphEditor;
        self.editor.status =
            "  [Tab] switch panel  [Enter] apply  [M] mat scope  [D] diam scope  \
             [N] new  [R] rename  [C] copy  [W] composite  [S] save  [L] load  [G/Q] exit"
                .into();
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
                let prev_idx = self.editor.kind_idx;
                let total = ComponentKind::all_palette().len()
                    + self.glyph_registry.custom_components().len();
                self.editor.nav_kind(dr, total);
                // If navigating away from a composite, drop back to CharGrid focus.
                if !self.editor_selected_is_composite()
                    && self.editor.focus == GlyphEditorFocus::CompositeGrid
                {
                    self.editor.focus = GlyphEditorFocus::CharGrid;
                }
                // Reset composite cursor and viewport only when the selected component changes,
                // so that returning to CompositeGrid after checking the list keeps the cursor in place.
                if self.editor.kind_idx != prev_idx {
                    self.editor.composite_cursor = (1, 1);
                    self.editor.composite_viewport = (0, 0);
                }
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
            self.editor.kind_idx = 0;
        }
    }

    pub fn editor_nav_end(&mut self) {
        if self.editor.focus == GlyphEditorFocus::ComponentList {
            let total = ComponentKind::all_palette().len()
                + self.glyph_registry.custom_components().len();
            if total > 0 {
                self.editor.kind_idx = total - 1;
            }
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

    fn editor_selected_is_composite(&self) -> bool {
        let static_len = ComponentKind::all_palette().len();
        if self.editor.kind_idx < static_len { return false; }
        let ci = self.editor.kind_idx - static_len;
        let customs = self.glyph_registry.custom_components();
        ci < customs.len() && customs[ci].composite_size.is_some()
    }

    // ── Annotations ──────────────────────────────────────────────────────────

    pub fn toggle_annotations(&mut self) {
        self.show_annotations = !self.show_annotations;
        self.status_msg = if self.show_annotations {
            "Annotations ON".into()
        } else {
            "Annotations OFF".into()
        };
    }

    // ── BOM view ─────────────────────────────────────────────────────────────

    pub fn enter_bom(&mut self) {
        self.pre_bom_mode = self.mode;
        self.mode = AppMode::BomView;
    }

    pub fn exit_bom(&mut self) {
        self.mode = self.pre_bom_mode;
    }

    // ── Assembly: selection ──────────────────────────────────────────────────

    pub fn start_selecting(&mut self) {
        self.select_start = Some(self.cursor);
        self.mode = AppMode::Selecting;
        self.status_msg = "Selection: arrows resize rect  [C] copy  [X] move  [Enter]/[R] save assembly  [Esc] cancel".into();
    }

    pub fn confirm_selection(&mut self) {
        if self.select_start.is_some() {
            self.input_buffer.clear();
            self.input_mode = InputMode::EditingText(TextEditTarget::AssemblyName);
            self.status_msg = "Assembly name:".into();
        }
    }

    pub fn cancel_selection(&mut self) {
        self.select_start = None;
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
        let Some(start) = self.select_start else { return };
        let end = self.cursor;
        let r0 = start.0.min(end.0);
        let r1 = start.0.max(end.0);
        let c0 = start.1.min(end.1);
        let c1 = start.1.max(end.1);
        let asm = Assembly::from_selection(
            &self.grid, r0, c0, r1, c1,
            "clipboard".into(), String::new(),
        );
        let count = asm.component_count();
        let w = c1 - c0 + 1;
        let h = r1 - r0 + 1;
        self.pending_stamp = Some(asm);
        self.stamp_cut_rect = if is_cut { Some((r0, c0, r1, c1)) } else { None };
        self.select_start = None;
        self.cursor = (r0, c0); // ghost starts aligned over original
        self.mode = AppMode::Stamping;
        let action = if is_cut { "move" } else { "copy" };
        self.status_msg = format!(
            "{}×{} ({} components) to {} — arrows to position, [Enter] paste, [Esc] cancel",
            w, h, count, action
        );
    }

    pub fn save_assembly_named(&mut self, name: String) {
        let Some(start) = self.select_start else { return };
        let end = self.cursor;
        let r0 = start.0.min(end.0);
        let r1 = start.0.max(end.0);
        let c0 = start.1.min(end.1);
        let c1 = start.1.max(end.1);

        let assembly = Assembly::from_selection(&self.grid, r0, c0, r1, c1, name, String::new());
        let comp_count = assembly.component_count();
        self.assembly_lib.assemblies.push(assembly);
        self.select_start = None;
        self.mode = AppMode::Build;

        let save_msg = if let Some(path) = &self.assembly_path.clone() {
            match self.assembly_lib.save(path) {
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
        self.pre_assembly_mode = self.mode;
        self.assembly_idx = self.assembly_idx.min(self.assembly_lib.assemblies.len().saturating_sub(1));
        self.mode = AppMode::AssemblyBrowser;
    }

    pub fn exit_assembly_browser(&mut self) {
        self.mode = self.pre_assembly_mode;
    }

    pub fn assembly_browser_up(&mut self) {
        if self.assembly_idx > 0 {
            self.assembly_idx -= 1;
        }
    }

    pub fn assembly_browser_down(&mut self) {
        if self.assembly_idx + 1 < self.assembly_lib.assemblies.len() {
            self.assembly_idx += 1;
        }
    }

    pub fn delete_assembly(&mut self) {
        let libs = &mut self.assembly_lib.assemblies;
        if self.assembly_idx < libs.len() {
            let name = libs[self.assembly_idx].name.clone();
            libs.remove(self.assembly_idx);
            if self.assembly_idx >= libs.len() && self.assembly_idx > 0 {
                self.assembly_idx -= 1;
            }
            if let Some(path) = &self.assembly_path.clone() {
                let _ = self.assembly_lib.save(path);
            }
            self.status_msg = format!("Deleted assembly '{name}'.");
        }
    }

    // ── Assembly: stamp ──────────────────────────────────────────────────────

    pub fn begin_stamp(&mut self) {
        let idx = self.assembly_idx;
        if idx < self.assembly_lib.assemblies.len() {
            self.pending_stamp = Some(self.assembly_lib.assemblies[idx].clone());
            self.mode = AppMode::Stamping;
            self.status_msg = "Move cursor to top-left corner, then [Enter] to stamp. [Esc] to cancel.".into();
        }
    }

    pub fn confirm_stamp(&mut self) {
        if let Some(asm) = self.pending_stamp.take() {
            let (r, c) = self.cursor;
            self.push_undo();
            // For moves: clear the source region before stamping so overlapping areas work correctly.
            if let Some((r0, c0, r1, c1)) = self.stamp_cut_rect.take() {
                for gr in r0..=r1 {
                    for gc in c0..=c1 {
                        self.grid.set(gr, gc, None);
                    }
                }
            }
            asm.stamp_onto(&mut self.grid, r, c);
            self.grid.rebuild_satellites();
            self.mode = AppMode::Build;
            self.status_msg = format!("Pasted {}×{} at ({},{}).", asm.width, asm.height, r, c);
            self.refresh_sim();
        }
    }

    pub fn cancel_stamp(&mut self) {
        self.pending_stamp = None;
        self.stamp_cut_rect = None;
        self.mode = AppMode::Build;
        self.status_msg = "Cancelled.".into();
    }

    // ── Component detail ─────────────────────────────────────────────────────

    /// Open detail overlay for the PLACED component at the cursor.
    pub fn enter_component_detail(&mut self) {
        let (r, c) = self.cursor;
        if let Some(comp) = self.grid.get(r, c) {
            self.pre_detail_mode = self.mode;
            self.detail_kind = comp.kind;
            self.detail_for_palette = false;
            self.detail_port_cursor = 0;
            self.mode = AppMode::ComponentDetail;
        }
    }

    /// Open detail overlay to set DEFAULT arm lengths for the currently selected palette kind.
    pub fn enter_palette_component_detail(&mut self) {
        let kind = self.selected_component_kind();
        if !kind.has_arm_stubs() {
            return;
        }
        self.pre_detail_mode = self.mode;
        self.detail_kind = kind;
        self.detail_for_palette = true;
        self.detail_port_cursor = 0;
        self.mode = AppMode::ComponentDetail;
    }

    pub fn exit_component_detail(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.mode = self.pre_detail_mode;
    }

    /// Returns the active ports for whatever is being edited (placed or palette default).
    pub fn detail_active_ports(&self) -> Vec<(usize, &'static str)> {
        if !self.detail_kind.has_arm_stubs() {
            return Vec::new();
        }
        const NAMES: [&str; 4] = ["North", "South", "East", "West"];
        let (n, s, e, w) = if self.detail_for_palette {
            self.detail_kind.connections()
        } else {
            let (r, c) = self.cursor;
            self.grid.get(r, c)
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
        if self.detail_for_palette {
            self.default_arm_lengths.get(&self.detail_kind).copied().unwrap_or([0.0; 4])
        } else {
            let (r, c) = self.cursor;
            self.grid.get(r, c).map(|co| co.arm_lengths).unwrap_or([0.0; 4])
        }
    }

    pub fn component_detail_nav(&mut self, delta: isize) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail_port_cursor = (self.detail_port_cursor as isize + delta)
                .rem_euclid(count as isize) as usize;
        }
    }

    pub fn component_detail_page_up(&mut self) {
        self.detail_port_cursor = self.detail_port_cursor.saturating_sub(10);
    }

    pub fn component_detail_page_down(&mut self) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail_port_cursor = (self.detail_port_cursor + 10).min(count - 1);
        }
    }

    pub fn component_detail_home(&mut self) {
        self.detail_port_cursor = 0;
    }

    pub fn component_detail_end(&mut self) {
        let count = self.detail_active_ports().len();
        if count > 0 {
            self.detail_port_cursor = count - 1;
        }
    }

    pub fn begin_port_length_edit(&mut self) {
        let ports = self.detail_active_ports();
        if let Some(&(raw_port, dir)) = ports.get(self.detail_port_cursor) {
            let arm_lengths = self.detail_arm_lengths();
            let cur_in = (arm_lengths[raw_port] * 12.0).round() as i32;
            self.input_buffer = if cur_in > 0 { cur_in.to_string() } else { String::new() };
            self.input_mode = InputMode::EditingLength;
            self.status_msg = format!("Enter {} stub length (inches):", dir);
        }
    }

    // ── Fluid type ───────────────────────────────────────────────────────────

    pub fn cycle_fluid_type(&mut self) {
        self.fluid_type = self.fluid_type.cycle();
        self.status_msg = format!("Fluid: {}", self.fluid_type.label());
        self.refresh_sim();
    }

    // ── Layout save / load ───────────────────────────────────────────────────

    // ── Glyph editor ─────────────────────────────────────────────────────────

    pub fn editor_begin_save(&mut self) {
        let path = self
            .glyph_registry
            .library_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("glyphs.json")
            .to_string();
        self.input_buffer = path;
        self.input_mode = InputMode::EditingText(TextEditTarget::SaveLibrary);
    }

    pub fn editor_begin_load(&mut self) {
        let path = self
            .glyph_registry
            .library_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("glyphs.json")
            .to_string();
        self.input_buffer = path;
        self.input_mode = InputMode::EditingText(TextEditTarget::LoadLibrary);
    }

    pub fn editor_begin_new_comp(&mut self) {
        self.input_buffer.clear();
        self.input_mode = InputMode::EditingText(TextEditTarget::NewCompName);
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
        self.input_buffer = customs[ci].label.clone();
        self.input_mode = InputMode::EditingText(TextEditTarget::RenameComp);
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
        self.input_buffer = format!("{source_label} Copy");
        self.input_mode = InputMode::EditingText(TextEditTarget::CopyComp);
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
        self.editor_pending_delete = Some(ci);
    }

    /// [Y] after the delete prompt — executes the deletion.
    pub fn editor_confirm_delete_comp(&mut self) {
        let Some(ci) = self.editor_pending_delete.take() else { return };
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
        self.editor_pending_delete = None;
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
        self.input_buffer = match customs[ci].composite_size {
            Some((w, h)) => format!("{w}x{h}"),
            None         => String::new(),
        };
        self.input_mode = InputMode::EditingText(TextEditTarget::CompWidth);
    }

    /// Commit a text-input prompt started from the glyph editor.
    pub fn commit_text_input(&mut self) {
        // resolve here so the borrow checker is happy inside the match arms
        let copy_kind_idx = self.editor.kind_idx;
        let buf = self.input_buffer.trim().to_string();
        match self.input_mode {
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
                    self.editor.kind_idx = ComponentKind::all_palette().len() + ci;
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
                            self.settings_status = format!("OK — loaded {}", path.display());
                        }
                        Err(e) => {
                            self.settings_status = format!("Added (not yet loaded): {e}");
                        }
                    }
                    self.settings_idx = self.config.glyph_files.len().saturating_sub(1);
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
                    self.build_custom_rgb = Some([r, g, b]);
                } else {
                    self.status_msg = "Format: R,G,B  e.g.  255,128,0".into();
                }
            }
            InputMode::EditingText(TextEditTarget::LabelText) => {
                self.mode = AppMode::Build;
                if let Some((r, c)) = self.edit_annotation_pos.take() {
                    // Editing existing label in place
                    self.push_undo();
                    if buf.is_empty() {
                        self.grid.set(r, c, None);
                        self.status_msg = "Label removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Label, self.selected_diameter, self.selected_material);
                        comp.text = Some(buf);
                        self.grid.set(r, c, Some(comp));
                        self.status_msg = "Label updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    // New label — enter placement mode so the user can choose position
                    self.pending_annotation = Some((ComponentKind::Label, buf));
                    self.status_msg = "Move cursor to position, [Enter] to place, [Esc] to cancel.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::NoteText) => {
                self.mode = AppMode::Build;
                if let Some((r, c)) = self.edit_annotation_pos.take() {
                    // Editing existing note in place
                    self.push_undo();
                    if buf.is_empty() {
                        self.grid.set(r, c, None);
                        self.status_msg = "Note removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Note, self.selected_diameter, self.selected_material);
                        comp.text = Some(buf);
                        self.grid.set(r, c, Some(comp));
                        self.status_msg = "Note updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    self.pending_annotation = Some((ComponentKind::Note, buf));
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
                if let Some((r, c)) = self.edit_annotation_pos.take() {
                    self.push_undo();
                    if buf.is_empty() {
                        self.grid.set(r, c, None);
                        self.status_msg = "Link removed.".into();
                    } else {
                        let mut comp = crate::components::Component::new(ComponentKind::Link, self.selected_diameter, self.selected_material);
                        comp.text = Some(buf);
                        self.grid.set(r, c, Some(comp));
                        self.status_msg = "Link updated.".into();
                    }
                    self.refresh_sim();
                } else if !buf.is_empty() {
                    self.pending_annotation = Some((ComponentKind::Link, buf));
                    self.status_msg = "Move cursor to position, [Enter] to place, [Esc] to cancel.".into();
                }
            }
            InputMode::EditingText(TextEditTarget::SourcePressure) => {
                self.input_mode = InputMode::Normal;
                self.mode = AppMode::Build;
                match buf.parse::<f32>() {
                    Ok(psi) => {
                        let psi = psi.clamp(10.0, 200.0);
                        let (r, c) = self.cursor;
                        if self.grid.get(r, c).map(|co| co.kind == ComponentKind::Source).unwrap_or(false) {
                            self.push_undo();
                            if let Some(comp) = self.grid.get_mut(r, c) {
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
            InputMode::EditingText(TextEditTarget::CopyComp) => {
                if buf.is_empty() {
                    self.editor.status = "Name cannot be empty.".into();
                } else {
                    let static_len = ComponentKind::all_palette().len();
                    let new_id = buf.to_lowercase().replace(' ', "_");
                    if copy_kind_idx < static_len {
                        // Copy from a built-in standard component → snapshot as custom
                        let kind = ComponentKind::all_palette()[copy_kind_idx];
                        let glyph = self.glyph_registry.resolve(kind, self.selected_material, self.selected_diameter);
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
                    self.editor.kind_idx = static_len + new_ci;
                    self.editor.status = format!("Copied to '{buf}'. Edit ports/cells as needed.");
                }
            }
            _ => {}
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.note_cursor_pos = 0;
        self.note_scroll_row = 0;
        self.note_scroll_col = 0;
    }

    // ── Build-mode color picker ──────────────────────────────────────────────

    /// Returns the active build color: custom_rgb if set, else palette selection.
    pub fn selected_build_color(&self) -> [u8; 3] {
        if let Some(rgb) = self.build_custom_rgb {
            return rgb;
        }
        let (r, g, b, _) = COLOR_PALETTE[self.build_color_cursor.min(COLOR_PALETTE.len() - 1)];
        [r, g, b]
    }

    /// Navigate the build-mode color palette grid.
    pub fn palette_color_nav(&mut self, dr: isize, dc: isize) {
        self.build_custom_rgb = None;
        let total = COLOR_PALETTE.len();
        let cols  = COLOR_PALETTE_COLS as isize;
        let rows  = ((total + COLOR_PALETTE_COLS - 1) / COLOR_PALETTE_COLS) as isize;
        let row = (self.build_color_cursor as isize / cols + dr).rem_euclid(rows);
        let col = (self.build_color_cursor as isize % cols + dc).rem_euclid(cols);
        self.build_color_cursor = ((row * cols + col) as usize).min(total - 1);
    }

    pub fn palette_begin_custom_rgb(&mut self) {
        let [r, g, b] = self.selected_build_color();
        self.input_buffer = format!("{r},{g},{b}");
        self.input_mode = InputMode::EditingText(TextEditTarget::BuildCustomRgb);
        self.status_msg = "Custom RGB (R,G,B):".into();
    }

    // ── Settings screen ──────────────────────────────────────────────────────

    pub fn open_settings(&mut self) {
        self.settings_status.clear();
        self.mode = AppMode::Settings;
    }

    pub fn close_settings(&mut self) {
        self.mode = AppMode::Build;
    }

    pub fn settings_nav(&mut self, delta: isize) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        self.settings_idx = (self.settings_idx as isize + delta)
            .clamp(0, n as isize - 1) as usize;
    }

    pub fn settings_home(&mut self) {
        self.settings_idx = 0;
    }

    pub fn settings_end(&mut self) {
        let n = self.config.glyph_files.len();
        if n > 0 { self.settings_idx = n - 1; }
    }

    pub fn settings_begin_add(&mut self) {
        self.input_buffer.clear();
        self.input_mode = InputMode::EditingText(TextEditTarget::AddGlyphFile);
        self.status_msg = "Glyph file path:".into();
    }

    pub fn settings_remove(&mut self) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        let i = self.settings_idx.min(n - 1);
        self.config.glyph_files.remove(i);
        self.config.save();
        self.settings_idx = self.settings_idx.min(self.config.glyph_files.len().saturating_sub(1));
        self.settings_status = "Removed.".into();
    }

    pub fn settings_load_now(&mut self) {
        let n = self.config.glyph_files.len();
        if n == 0 { return; }
        let path = self.config.glyph_files[self.settings_idx.min(n - 1)].clone();
        match self.try_load_glyph_file(&path) {
            Ok(()) => self.settings_status = format!("OK — loaded {}", path.display()),
            Err(e) => self.settings_status = format!("Error: {e}"),
        }
    }

    pub fn editor_cycle_mat_scope(&mut self) {
        self.editor.cycle_mat_scope();
    }

    pub fn editor_cycle_diam_scope(&mut self) {
        self.editor.cycle_diam_scope();
    }

    pub fn editor_begin_custom_rgb(&mut self) {
        let [r, g, b] = self.editor.current_color();
        self.input_buffer = format!("{r},{g},{b}");
        self.input_mode = InputMode::EditingText(TextEditTarget::CustomRgb);
        self.status_msg = "Custom RGB (0-255 each):".into();
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub fn component_at_cursor(&self) -> Option<&Component> {
        let (r, c) = self.cursor;
        self.grid.get(r, c)
    }

    pub fn flow_state_at_cursor(&self) -> Option<&crate::simulation::FlowState> {
        let (r, c) = self.cursor;
        self.sim_result.as_ref()?.cell_states.get(&(r, c))
    }

    pub fn flow_data_at_cursor(&self) -> Option<&NodeFlowData> {
        let (r, c) = self.cursor;
        self.sim_result.as_ref()?.flow_data.get(&(r, c))
    }

    // ── Undo / redo ───────────────────────────────────────────────────────────

    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.grid.clone());
        if self.undo_stack.len() > UNDO_MAX {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.grid.clone());
            self.grid = prev;
            self.grid.rebuild_satellites();
            self.refresh_sim();
            self.status_msg = format!("Undo  ({} left)", self.undo_stack.len());
        } else {
            self.status_msg = "Nothing to undo.".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.grid.clone());
            self.grid = next;
            self.grid.rebuild_satellites();
            self.refresh_sim();
            self.status_msg = format!("Redo  ({} redo left)", self.redo_stack.len());
        } else {
            self.status_msg = "Nothing to redo.".into();
        }
    }

    // ── Annotation placement ─────────────────────────────────────────────────

    pub fn begin_label_placement(&mut self) {
        let (r, c) = self.cursor;
        let is_existing = self.grid.get(r, c).map(|co| co.kind == ComponentKind::Label).unwrap_or(false);
        let existing = self.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Label)
            .and_then(|co| co.text.as_deref())
            .unwrap_or("")
            .to_string();
        self.input_buffer = existing;
        self.note_cursor_pos = self.input_buffer.len();
        self.note_scroll_col = 0;
        self.note_update_scroll();
        self.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.input_mode = InputMode::EditingText(TextEditTarget::LabelText);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn begin_note_placement(&mut self) {
        let (r, c) = self.cursor;
        let is_existing = self.grid.get(r, c).map(|co| co.kind == ComponentKind::Note).unwrap_or(false);
        let existing = self.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Note)
            .and_then(|co| co.text.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_default();
        self.input_buffer = existing;
        self.note_cursor_pos = self.input_buffer.len();
        self.note_scroll_row = 0;
        self.note_scroll_col = 0;
        self.note_update_scroll();
        self.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.input_mode = InputMode::EditingText(TextEditTarget::NoteText);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn begin_link_placement(&mut self) {
        let (r, c) = self.cursor;
        let is_existing = self.grid.get(r, c).map(|co| co.kind == ComponentKind::Link).unwrap_or(false);
        let existing = self.grid.get(r, c)
            .filter(|co| co.kind == ComponentKind::Link)
            .and_then(|co| co.text.as_deref())
            .unwrap_or("")
            .to_string();
        self.input_buffer = existing;
        self.note_cursor_pos = self.input_buffer.len();
        self.note_scroll_col = 0;
        self.note_update_scroll();
        self.edit_annotation_pos = if is_existing { Some((r, c)) } else { None };
        self.input_mode = InputMode::EditingText(TextEditTarget::LinkPath);
        self.mode = AppMode::AnnotationDialog;
    }

    pub fn follow_link_at_cursor(&mut self) {
        let (r, c) = self.cursor;
        let Some(comp) = self.grid.get(r, c) else { return };
        if comp.kind != ComponentKind::Link { return }
        let path_str = comp.text.clone().unwrap_or_default();
        if path_str.is_empty() {
            self.status_msg = "Link has no target — press [E] to set path.".into();
            return;
        }
        self.pending_link_path = Some(path_str);
        if self.grid_has_content() {
            self.confirm_new_choice = 0;
            self.mode = AppMode::ConfirmNew;
        } else {
            self.do_follow_link();
        }
    }

    pub fn do_follow_link(&mut self) {
        if let Some(path_str) = self.pending_link_path.take() {
            let path = std::path::PathBuf::from(&path_str);
            match self.load_layout_from(&path) {
                Ok(()) => {}
                Err(e) => self.status_msg = format!("Link failed: {e}"),
            }
            self.mode = AppMode::Build;
        }
    }

    // ── Palette search ────────────────────────────────────────────────────────

    pub fn palette_item_matches(&self, idx: usize, query: &str) -> bool {
        let Some(kind) = self.palette.get(idx) else { return false };
        if *kind == ComponentKind::Custom {
            let customs = self.glyph_registry.custom_components();
            let ci = self.palette_custom_indices.get(idx).copied().flatten();
            if let Some(ci) = ci.filter(|&ci| ci < customs.len()) {
                return customs[ci].label.to_lowercase().contains(query);
            }
            return "custom comp".contains(query);
        }
        kind.label().to_lowercase().contains(query)
    }

    /// Jump palette_idx to the first palette item that matches the current search query.
    pub fn palette_search_jump_first(&mut self) {
        let query = self.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.palette.len();
        for i in 0..len {
            if self.palette_item_matches(i, &query) {
                self.palette_idx = i;
                return;
            }
        }
    }

    /// Move palette_idx to the next matching item (wraps around).
    pub fn palette_search_next(&mut self) {
        let query = self.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.palette.len();
        for offset in 1..=len {
            let i = (self.palette_idx + offset) % len;
            if self.palette_item_matches(i, &query) {
                self.palette_idx = i;
                return;
            }
        }
    }

    /// Move palette_idx to the previous matching item (wraps around).
    pub fn palette_search_prev(&mut self) {
        let query = self.palette_search.to_lowercase();
        if query.is_empty() { return; }
        let len = self.palette.len();
        for offset in 1..=len {
            let i = (self.palette_idx + len - offset) % len;
            if self.palette_item_matches(i, &query) {
                self.palette_idx = i;
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

    pub fn place_pending_annotation(&mut self) {
        if let Some((kind, text)) = self.pending_annotation.take() {
            let (r, c) = self.cursor;
            self.push_undo();
            let mut comp = crate::components::Component::new(kind, self.selected_diameter, self.selected_material);
            comp.text = Some(text);
            self.grid.set(r, c, Some(comp));
            self.refresh_sim();
            self.status_msg = "Annotation placed.".into();
        }
    }

    pub fn cancel_pending_annotation(&mut self) {
        self.pending_annotation = None;
        self.status_msg = "Placement cancelled.".into();
    }

    // ── Export ────────────────────────────────────────────────────────────────

    pub fn open_export_dialog(&mut self) {
        self.mode = AppMode::ExportDialog;
    }

    pub fn export_text_to(&self, path: &std::path::Path) -> Result<(), String> {
        use crate::components::ComponentKind;

        // Find content bounds (include satellite cells)
        let mut min_r = self.grid.height;
        let mut max_r = 0usize;
        let mut min_c = self.grid.width;
        let mut max_c = 0usize;
        for r in 0..self.grid.height {
            for c in 0..self.grid.width {
                if self.grid.get(r, c).is_some() || self.grid.satellite_anchor(r, c).is_some() {
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
        for r in 0..self.grid.height {
            for c in 0..self.grid.width {
                if let Some(comp) = self.grid.get(r, c) {
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
                                // * + ═×(max_w+2) + ╗ → width = max_w+4; ╗ at c+max_w+3
                                // top + pad + lines + pad + bottom → height = num_lines+4; ╝ at r+num_lines+3
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
            self.grid.get(gr, gc).is_none() && self.grid.satellite_anchor(gr, gc).is_none()
        };

        // Precompute note box overlay: ╔═...═╗ / ║text║ / ╚═...═╝
        let mut note_export: std::collections::HashMap<(usize, usize), char> =
            std::collections::HashMap::new();
        for nr in 0..self.grid.height {
            for nc in 0..self.grid.width {
                if let Some(comp) = self.grid.get(nr, nc) {
                    if comp.kind == ComponentKind::Note {
                        if let Some(text) = &comp.text {
                            let segs: Vec<&str> = text.split('\n').collect();
                            let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                            let inner_w = max_w + 2; // 1 left-pad + text + 1 right-pad
                            let right_c = nc + inner_w + 1;
                            // Top border (anchor * from export_cell_char; fill ═×inner_w; ╗)
                            for ci in 1..=inner_w { note_export.insert((nr, nc + ci), '═'); }
                            note_export.insert((nr, right_c), '╗');
                            // Top blank padding row
                            note_export.insert((nr + 1, nc), '║');
                            for ci in 1..=inner_w { note_export.insert((nr + 1, nc + ci), ' '); }
                            note_export.insert((nr + 1, right_c), '║');
                            // Content rows
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
                            // Bottom blank padding row
                            let bpad = nr + segs.len() + 2;
                            note_export.insert((bpad, nc), '║');
                            for ci in 1..=inner_w { note_export.insert((bpad, nc + ci), ' '); }
                            note_export.insert((bpad, right_c), '║');
                            // Bottom border
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
                // Apply note box overlay on empty cells
                if ch == ' ' {
                    if let Some(&nch) = note_export.get(&(r, c)) {
                        row_chars[c - min_c] = nch;
                    }
                }
            }

            // Spread label text: [text]
            for c in min_c..=max_c {
                if let Some(comp) = self.grid.get(r, c) {
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

        // Satellite cell — compute composite box char
        if let Some((ar, ac)) = self.grid.satellite_anchor(r, c) {
            if let Some(comp) = self.grid.get(ar, ac) {
                let pr = comp.effective_port_row();
                let (fw, fh) = comp.effective_footprint();
                let dr = r.wrapping_add(pr).wrapping_sub(ar);
                let dc = c.wrapping_sub(ac);
                return self.export_composite_char(comp, fw, fh, pr, dr, dc);
            }
        }

        let Some(comp) = self.grid.get(r, c) else { return ' '; };

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

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn refresh_sim(&mut self) {
        if self.mode == AppMode::Simulating || self.mode == AppMode::Paused {
            self.sim_result = Some(simulate(&self.grid, self.fluid_type, &self.glyph_registry));
        }
    }
}

// ── Smart orientation ─────────────────────────────────────────────────────────
//
// When placing a tee, elbow, or H/V component on or next to existing pipes,
// automatically pick the orientation that fits the live neighbor connections.
// For example: placing any Tee variant between three connected pipes will snap
// to the correct ├ / ┤ / ┬ / ┴ face automatically.

fn smart_orient(selected: ComponentKind, r: usize, c: usize, grid: &Grid) -> ComponentKind {
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
    // We look at the neighbor's port on the SHARED FACE (not whether it's
    // currently connected — just whether the port exists).
    // connections() = (north, south, east, west)
    let height = grid.height;
    let width  = grid.width;

    let n = r > 0          && grid.get(r - 1, c).map(|co| co.connections().1).unwrap_or(false);
    let s = r + 1 < height && grid.get(r + 1, c).map(|co| co.connections().0).unwrap_or(false);
    let e = c + 1 < width  && grid.get(r, c + 1).map(|co| co.connections().3).unwrap_or(false);
    let w = c > 0          && grid.get(r, c - 1).map(|co| co.connections().2).unwrap_or(false);

    // If the cell is already occupied, exclude its own ports from the neighbour
    // detection — we only care about what neighbours *offer* to the new component.
    // (Already handled: we never query grid.get(r,c) above.)

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
