use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, AppMode, Focus, InputMode};
use crate::components::ComponentKind;
use crate::glyphs::{EditorDisplayRow, GlyphEditorFocus, PortKind};

pub fn handle_key(
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
        if code == KeyCode::Char('z')
            && matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused)
        {
            app.undo();
            return;
        }
        if code == KeyCode::Char('y')
            && matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused)
        {
            app.redo();
            return;
        }
    }

    // Any key dismisses the splash screen.
    if app.mode == AppMode::Splash {
        app.mode = AppMode::Build;
        return;
    }

    // Ctrl+S / Ctrl+O — open the file dialog (unavailable while typing).
    if !matches!(app.text_input.input_mode, InputMode::EditingText(_) | InputMode::EditingLength) {
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
    if matches!(app.text_input.input_mode, InputMode::EditingText(_)) {
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
            // Left/Right navigate within the label / link-path input fields.
            KeyCode::Left  if app.is_label_text_mode() || app.is_link_path_mode() => app.label_move_left(),
            KeyCode::Right if app.is_label_text_mode() || app.is_link_path_mode() => app.label_move_right(),
            KeyCode::Char(ch)  => app.push_input_char(ch),
            _ => {}
        }
        return;
    }

    // Length-edit mode intercepts all keys.
    if app.text_input.input_mode == InputMode::EditingLength {
        match code {
            KeyCode::Enter     => app.commit_length_input(),
            KeyCode::Esc       => app.cancel_input(),
            KeyCode::Backspace => app.pop_input_char(),
            KeyCode::Char(ch)  => app.push_input_char(ch),
            _ => {}
        }
        return;
    }

    // ── Search short-circuits ─────────────────────────────────────────────────
    // When palette or help search is active, route all keys directly to their
    // handler BEFORE any global letter hotkeys ('c', 'h', 'n', 'q', etc.) can
    // intercept them.
    if app.focus == Focus::Palette && app.pal.palette_search_active {
        let shift = modifiers.contains(KeyModifiers::SHIFT);
        handle_palette_key(app, code, shift);
        return;
    }
    if app.mode == AppMode::Help && app.help_search_active {
        match code {
            KeyCode::Esc => {
                app.help_search_active = false;
                app.help_search.clear();
            }
            KeyCode::Backspace => {
                app.help_search.pop();
                app.help_search_jump_first();
            }
            KeyCode::Up | KeyCode::PageUp   => app.help_search_prev(),
            KeyCode::Down | KeyCode::PageDown => app.help_search_next(),
            KeyCode::Char(ch) => {
                app.help_search.push(ch);
                app.help_search_jump_first();
            }
            _ => {}
        }
        return;
    }

    // C — open settings screen.
    // Excluded modes (GlyphEditor, ExportDialog, AnnotationDialog, FileDialog,
    // ConfirmNew, ConfirmQuit) handle 'c' themselves — fall through to their handlers.
    if code == KeyCode::Char('c') && !modifiers.contains(KeyModifiers::CONTROL) {
        if app.mode == AppMode::Settings {
            app.close_settings();
            return;
        } else if !matches!(
            app.mode,
            AppMode::FileDialog | AppMode::ConfirmNew | AppMode::ConfirmQuit
            | AppMode::GlyphEditor | AppMode::ExportDialog | AppMode::AnnotationDialog
        ) {
            app.open_settings();
            return;
        }
        // Fall through — mode-specific handler will see 'c'.
    }

    // Settings screen intercepts all keys.
    if app.mode == AppMode::Settings {
        handle_settings_key(app, code);
        return;
    }

    // ? — toggle dedicated help screen (hot-reloads help.txt each open).
    if code == KeyCode::Char('?') {
        if app.mode == AppMode::Help {
            app.close_help();
        } else {
            app.open_help();
        }
        return;
    }

    // Help screen intercepts scroll and close keys (search is handled earlier).
    if app.mode == AppMode::Help {
        // [/] activates search.
        if code == KeyCode::Char('/') {
            app.help_search_active = true;
            app.help_search.clear();
            return;
        }
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
    // Also used when following a Link — pending_link_path distinguishes the two flows.
    if app.mode == AppMode::ConfirmNew {
        match code {
            KeyCode::Up => {
                app.dialog.confirm_new_choice = app.dialog.confirm_new_choice.saturating_sub(1);
            }
            KeyCode::Down => {
                app.dialog.confirm_new_choice = (app.dialog.confirm_new_choice + 1).min(2);
            }
            KeyCode::Enter => {
                match app.dialog.confirm_new_choice {
                    0 => {
                        use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                        let purpose = if app.text_input.pending_link_path.is_some() {
                            FileDialogPurpose::SaveThenFollowLink
                        } else {
                            FileDialogPurpose::SaveThenNew
                        };
                        app.open_file_dialog(FileDialogMode::Save, purpose);
                    }
                    1 => {
                        if app.text_input.pending_link_path.is_some() {
                            app.do_follow_link();
                        } else {
                            app.do_new_diagram();
                        }
                    }
                    _ => {
                        app.text_input.pending_link_path = None;
                        app.mode = AppMode::Build;
                    }
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                let purpose = if app.text_input.pending_link_path.is_some() {
                    FileDialogPurpose::SaveThenFollowLink
                } else {
                    FileDialogPurpose::SaveThenNew
                };
                app.open_file_dialog(FileDialogMode::Save, purpose);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if app.text_input.pending_link_path.is_some() {
                    app.do_follow_link();
                } else {
                    app.do_new_diagram();
                }
            }
            _ => {
                app.text_input.pending_link_path = None;
                app.mode = AppMode::Build;
            }
        }
        return;
    }

    // Quit confirmation — S=save+quit, Q=quit, anything else=cancel.
    if app.mode == AppMode::ConfirmQuit {
        match code {
            KeyCode::Up => {
                app.dialog.confirm_quit_choice = app.dialog.confirm_quit_choice.saturating_sub(1);
            }
            KeyCode::Down => {
                app.dialog.confirm_quit_choice = (app.dialog.confirm_quit_choice + 1).min(2);
            }
            KeyCode::Enter => {
                match app.dialog.confirm_quit_choice {
                    0 => {
                        use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                        app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenQuit);
                    }
                    1 => app.should_quit = true,
                    _ => app.mode = app.dialog.pre_quit_mode,
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::SaveThenQuit);
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
            _ => app.mode = app.dialog.pre_quit_mode,
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
                app.mode = app.dialog.pre_dialog_mode;
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::ExportText);
                if let Some(fd) = app.dialog.file_dialog.as_mut() {
                    fd.filename_input = "diagram.txt".to_string();
                    fd.focus_input = true;
                }
            }
            KeyCode::Char('j') | KeyCode::Char('J') => {
                use crate::file_dialog::{FileDialogMode, FileDialogPurpose};
                app.mode = app.dialog.pre_dialog_mode;
                app.open_file_dialog(FileDialogMode::Save, FileDialogPurpose::ExportJson);
                if let Some(fd) = app.dialog.file_dialog.as_mut() {
                    if fd.filename_input.is_empty() {
                        fd.filename_input = "diagram.json".to_string();
                    }
                    fd.focus_input = true;
                }
            }
            _ => { app.mode = app.dialog.pre_dialog_mode; }
        }
        return;
    }

    // N — new diagram (prompts to save if grid has content).
    if (code == KeyCode::Char('n') || code == KeyCode::Char('N'))
        && !matches!(app.mode, AppMode::GlyphEditor | AppMode::ExportDialog | AppMode::AnnotationDialog)
    {
        app.new_diagram();
        return;
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

    // Cost estimator — own key handler.
    if app.mode == AppMode::CostEstimator {
        handle_cost_estimator_key(app, code);
        return;
    }

    // Selection mode — arrows extend rect; C=copy, X=move, Enter/R=save-assembly, Esc=cancel.
    if app.mode == AppMode::Selecting {
        match code {
            KeyCode::Up    => app.move_cursor(-1, 0, canvas_h, canvas_w),
            KeyCode::Down  => app.move_cursor(1, 0, canvas_h, canvas_w),
            KeyCode::Left  => app.move_cursor(0, -1, canvas_h, canvas_w),
            KeyCode::Right => app.move_cursor(0, 1, canvas_h, canvas_w),
            KeyCode::Char('c') | KeyCode::Char('C') => app.copy_selection(),
            KeyCode::Char('x') | KeyCode::Char('X') => app.move_selection(),
            KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => app.confirm_selection(),
            KeyCode::Esc   | KeyCode::Char('q') | KeyCode::Char('Q') => app.cancel_selection(),
            _ => {}
        }
        return;
    }

    // Assembly browser — navigate, stamp, delete, close.
    if app.mode == AppMode::AssemblyBrowser {
        match code {
            KeyCode::Up       => app.assembly_browser_up(),
            KeyCode::Down     => app.assembly_browser_down(),
            KeyCode::PageUp   => {
                app.selection.assembly_idx = app.selection.assembly_idx.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let last = app.selection.assembly_lib.assemblies.len().saturating_sub(1);
                app.selection.assembly_idx = (app.selection.assembly_idx + 10).min(last);
            }
            KeyCode::Home => { app.selection.assembly_idx = 0; }
            KeyCode::End  => {
                let last = app.selection.assembly_lib.assemblies.len().saturating_sub(1);
                app.selection.assembly_idx = last;
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
        app.dialog.pre_quit_mode = app.mode;
        app.dialog.confirm_quit_choice = 0;
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

    // $ — cost estimator.
    if code == KeyCode::Char('$') {
        app.open_cost_estimator();
        return;
    }

    // W — toggle DWV (drain-waste-vent) mode.
    if code == KeyCode::Char('w') || code == KeyCode::Char('W') {
        app.toggle_dwv_mode();
        return;
    }

    // R — start rectangle selection (then C=copy, X=move, Enter=save-as-assembly).
    if code == KeyCode::Char('r') || code == KeyCode::Char('R') {
        app.start_selecting();
        return;
    }

    // Y — open assembly browser (browse & stamp saved assemblies).
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
    if (code == KeyCode::Char('x') || code == KeyCode::Char('X'))
        && matches!(app.mode, AppMode::Build | AppMode::Simulating | AppMode::Paused)
    {
        app.dialog.pre_dialog_mode = app.mode;
        app.open_export_dialog();
        return;
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
            // If canvas focus and cursor is on a Source or PRV, open the pressure dialog.
            if app.focus == Focus::Canvas {
                let (r, c) = app.canvas.cursor;
                let kind = app.canvas.grid.get(r, c).map(|co| co.kind);
                if matches!(kind, Some(ComponentKind::Source) | Some(ComponentKind::PressureReducingValve)) {
                    app.begin_source_pressure_dialog();
                    return;
                }
            }
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
        // Clear palette search when leaving palette focus.
        if matches!(app.focus, Focus::Palette) {
            app.pal.palette_search_active = false;
            app.pal.palette_search.clear();
        }
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
            if app.text_input.pending_annotation.is_some() {
                app.place_pending_annotation();
            } else {
                let (r, c) = app.canvas.cursor;
                // Enter on a placed Link follows it rather than editing.
                if app.canvas.grid.get(r, c).map(|co| co.kind == ComponentKind::Link).unwrap_or(false) {
                    app.follow_link_at_cursor();
                } else {
                    match app.selected_component_kind() {
                        ComponentKind::Label => app.begin_label_placement(),
                        ComponentKind::Note  => app.begin_note_placement(),
                        ComponentKind::Link  => app.begin_link_placement(),
                        _                    => app.place_component(),
                    }
                }
            }
        }
        KeyCode::Esc => {
            if app.text_input.pending_annotation.is_some() {
                app.cancel_pending_annotation();
            }
        }
        KeyCode::Delete | KeyCode::Backspace => {
            if app.text_input.pending_annotation.is_none() {
                app.delete_component();
            }
        }
        KeyCode::Char('v') | KeyCode::Char('V') => app.toggle_valve_at_cursor(),
        KeyCode::Char('m') | KeyCode::Char('M') => app.cycle_material_at_cursor(),
        KeyCode::Char('d') | KeyCode::Char('D') => {
            let (r, c) = app.canvas.cursor;
            let is_dwv = app.canvas.grid.get(r, c).map(|co| co.kind.is_dwv()).unwrap_or(false);
            if is_dwv { app.cycle_drain_diameter_at_cursor(); } else { app.cycle_diameter(); }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.adjust_length_at_cursor(if shift { 6.0 } else { 1.0 });
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.adjust_length_at_cursor(if shift { -6.0 } else { -1.0 });
        }
        KeyCode::Char('l') | KeyCode::Char('L') => app.begin_length_edit(),
        KeyCode::Char('t') | KeyCode::Char('T') => app.cycle_drain_type_at_cursor(),
        KeyCode::Char('h') | KeyCode::Char('H') => app.cycle_line_temp_at_cursor(),
        KeyCode::Char('i') => app.adjust_source_pressure_at_cursor(1.0),
        KeyCode::Char('I') => app.adjust_source_pressure_at_cursor(-1.0),
        KeyCode::Char('p') | KeyCode::Char('P') => app.begin_source_pressure_dialog(),
        KeyCode::Char('[') => app.palette_up(),
        KeyCode::Char(']') => app.palette_down(),
        KeyCode::Char('e') | KeyCode::Char('E') => {
            let (r, c) = app.canvas.cursor;
            match app.canvas.grid.get(r, c).map(|co| co.kind) {
                Some(ComponentKind::Label) => app.begin_label_placement(),
                Some(ComponentKind::Note)  => app.begin_note_placement(),
                Some(ComponentKind::Link)  => app.begin_link_placement(),
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_palette_key(app: &mut App, code: KeyCode, shift: bool) {
    // ── Search mode intercepts most keys ──────────────────────────────────────
    if app.pal.palette_search_active {
        match code {
            KeyCode::Esc => {
                app.pal.palette_search_active = false;
                app.pal.palette_search.clear();
            }
            KeyCode::Enter => {
                app.pal.palette_search_active = false;
                app.pal.palette_search.clear();
                app.focus = Focus::Canvas;
            }
            KeyCode::Backspace => {
                app.pal.palette_search.pop();
                app.palette_search_jump_first();
            }
            KeyCode::Down => app.palette_search_next(),
            KeyCode::Up   => app.palette_search_prev(),
            KeyCode::Char(ch) => {
                app.pal.palette_search.push(ch);
                app.palette_search_jump_first();
            }
            _ => {}
        }
        return;
    }

    // ── Group picker intercept ────────────────────────────────────────────────
    if app.pal.group_picker_active {
        match code {
            KeyCode::Up => {
                if app.pal.group_picker_idx > 0 { app.pal.group_picker_idx -= 1; }
            }
            KeyCode::Down => {
                let max = app.config.groups.len(); // +1 for "[+ New Group]" at index == len
                if app.pal.group_picker_idx < max { app.pal.group_picker_idx += 1; }
            }
            KeyCode::Enter => { app.palette_confirm_group_pick(); }
            KeyCode::Esc   => { app.palette_cancel_group_picker(); }
            _ => {}
        }
        return;
    }

    // [/] activates search.
    if code == KeyCode::Char('/') {
        app.pal.palette_search_active = true;
        app.pal.palette_search.clear();
        return;
    }

    match code {
        // Enter on group header = toggle collapse; on component = return focus to canvas.
        KeyCode::Enter => {
            if app.palette_cursor_on_header() {
                app.palette_toggle_group();
            } else {
                app.focus = Focus::Canvas;
            }
        }
        // [Space]: toggle group collapse (on header) or open group picker (on component)
        KeyCode::Char(' ') => {
            if app.palette_cursor_on_header() {
                app.palette_toggle_group();
            } else {
                app.palette_open_group_picker();
            }
        }
        KeyCode::Delete => {
            if app.palette_cursor_on_header() {
                app.palette_delete_group();
            }
        }
        // [n] on a group header creates a new group
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if app.palette_cursor_on_header() {
                app.begin_new_group();
            }
        }
        KeyCode::Up       => app.palette_display_up(),
        KeyCode::Down     => app.palette_display_down(),
        KeyCode::Home     => app.palette_display_home(),
        KeyCode::End      => app.palette_display_end(),
        KeyCode::PageUp   => app.palette_display_page_up(),
        KeyCode::PageDown => app.palette_display_page_down(),
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

fn handle_cost_estimator_key(app: &mut App, code: KeyCode) {
    use crate::app::InputMode;
    use crate::app::TextEditTarget;

    // If currently editing a price, intercept all keys for the edit buffer.
    if matches!(app.text_input.input_mode, InputMode::EditingText(TextEditTarget::CostPrice)) {
        match code {
            KeyCode::Enter => app.confirm_cost_price_edit(),
            KeyCode::Esc   => app.cancel_cost_price_edit(),
            KeyCode::Backspace => { app.text_input.input_buffer.pop(); }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                app.text_input.input_buffer.push(c);
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K')   => app.cost_nav(-1),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => app.cost_nav(1),
        KeyCode::PageUp   => app.cost_nav(-10),
        KeyCode::PageDown => app.cost_nav(10),
        KeyCode::Home     => { app.cost_cursor = 0; }
        KeyCode::End      => {
            use crate::cost_config::FITTING_GROUPS;
            use crate::glyphs::{ALL_DIAMETERS, ALL_MATERIALS};
            app.cost_cursor = ALL_MATERIALS.len() * ALL_DIAMETERS.len() + FITTING_GROUPS.len() - 1;
        }
        KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') => app.begin_cost_price_edit(),
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('$') => {
            app.close_cost_estimator();
        }
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
        KeyCode::Char('g') | KeyCode::Char('G') => app.cycle_grid_scale(),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('c') | KeyCode::Char('C')
        | KeyCode::Esc => app.close_settings(),
        _ => {}
    }
}

fn handle_glyph_editor_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Intercept all keys while a delete confirmation is pending.
    if app.dialog.editor_pending_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => app.editor_confirm_delete_comp(),
            _ => app.editor_cancel_delete_comp(),
        }
        return;
    }

    // Group picker intercept (editor).
    if app.editor.group_picker_active {
        match code {
            KeyCode::Up => {
                if app.editor.group_picker_idx > 0 { app.editor.group_picker_idx -= 1; }
            }
            KeyCode::Down => {
                let max = app.config.groups.len();
                if app.editor.group_picker_idx < max { app.editor.group_picker_idx += 1; }
            }
            KeyCode::Enter => { app.editor_confirm_group_pick(); }
            KeyCode::Esc   => { app.editor_cancel_group_picker(); }
            _ => {}
        }
        return;
    }

    let _shift = modifiers.contains(KeyModifiers::SHIFT);

    match code {
        // Exit glyph editor.
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('g') | KeyCode::Char('G') => {
            app.exit_glyph_editor();
        }
        // [Space] in ComponentList: toggle group (on header) or open group picker (on component)
        KeyCode::Char(' ') if app.editor.focus == GlyphEditorFocus::ComponentList => {
            let on_header = matches!(
                app.editor.display_rows.get(app.editor.display_idx),
                Some(EditorDisplayRow::GroupHeader { .. })
            );
            if on_header {
                app.editor_toggle_group();
            } else {
                app.editor_open_group_picker();
            }
        }
        // Cycle focus between panels
        KeyCode::Tab => app.editor_cycle_focus(),
        // Navigate within the focused panel (arrow keys only — avoids letter conflicts)
        KeyCode::Up    => app.editor_nav(-1, 0),
        KeyCode::Down  => app.editor_nav(1, 0),
        KeyCode::Left  => app.editor_nav(0, -1),
        KeyCode::Right => app.editor_nav(0, 1),
        KeyCode::Home     => app.editor_nav_home(),
        KeyCode::End      => app.editor_nav_end(),
        KeyCode::PageUp   => app.editor_display_page_up(),
        KeyCode::PageDown => app.editor_display_page_down(),
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
        // Rename the currently selected custom component
        KeyCode::Char('r') | KeyCode::Char('R') => app.editor_begin_rename_comp(),
        // Duplicate the selected custom component as a new template
        KeyCode::Char('c') | KeyCode::Char('C') => app.editor_begin_copy_comp(),
        // Set composite width for the selected custom component
        KeyCode::Char('w') | KeyCode::Char('W') => app.editor_begin_set_composite_width(),
        // Del: in ComponentList — delete group if on header, else delete custom comp.
        //      In CompositeGrid — clear composite cell.
        // Backspace: same as Del but never deletes a group.
        KeyCode::Delete => {
            if app.editor.focus == GlyphEditorFocus::ComponentList {
                let on_header = matches!(
                    app.editor.display_rows.get(app.editor.display_idx),
                    Some(EditorDisplayRow::GroupHeader { .. })
                );
                if on_header {
                    app.editor_delete_group();
                } else {
                    app.editor_delete_custom_comp();
                }
            } else {
                app.editor_clear_composite_cell();
            }
        }
        KeyCode::Backspace => {
            if app.editor.focus == GlyphEditorFocus::ComponentList {
                app.editor_delete_custom_comp();
            } else {
                app.editor_clear_composite_cell();
            }
        }
        // Port placement — direct type selection on box border cells
        KeyCode::Char('i') | KeyCode::Char('I') => app.editor_set_port(PortKind::Inlet),
        KeyCode::Char('o') | KeyCode::Char('O') => app.editor_set_port(PortKind::Outlet),
        _ => {}
    }
}
