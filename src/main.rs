mod app;
mod assembly;
mod components;
mod config;
mod file_dialog;
mod fluid;
mod glyphs;
mod grid;
mod simulation;
mod ui;

use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode, Focus, InputMode, GRID_COLS_MIN, GRID_ROWS_MIN};
use components::{ComponentKind};
use glyphs::{GlyphEditorFocus, PortKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Size the grid to fill the current terminal, with enforced minimums.
    let initial_size = terminal.size()?;
    let grid_cols = ((initial_size.width as f32 * 0.72) as usize)
        .saturating_sub(2)
        .max(GRID_COLS_MIN);
    let grid_rows = (initial_size.height.saturating_sub(11) as usize).max(GRID_ROWS_MIN);
    let mut app = App::new(grid_cols, grid_rows);
    app.load_config();
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        let size = terminal.size()?;
        let canvas_h = size.height.saturating_sub(11) as usize;
        let canvas_w = (size.width as f32 * 0.72) as usize - 2;

        terminal.draw(|f| ui::render(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Double-tap fix: ignore key-release and key-repeat events.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(&mut app, key.code, key.modifiers, canvas_h, canvas_w);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

fn handle_key(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
    canvas_h: usize,
    canvas_w: usize,
) {
    // Ctrl-C always quits.
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Ctrl+Z undo / Ctrl+Y redo — global, available in build modes.
    if modifiers.contains(KeyModifiers::CONTROL) {
        if code == KeyCode::Char('z') {
            if matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused) {
                app.undo();
                return;
            }
        }
        if code == KeyCode::Char('y') {
            if matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused) {
                app.redo();
                return;
            }
        }
    }

    // Any key dismisses the splash screen.
    if app.mode == AppMode::Splash {
        app.mode = AppMode::Build;
        return;
    }

    // Ctrl+S / Ctrl+O — open the file dialog (unavailable while typing).
    if !matches!(app.input_mode, InputMode::EditingText(_) | InputMode::EditingLength) {
        if code == KeyCode::Char('s') && modifiers.contains(KeyModifiers::CONTROL) {
            if !matches!(app.mode, AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit | AppMode::ExportDialog | AppMode::AnnotationDialog) {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveLayout);
            }
            return;
        }
        if code == KeyCode::Char('o') && modifiers.contains(KeyModifiers::CONTROL) {
            if !matches!(app.mode, AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit | AppMode::ExportDialog | AppMode::AnnotationDialog) {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.open_file_dialog(FileDialogMode::Open, FileDialogPurpose::LoadLayout);
            }
            return;
        }
    }

    // Text-input mode intercepts all keys (for save/load filename & component name).
    if matches!(app.input_mode, InputMode::EditingText(_)) {
        match code {
            // Shift+Enter inserts a line break in note text; plain Enter confirms.
            KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => app.push_note_newline(),
            KeyCode::Enter     => app.commit_text_input(),
            KeyCode::Esc       => app.cancel_input(),
            KeyCode::Backspace => app.pop_input_char(),
            // Arrow keys navigate within the note text area.
            KeyCode::Up    if app.is_note_text_mode()  => app.note_move_up(),
            KeyCode::Down  if app.is_note_text_mode()  => app.note_move_down(),
            KeyCode::Left  if app.is_note_text_mode()  => app.note_move_left(),
            KeyCode::Right if app.is_note_text_mode()  => app.note_move_right(),
            // Left/Right navigate within the label input field.
            KeyCode::Left  if app.is_label_text_mode() => app.label_move_left(),
            KeyCode::Right if app.is_label_text_mode() => app.label_move_right(),
            KeyCode::Char(ch)  => app.push_input_char(ch),
            _ => {}
        }
        return;
    }

    // Length-edit mode intercepts all keys.
    if app.input_mode == InputMode::EditingLength {
        match code {
            KeyCode::Enter     => app.commit_length_input(),
            KeyCode::Esc       => app.cancel_input(),
            KeyCode::Backspace => app.pop_input_char(),
            KeyCode::Char(ch)  => app.push_input_char(ch),
            _ => {}
        }
        return;
    }

    // C — open settings screen.
    if code == KeyCode::Char('c') && !modifiers.contains(KeyModifiers::CONTROL) {
        if app.mode == AppMode::Settings {
            app.close_settings();
        } else if !matches!(
            app.mode,
            AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit
            | AppMode::GlyphEditor | AppMode::ExportDialog | AppMode::AnnotationDialog
        ) {
            app.open_settings();
        }
        return;
    }

    // Settings screen intercepts all keys.
    if app.mode == AppMode::Settings {
        handle_settings_key(app, code);
        return;
    }

    // H — toggle dedicated help screen (hot-reloads help.txt each open).
    if code == KeyCode::Char('h') || code == KeyCode::Char('H') {
        if app.mode == AppMode::Help {
            app.close_help();
        } else {
            app.open_help();
        }
        return;
    }

    // Help screen intercepts scroll and close keys.
    if app.mode == AppMode::Help {
        match code {
            KeyCode::Up       => app.help_scroll_up(1),
            KeyCode::Down     => app.help_scroll_down(1),
            KeyCode::PageUp   => app.help_scroll_up(20),
            KeyCode::PageDown => app.help_scroll_down(20),
            KeyCode::Home     => app.help_scroll = 0,
            KeyCode::End      => { app.help_scroll = app.help_lines.len(); }
            _                 => app.close_help(),
        }
        return;
    }

    // Confirm-new dialog: S=save+new, D=discard+new, C/Esc=cancel, arrows select.
    if app.mode == AppMode::ConfirmNew {
        match code {
            KeyCode::Up => {
                app.confirm_new_choice = app.confirm_new_choice.saturating_sub(1);
            }
            KeyCode::Down => {
                app.confirm_new_choice = (app.confirm_new_choice + 1).min(2);
            }
            KeyCode::Enter => {
                match app.confirm_new_choice {
                    0 => {
                        use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                        app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenNew);
                    }
                    1 => app.do_new_diagram(),
                    _ => app.mode = AppMode::Build,
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenNew);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => app.do_new_diagram(),
            _ => app.mode = AppMode::Build, // C, Esc, any other key = cancel
        }
        return;
    }

    // Quit confirmation — S=save+quit, Q=quit, anything else=cancel.
    if app.mode == AppMode::ConfirmQuit {
        match code {
            KeyCode::Up => {
                app.confirm_quit_choice = app.confirm_quit_choice.saturating_sub(1);
            }
            KeyCode::Down => {
                app.confirm_quit_choice = (app.confirm_quit_choice + 1).min(2);
            }
            KeyCode::Enter => {
                match app.confirm_quit_choice {
                    0 => {
                        use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                        app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenQuit);
                    }
                    1 => app.should_quit = true,
                    _ => app.mode = app.pre_quit_mode,
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenQuit);
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
            _ => app.mode = app.pre_quit_mode,
        }
        return;
    }

    // File dialog intercepts all keys.
    if app.mode == AppMode::FileDialog {
        match code {
            KeyCode::Esc          => app.cancel_file_dialog(),
            KeyCode::Up           => app.file_dialog_nav(-1),
            KeyCode::Down         => app.file_dialog_nav(1),
            KeyCode::PageUp       => app.file_dialog_page_up(),
            KeyCode::PageDown     => app.file_dialog_page_down(),
            KeyCode::Home         => app.file_dialog_home(),
            KeyCode::End          => app.file_dialog_end(),
            KeyCode::Left | KeyCode::Backspace => app.file_dialog_backspace(),
            KeyCode::Tab          => app.file_dialog_toggle_focus(),
            KeyCode::Enter        => app.file_dialog_confirm(),
            KeyCode::Char(ch)     => app.file_dialog_type_char(ch),
            _ => {}
        }
        return;
    }

    // Export dialog — [T] text, [J] JSON, Esc=cancel.
    if app.mode == AppMode::ExportDialog {
        match code {
            KeyCode::Char('t') | KeyCode::Char('T') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.mode = app.pre_dialog_mode;
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::ExportText);
                if let Some(fd) = app.file_dialog.as_mut() {
                    fd.filename_input = "diagram.txt".to_string();
                    fd.focus_input = true;
                }
            }
            KeyCode::Char('j') | KeyCode::Char('J') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.mode = app.pre_dialog_mode;
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::ExportJson);
                if let Some(fd) = app.file_dialog.as_mut() {
                    if fd.filename_input.is_empty() {
                        fd.filename_input = "diagram.json".to_string();
                    }
                    fd.focus_input = true;
                }
            }
            _ => { app.mode = app.pre_dialog_mode; }
        }
        return;
    }

    // N — new diagram (prompts to save if grid has content).
    if code == KeyCode::Char('n') || code == KeyCode::Char('N') {
        if !matches!(app.mode, AppMode::GlyphEditor | AppMode::ExportDialog | AppMode::AnnotationDialog) {
            app.new_diagram();
            return;
        }
    }

    // Glyph editor has its own key handler.
    if app.mode == AppMode::GlyphEditor {
        handle_glyph_editor_key(app, code, modifiers);
        return;
    }

    // BOM view intercepts all keys except its own dismiss bindings.
    if app.mode == AppMode::BomView {
        if code == KeyCode::Char('q') || code == KeyCode::Char('Q')
            || code == KeyCode::Char('b') || code == KeyCode::Char('B')
        {
            app.exit_bom();
        }
        return;
    }

    // Selection mode — arrow keys extend the rect, Enter/R saves, Esc cancels.
    if app.mode == AppMode::Selecting {
        let shift = modifiers.contains(KeyModifiers::SHIFT);
        match code {
            KeyCode::Up    => app.move_cursor(-1, 0, canvas_h, canvas_w),
            KeyCode::Down  => app.move_cursor(1, 0, canvas_h, canvas_w),
            KeyCode::Left  => app.move_cursor(0, -1, canvas_h, canvas_w),
            KeyCode::Right => app.move_cursor(0, 1, canvas_h, canvas_w),
            KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => app.confirm_selection(),
            KeyCode::Esc   | KeyCode::Char('q') | KeyCode::Char('Q') => app.cancel_selection(),
            _ => {}
        }
        let _ = shift;
        return;
    }

    // Assembly browser — navigate, stamp, delete, close.
    if app.mode == AppMode::AssemblyBrowser {
        match code {
            KeyCode::Up       => app.assembly_browser_up(),
            KeyCode::Down     => app.assembly_browser_down(),
            KeyCode::PageUp   => {
                app.assembly_idx = app.assembly_idx.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let last = app.assembly_lib.assemblies.len().saturating_sub(1);
                app.assembly_idx = (app.assembly_idx + 10).min(last);
            }
            KeyCode::Home => { app.assembly_idx = 0; }
            KeyCode::End  => {
                let last = app.assembly_lib.assemblies.len().saturating_sub(1);
                app.assembly_idx = last;
            }
            KeyCode::Enter                      => app.begin_stamp(),
            KeyCode::Delete | KeyCode::Backspace => app.delete_assembly(),
            KeyCode::Char('y') | KeyCode::Char('Y')
            | KeyCode::Char('q') | KeyCode::Char('Q') => app.exit_assembly_browser(),
            _ => {}
        }
        return;
    }

    // Stamp mode — position with arrow keys, Enter to confirm, Esc to cancel.
    if app.mode == AppMode::Stamping {
        match code {
            KeyCode::Up    => app.move_cursor(-1, 0, canvas_h, canvas_w),
            KeyCode::Down  => app.move_cursor(1, 0, canvas_h, canvas_w),
            KeyCode::Left  => app.move_cursor(0, -1, canvas_h, canvas_w),
            KeyCode::Right => app.move_cursor(0, 1, canvas_h, canvas_w),
            KeyCode::Enter                       => app.confirm_stamp(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => app.cancel_stamp(),
            _ => {}
        }
        return;
    }

    // Component detail overlay — navigate ports and edit stub lengths.
    if app.mode == AppMode::ComponentDetail {
        match code {
            KeyCode::Up       => app.component_detail_nav(-1),
            KeyCode::Down     => app.component_detail_nav(1),
            KeyCode::PageUp   => app.component_detail_page_up(),
            KeyCode::PageDown => app.component_detail_page_down(),
            KeyCode::Home     => app.component_detail_home(),
            KeyCode::End      => app.component_detail_end(),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Char('L') => app.begin_port_length_edit(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => app.exit_component_detail(),
            _ => {}
        }
        return;
    }

    // Quit — show confirmation dialog instead of quitting immediately.
    if code == KeyCode::Char('q') || code == KeyCode::Char('Q') {
        app.pre_quit_mode = app.mode;
        app.confirm_quit_choice = 0;
        app.mode = AppMode::ConfirmQuit;
        return;
    }

    // G — open glyph editor.
    if code == KeyCode::Char('g') || code == KeyCode::Char('G') {
        app.enter_glyph_editor();
        return;
    }

    // B — bill of materials overlay.
    if code == KeyCode::Char('b') || code == KeyCode::Char('B') {
        app.enter_bom();
        return;
    }

    // R — start rectangle selection for saving as assembly.
    if code == KeyCode::Char('r') || code == KeyCode::Char('R') {
        app.start_selecting();
        return;
    }

    // Y — open assembly browser.
    if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
        app.enter_assembly_browser();
        return;
    }

    // A — toggle dimension annotations.
    if code == KeyCode::Char('a') || code == KeyCode::Char('A') {
        app.toggle_annotations();
        return;
    }

    // X — open export dialog.
    if code == KeyCode::Char('x') || code == KeyCode::Char('X') {
        if matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused) {
            app.pre_dialog_mode = app.mode;
            app.open_export_dialog();
            return;
        }
    }

    // 1-6 — direct material selection (also applies to component at cursor).
    if let KeyCode::Char(c) = code {
        if matches!(c, '1'..='6') {
            app.set_material_by_index((c as u8 - b'1') as usize);
            return;
        }
    }

    // F — cycle fluid type (available outside glyph editor, any mode).
    if code == KeyCode::Char('f') || code == KeyCode::Char('F') {
        app.cycle_fluid_type();
        return;
    }

    // Simulation controls — always available.
    match code {
        KeyCode::Char('p') | KeyCode::Char('P') => {
            app.play();
            return;
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if app.mode != AppMode::Build {
                app.stop();
                return;
            }
        }
        KeyCode::Char(' ') => {
            if app.mode != AppMode::Build {
                app.pause_toggle();
                return;
            }
        }
        _ => {}
    }

    // Tab cycles focus: Canvas → Palette (component list) → PaletteColors (material/color) → Canvas
    if code == KeyCode::Tab {
        app.focus = match app.focus {
            Focus::Canvas        => Focus::Palette,
            Focus::Palette       => Focus::PaletteColors,
            Focus::PaletteColors => Focus::Canvas,
        };
        return;
    }

    let shift = modifiers.contains(KeyModifiers::SHIFT);
    match app.focus {
        Focus::Canvas        => handle_canvas_key(app, code, shift, canvas_h, canvas_w),
        Focus::Palette       => handle_palette_key(app, code, shift),
        Focus::PaletteColors => handle_palette_colors_key(app, code),
    }
}

