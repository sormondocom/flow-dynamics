use std::path::{Path, PathBuf};

use crate::assembly::{Assembly, AssemblyLibrary};
use crate::canvas_state::CanvasState;
use crate::component_detail_state::ComponentDetailState;
use crate::components::{Component, ComponentKind};
use crate::config::AppConfig;
use crate::dialog_state::DialogState;
use crate::file_dialog::{FileDialogMode, FileDialogPurpose, FileDialogState};
use crate::fluid::FluidType;
use crate::glyphs::{
    CustomCompDef, EditorDisplayRow, GlyphDef, GlyphEditorFocus, GlyphEditorState, GlyphRegistry,
    ALL_DIAMETERS, ALL_MATERIALS, COLOR_PALETTE, COLOR_PALETTE_COLS,
};
use crate::grid::Grid;
use crate::palette_state::PaletteState;
use crate::selection_state::SelectionState;
use crate::sim_state::SimState;
use crate::simulation::{simulate, validate_dwv, DwvResult, NodeFlowData};
use crate::text_input_state::TextInputState;
use crate::undo_state::UndoState;

mod groups;
mod glyph_editor;
mod canvas_ops;
mod io_ops;
mod assembly;
mod text_input;

pub const GRID_COLS_MIN: usize = 79;
pub const GRID_ROWS_MIN: usize = 40;

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
    /// Full-screen cost estimator / price editor.
    CostEstimator,
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
    /// Exact PSI setpoint typed into the PRV dialog.
    PrvSetpoint,
    /// File path for a Link annotation being placed or edited.
    LinkPath,
    /// Price value being edited in the cost estimator.
    CostPrice,
    /// Name for a new component group.
    NewGroupName,
    /// Name of the group to assign the selected component to (typed after picker).
    GroupAssign,
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
    pub tick: u64,
    pub should_quit: bool,
    pub status_msg: String,
    pub glyph_registry: GlyphRegistry,
    pub editor: GlyphEditorState,
    pub layout_path: Option<PathBuf>,
    pub show_annotations: bool,
    /// Lines loaded from help.txt (hot-reloaded each time help opens).
    pub help_lines: Vec<String>,
    /// Scroll offset within the help screen.
    pub help_scroll: usize,
    /// Persistent application configuration (glyph auto-load list, etc.)
    pub config: AppConfig,
    /// Current search query typed with [/] in the help screen.
    pub help_search: String,
    /// Whether the help search bar is active.
    pub help_search_active: bool,

    pub canvas: CanvasState,
    pub pal: PaletteState,
    pub sim: SimState,
    pub undo: UndoState,
    pub selection: SelectionState,
    pub text_input: TextInputState,
    pub dialog: DialogState,
    pub detail: ComponentDetailState,
    /// Row cursor within the cost estimator price list.
    pub cost_cursor: usize,
    /// Whether DWV (drain-waste-vent) mode is active — shows DWV palette + validation.
    pub dwv_mode: bool,
    /// Latest DWV validation result (recomputed when dwv_mode is on and canvas changes).
    pub dwv_result: Option<DwvResult>,
}

impl App {
    pub fn new(grid_cols: usize, grid_rows: usize) -> Self {
        let splash_grid = Self::try_load_splash();
        let splash_sim  = splash_grid.as_ref().map(|g| simulate(g, FluidType::default(), &GlyphRegistry::new()));
        let mut app = Self {
            mode: AppMode::Splash,
            focus: Focus::Canvas,
            tick: 0,
            should_quit: false,
            status_msg: String::new(),
            glyph_registry: GlyphRegistry::new(),
            editor: GlyphEditorState::default(),
            layout_path: None,
            show_annotations: false,
            help_lines: Self::try_load_help(),
            help_scroll: 0,
            config: AppConfig::default(),
            help_search: String::new(),
            help_search_active: false,

            canvas: CanvasState::new(grid_cols, grid_rows),
            pal: PaletteState::default(),
            sim: SimState {
                sim_result: None,
                fluid_type: FluidType::default(),
                splash_grid,
                splash_sim,
                sim_refreshed: false,
            },
            undo: UndoState::new(),
            selection: SelectionState::new(Self::try_load_assemblies()),
            text_input: TextInputState::default(),
            dialog: DialogState::default(),
            detail: ComponentDetailState::default(),
            cost_cursor: 0,
            dwv_mode: false,
            dwv_result: None,
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

    pub fn selected_component_kind(&self) -> ComponentKind {
        self.pal.palette[self.pal.palette_idx]
    }

    // ── Simulation controls ──────────────────────────────────────────────────

    pub fn play(&mut self) {
        self.sim.sim_result = Some(simulate(&self.canvas.grid, self.sim.fluid_type, &self.glyph_registry));
        self.mode = AppMode::Simulating;
        self.status_msg = "Simulation running.".into();
    }

    pub fn stop(&mut self) {
        self.sim.sim_result = None;
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
            | AppMode::ExportDialog | AppMode::AnnotationDialog | AppMode::CostEstimator => {}
        }
    }

    pub fn on_tick(&mut self) {
        // Advance the animation clock for splash and simulation.
        // Simulation is driven by the background worker thread in main.rs.
        if matches!(self.mode, AppMode::Splash | AppMode::Simulating) {
            self.tick = self.tick.wrapping_add(1);
        }
    }

    pub fn refresh_sim(&mut self) {
        if self.mode == AppMode::Simulating || self.mode == AppMode::Paused {
            self.sim.sim_result = Some(simulate(&self.canvas.grid, self.sim.fluid_type, &self.glyph_registry));
            self.sim.sim_refreshed = true;
        }
    }

    // ── Help ─────────────────────────────────────────────────────────────────

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
        self.dialog.pre_help_mode = self.mode;
        self.help_lines = Self::try_load_help();
        self.help_scroll = 0;
        self.mode = AppMode::Help;
    }

