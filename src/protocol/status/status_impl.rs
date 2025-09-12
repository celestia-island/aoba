use super::{Status, SubpageForm};

impl Status {
    /// Create a new Status instance with default values
    pub fn new() -> Self {
        // The derive_struct macro generates a Default implementation
        Self::default()
    }

    /// Initialize the subpage form with default values
    pub fn init_subpage_form(&mut self) {
        self.page.subpage_form = Some(SubpageForm {
            registers: Vec::new(),
            master_cursor: 0,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_edit_index: None,
            master_input_buffer: String::new(),
            cursor: 0,
            loop_enabled: false,
            master_passive: None,
            editing: false,

            // Configuration fields with defaults
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,

            // Serial configuration defaults
            baud: 9600,
            parity: serialport::Parity::None,
            data_bits: 8,
            stop_bits: 1,
            global_interval_ms: 1000,
            global_timeout_ms: 5000,
        });
    }

    /// Sync form data to slave context (stub implementation)
    pub fn sync_form_to_slave_context(&mut self, _port_name: &str) {
        // TODO: Implement synchronization logic
        // This is a stub to allow compilation
    }
}