fn handle_canvas_key(
    app: &mut App,
    code: KeyCode,
    shift: bool,
    canvas_h: usize,
    canvas_w: usize,
) {
    match code {
        KeyCode::Up    => app.move_cursor(-1, 0, canvas_h, canvas_w),
        KeyCode::Down  => app.move_cursor(1, 0, canvas_h, canvas_w),
        KeyCode::Left  => app.move_cursor(0, -1, canvas_h, canvas_w),
        KeyCode::Right => app.move_cursor(0, 1, canvas_h, canvas_w),
        KeyCode::Home  => app.jump_to_content_start(canvas_h, canvas_w),
        KeyCode::End   => app.jump_to_content_end(canvas_h, canvas_w),
        KeyCode::Enter => {
            if app.pending_annotation.is_some() {
                app.place_pending_annotation();
            } else {
                match app.selected_component_kind() {
                    ComponentKind::Label => app.begin_label_placement(),
                    ComponentKind::Note  => app.begin_note_placement(),
                    _                    => app.place_component(),
                }
            }
        }
        KeyCode::Esc => {
            if app.pending_annotation.is_some() {
                app.cancel_pending_annotation();
            }
        }
        KeyCode::Delete | KeyCode::Backspace => {
            if app.pending_annotation.is_none() {
                app.delete_component();
            }
        }
        KeyCode::Char('v') | KeyCode::Char('V') => app.toggle_valve_at_cursor(),
        KeyCode::Char('m') | KeyCode::Char('M') => app.cycle_material_at_cursor(),
        KeyCode::Char('d') | KeyCode::Char('D') => app.cycle_diameter(),
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.adjust_length_at_cursor(if shift { 6.0 } else { 1.0 });
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.adjust_length_at_cursor(if shift { -6.0 } else { -1.0 });
        }
        KeyCode::Char('l') | KeyCode::Char('L') => app.begin_length_edit(),
        KeyCode::Char('t') | KeyCode::Char('T') => app.cycle_drain_type_at_cursor(),
        KeyCode::Char('i') => app.adjust_source_pressure_at_cursor(10.0),
        KeyCode::Char('I') => app.adjust_source_pressure_at_cursor(-10.0),
        KeyCode::Char('[') => app.palette_up(),
        KeyCode::Char(']') => app.palette_down(),
        KeyCode::Char('e') | KeyCode::Char('E') => {
            let (r, c) = app.cursor;
            match app.grid.get(r, c).map(|co| co.kind) {
                Some(ComponentKind::Label) => app.begin_label_placement(),
                Some(ComponentKind::Note)  => app.begin_note_placement(),
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_palette_key(app: &mut App, code: KeyCode, shift: bool) {
    match code {
        // Enter returns focus to the canvas — pick a component, press Enter, keep building.
        KeyCode::Enter => { app.focus = Focus::Canvas; }
        KeyCode::Up       => app.palette_up(),
        KeyCode::Down     => app.palette_down(),
        KeyCode::Home     => app.palette_home(),
        KeyCode::End      => app.palette_end(),
        KeyCode::PageUp   => app.palette_page_up(),
        KeyCode::PageDown => app.palette_page_down(),
        KeyCode::Char('d') | KeyCode::Char('D') => app.cycle_diameter(),
        KeyCode::Char('m') | KeyCode::Char('M') => app.cycle_material_at_cursor(),
        KeyCode::Char('l') | KeyCode::Char('L') => {
            if matches!(app.selected_component_kind(), ComponentKind::PipeH | ComponentKind::PipeV) {
                app.begin_length_edit();
            } else {
                app.enter_palette_component_detail();
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.adjust_palette_kind_length(if shift { 6.0 } else { 1.0 });
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.adjust_palette_kind_length(if shift { -6.0 } else { -1.0 });
        }
        _ => {}
    }
}

fn handle_palette_colors_key(app: &mut App, code: KeyCode) {
    let color_active = app.selected_component_kind().supports_color_override();
    match code {
        KeyCode::Enter => { app.focus = Focus::Canvas; }
        // Up/Down: navigate color grid rows when color override is active,
        // otherwise step through the material list.
        KeyCode::Up   if color_active  => app.palette_color_nav(-1, 0),
        KeyCode::Down if color_active  => app.palette_color_nav(1, 0),
        KeyCode::Up                    => app.nav_material(-1),
        KeyCode::Down                  => app.nav_material(1),
        // Left/Right always navigate color grid columns (no-op when inactive).
        KeyCode::Left  if color_active => app.palette_color_nav(0, -1),
        KeyCode::Right if color_active => app.palette_color_nav(0, 1),
        KeyCode::Home if !color_active => app.nav_material_home(),
        KeyCode::End  if !color_active => app.nav_material_end(),
        KeyCode::Char('e') | KeyCode::Char('E') if color_active => app.palette_begin_custom_rgb(),
        KeyCode::Char('m') | KeyCode::Char('M') => app.cycle_material_at_cursor(),
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up       => app.settings_nav(-1),
        KeyCode::Down     => app.settings_nav(1),
        KeyCode::PageUp   => app.settings_nav(-10),
        KeyCode::PageDown => app.settings_nav(10),
        KeyCode::Home     => app.settings_home(),
        KeyCode::End      => app.settings_end(),
        KeyCode::Char('a') | KeyCode::Char('A') => app.settings_begin_add(),
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => app.settings_remove(),
        KeyCode::Char('l') | KeyCode::Char('L') => app.settings_load_now(),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('c') | KeyCode::Char('C')
        | KeyCode::Esc => app.close_settings(),
        _ => {}
    }
}

fn handle_glyph_editor_key(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    match code {
        // Exit glyph editor
        KeyCode::Char('q') | KeyCode::Char('Q')
        | KeyCode::Char('g') | KeyCode::Char('G') => {
            app.exit_glyph_editor();
        }
        // Cycle focus between panels
        KeyCode::Tab => app.editor_cycle_focus(),
        // Navigate within the focused panel (arrow keys only — avoids letter conflicts)
        KeyCode::Up    => app.editor_nav(-1, 0),
        KeyCode::Down  => app.editor_nav(1, 0),
        KeyCode::Left  => app.editor_nav(0, -1),
        KeyCode::Right => app.editor_nav(0, 1),
        KeyCode::Home  => app.editor_nav_home(),
        KeyCode::End   => app.editor_nav_end(),
        // Apply current char + color as a glyph override for the selected component
        KeyCode::Enter => app.editor_apply_glyph(),
        // Scope selectors (which materials / diameters this override targets)
        KeyCode::Char('m') | KeyCode::Char('M') => app.editor_cycle_mat_scope(),
        // [D] = Drain port when editing a composite tile, diam scope otherwise
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if app.editor.focus == GlyphEditorFocus::CompositeGrid {
                app.editor_set_port(PortKind::Drain);
            } else {
                app.editor_cycle_diam_scope();
            }
        }
        // Library I/O (open text-input prompts)
        KeyCode::Char('s') | KeyCode::Char('S') => app.editor_begin_save(),
        KeyCode::Char('l') | KeyCode::Char('L') => app.editor_begin_load(),
        // Enter a custom RGB color value
        KeyCode::Char('e') | KeyCode::Char('E') => app.editor_begin_custom_rgb(),
        // Define a new custom component using current char + color
        KeyCode::Char('n') | KeyCode::Char('N') => app.editor_begin_new_comp(),
        // Set composite width for the selected custom component
        KeyCode::Char('w') | KeyCode::Char('W') => app.editor_begin_set_composite_width(),
        // Clear the tile under the composite cursor
        KeyCode::Delete | KeyCode::Backspace => app.editor_clear_composite_cell(),
        // Port placement — direct type selection on box border cells
        KeyCode::Char('i') | KeyCode::Char('I') => app.editor_set_port(PortKind::Inlet),
        KeyCode::Char('o') | KeyCode::Char('O') => app.editor_set_port(PortKind::Outlet),
        _ => {}
    }
}
