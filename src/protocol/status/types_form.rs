use crate::protocol::status::{EditingField, MasterEditField, RegisterEntry};

#[derive(Debug, Clone)]
pub struct SubpageForm {
    pub editing: bool,
    pub loop_enabled: bool,
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: serialport::Parity,
    pub cursor: usize,
    pub editing_field: Option<EditingField>,
    pub input_buffer: String,
    pub edit_choice_index: Option<usize>,
    pub edit_confirmed: bool,
    pub registers: Vec<RegisterEntry>,
    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
    pub poll_round_index: usize,
    pub in_flight_reg_index: Option<usize>,
    pub global_interval_ms: u64,
    pub global_timeout_ms: u64,
}
impl Default for SubpageForm {
    fn default() -> Self {
        Self {
            editing: false,
            loop_enabled: false,
            baud: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: serialport::Parity::None,
            cursor: 0,
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,
            registers: Vec::new(),
            master_cursor: 0,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_edit_index: None,
            master_input_buffer: String::new(),
            poll_round_index: 0,
            in_flight_reg_index: None,
            global_interval_ms: 1000,
            global_timeout_ms: 3000,
        }
    }
}
