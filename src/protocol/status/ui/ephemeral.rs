use crate::protocol::status::{AppMode, InputMode, LogEntry, SubpageTab};

#[derive(Debug, Clone)]
pub struct ModbusEphemeral {
    pub selected: usize,
    pub subpage_active: bool,
    pub subpage_form: Option<crate::protocol::status::SubpageForm>,
    pub subpage_tab_index: SubpageTab,
    pub logs: Vec<LogEntry>,
    pub log_selected: usize,
    pub log_view_offset: usize,
    pub log_auto_scroll: bool,
    pub log_clear_pending: bool,
    pub input_mode: InputMode,
    pub input_editing: bool,
    pub input_buffer: String,
    pub app_mode: AppMode,
    pub cursor: usize,
    pub master_input_buffer: String,
    pub master_cursor: usize,
}

impl Default for ModbusEphemeral {
    fn default() -> Self {
        Self {
            selected: 0,
            subpage_active: false,
            subpage_form: None,
            subpage_tab_index: SubpageTab::Config,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            log_clear_pending: false,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
            app_mode: AppMode::Modbus,
            cursor: 0,
            master_input_buffer: String::new(),
            master_cursor: 0,
        }
    }
}
