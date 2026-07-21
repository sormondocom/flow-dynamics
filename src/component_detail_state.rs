use crate::components::ComponentKind;

pub struct ComponentDetailState {
    pub detail_port_cursor: usize,
    pub detail_kind: ComponentKind,
    pub detail_for_palette: bool,
}

impl Default for ComponentDetailState {
    fn default() -> Self {
        Self {
            detail_port_cursor: 0,
            detail_kind: ComponentKind::PipeH,
            detail_for_palette: false,
        }
    }
}