    pub fn close_help(&mut self) {
        self.mode = self.dialog.pre_help_mode;
    }

    pub fn help_scroll_up(&mut self, n: usize) {
        self.help_scroll = self.help_scroll.saturating_sub(n);
    }

    pub fn help_scroll_down(&mut self, n: usize) {
        self.help_scroll = self.help_scroll.saturating_add(n);
    }

    // ── Undo / redo ───────────────────────────────────────────────────────────

    pub fn push_undo(&mut self) {
        self.undo.push(&self.canvas.grid);
    }

    pub fn undo(&mut self) {
        if self.undo.undo(&mut self.canvas.grid) {
            self.canvas.grid.rebuild_satellites();
            self.refresh_sim();
            self.status_msg = format!("Undo  ({} left)", self.undo.undo_count());
        } else {
            self.status_msg = "Nothing to undo.".into();
        }
    }

    pub fn redo(&mut self) {
        if self.undo.redo(&mut self.canvas.grid) {
            self.canvas.grid.rebuild_satellites();
            self.refresh_sim();
            self.status_msg = format!("Redo  ({} redo left)", self.undo.redo_count());
        } else {
            self.status_msg = "Nothing to redo.".into();
        }
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub fn component_at_cursor(&self) -> Option<&Component> {
        let (r, c) = self.canvas.cursor;
        self.canvas.grid.get(r, c)
    }

    pub fn flow_state_at_cursor(&self) -> Option<&crate::simulation::FlowState> {
        let (r, c) = self.canvas.cursor;
        self.sim.sim_result.as_ref()?.cell_states.get(&(r, c))
    }

    pub fn flow_data_at_cursor(&self) -> Option<&NodeFlowData> {
        let (r, c) = self.canvas.cursor;
        self.sim.sim_result.as_ref()?.flow_data.get(&(r, c))
    }

    // ── Jump to content ───────────────────────────────────────────────────────

    /// Jump to the topmost–leftmost cell that has content, or (0,0) if empty.
    pub fn jump_to_content_start(&mut self, viewport_h: usize, viewport_w: usize) {
        let target = (0..self.canvas.grid.height)
            .find_map(|r| {
                (0..self.canvas.grid.width)
                    .find(|&c| self.canvas.grid.get(r, c).is_some())
                    .map(|c| (r, c))
            })
            .unwrap_or((0, 0));
        self.canvas.cursor = target;
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    /// Jump to the bottommost–rightmost cell that has content, or grid origin if empty.
    pub fn jump_to_content_end(&mut self, viewport_h: usize, viewport_w: usize) {
        let target = (0..self.canvas.grid.height)
            .rev()
            .find_map(|r| {
                (0..self.canvas.grid.width)
                    .rev()
                    .find(|&c| self.canvas.grid.get(r, c).is_some())
                    .map(|c| (r, c))
            })
            .unwrap_or((0, 0));
        self.canvas.cursor = target;
        self.scroll_viewport_to_cursor(viewport_h, viewport_w);
    }

    // ── BOM view ─────────────────────────────────────────────────────────────

    pub fn enter_bom(&mut self) {
        self.dialog.pre_bom_mode = self.mode;
        self.mode = AppMode::BomView;
    }

    pub fn exit_bom(&mut self) {
        self.mode = self.dialog.pre_bom_mode;
    }

    // ── Splash / assemblies loaders ───────────────────────────────────────────

    pub(super) fn try_load_assemblies() -> AssemblyLibrary {
        AssemblyLibrary::load(std::path::Path::new("assemblies.json"))
            .unwrap_or_default()
    }

    pub(super) fn try_load_splash() -> Option<Grid> {
        let text = std::fs::read_to_string("splash.json").ok()?;
        let mut grid: Grid = serde_json::from_str(&text).ok()?;
        grid.rebuild_satellites();
        Some(grid)
    }

    // ── Export dialog opener ──────────────────────────────────────────────────

    pub fn open_export_dialog(&mut self) {
        self.mode = AppMode::ExportDialog;
    }
}
