use crate::app::InputMode;
use crate::components::ComponentKind;

pub struct TextInputState {
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub note_cursor_pos: usize,
    pub note_scroll_row: usize,
    pub note_scroll_col: usize,
    pub pending_annotation: Option<(ComponentKind, String)>,
    pub edit_annotation_pos: Option<(usize, usize)>,
    pub pending_link_path: Option<String>,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            note_cursor_pos: 0,
            note_scroll_row: 0,
            note_scroll_col: 0,
            pending_annotation: None,
            edit_annotation_pos: None,
            pending_link_path: None,
        }
    }
}
