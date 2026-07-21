use crate::app::AppMode;
use crate::file_dialog::FileDialogState;

pub struct DialogState {
    pub file_dialog: Option<FileDialogState>,
    pub pre_dialog_mode: AppMode,
    pub pre_bom_mode: AppMode,
    pub pre_help_mode: AppMode,
    pub pre_detail_mode: AppMode,
    pub pre_quit_mode: AppMode,
    pub editor_pending_delete: Option<usize>,
    pub confirm_new_choice: usize,
    pub confirm_quit_choice: usize,
    pub settings_idx: usize,
    pub settings_status: String,
}

impl Default for DialogState {
    fn default() -> Self {
        Self {
            file_dialog: None,
            pre_dialog_mode: AppMode::Build,
            pre_bom_mode: AppMode::Build,
            pre_help_mode: AppMode::Build,
            pre_detail_mode: AppMode::Build,
            pre_quit_mode: AppMode::Build,
            editor_pending_delete: None,
            confirm_new_choice: 0,
            confirm_quit_choice: 0,
            settings_idx: 0,
            settings_status: String::new(),
        }
    }
}
