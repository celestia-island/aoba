pub mod modbus;
pub mod port;
pub mod ui;

use chrono::{DateTime, Local};
use std::collections::HashMap;

use crate::protocol::status::types::{
    self,
    ui::{AboutStatus, EntryStatus, ModbusConfigStatus, ModbusDashboardStatus, ModbusLogStatus},
};

impl Status {
    pub fn snapshot_entry(&self) -> EntryStatus {
        match &self.page {
            types::Page::Entry { cursor } => EntryStatus { cursor: *cursor },
            _ => EntryStatus { cursor: None },
        }
    }

    pub fn snapshot_modbus_config(&self) -> ModbusConfigStatus {
        match &self.page {
            types::Page::ModbusConfig {
                selected_port,
                edit_active,
                edit_port,
                edit_field_index,
                edit_field_key,
                edit_buffer,
                edit_cursor_pos,
                ..
            } => ModbusConfigStatus {
                selected_port: *selected_port,
                edit_active: *edit_active,
                edit_port: edit_port.clone(),
                edit_field_index: *edit_field_index,
                edit_field_key: edit_field_key.clone(),
                edit_buffer: edit_buffer.clone(),
                edit_cursor_pos: *edit_cursor_pos,
            },
            _ => ModbusConfigStatus {
                selected_port: 0,
                edit_active: false,
                edit_port: None,
                edit_field_index: 0,
                edit_field_key: None,
                edit_buffer: String::new(),
                edit_cursor_pos: 0,
            },
        }
    }

    pub fn snapshot_modbus_dashboard(&self) -> ModbusDashboardStatus {
        match &self.page {
            types::Page::ModbusDashboard {
                selected_port,
                cursor,
                editing_field,
                input_buffer,
                edit_choice_index,
                edit_confirmed,
                master_cursor,
                master_field_selected,
                master_field_editing,
                master_edit_field,
                master_edit_index,
                master_input_buffer,
                poll_round_index,
                in_flight_reg_index,
                ..
            } => ModbusDashboardStatus {
                selected_port: *selected_port,
                cursor: *cursor,
                editing_field: *editing_field,
                input_buffer: input_buffer.clone(),
                edit_choice_index: *edit_choice_index,
                edit_confirmed: *edit_confirmed,
                master_cursor: *master_cursor,
                master_field_selected: *master_field_selected,
                master_field_editing: *master_field_editing,
                master_edit_field: master_edit_field.clone(),
                master_edit_index: *master_edit_index,
                master_input_buffer: master_input_buffer.clone(),
                poll_round_index: *poll_round_index,
                in_flight_reg_index: *in_flight_reg_index,
            },
            _ => ModbusDashboardStatus {
                selected_port: 0,
                cursor: 0,
                editing_field: None,
                input_buffer: String::new(),
                edit_choice_index: None,
                edit_confirmed: false,
                master_cursor: 0,
                master_field_selected: false,
                master_field_editing: false,
                master_edit_field: None,
                master_edit_index: None,
                master_input_buffer: String::new(),
                poll_round_index: 0,
                in_flight_reg_index: None,
            },
        }
    }

    pub fn snapshot_modbus_log(&self) -> ModbusLogStatus {
        match &self.page {
            types::Page::ModbusLog { selected_port, .. } => ModbusLogStatus {
                selected_port: *selected_port,
            },
            _ => ModbusLogStatus { selected_port: 0 },
        }
    }

    pub fn snapshot_about(&self) -> AboutStatus {
        match &self.page {
            types::Page::About { view_offset } => AboutStatus {
                view_offset: *view_offset,
            },
            _ => AboutStatus { view_offset: 0 },
        }
    }
}
use yuuka::derive_struct;

derive_struct! {
    pub Status {
        ports: {
            order: Vec<String> = vec![],
            map: HashMap<String, crate::protocol::status::types::port::PortData> = HashMap::new(),
        },

        page: enum Page {
            Entry {
                cursor?: crate::protocol::status::types::ui::EntryCursor,
            },
            ModbusConfig {
                selected_port: usize,

                edit_active: bool = false,
                edit_port?: String,
                edit_field_index: usize = 0,
                edit_field_key?: String,
                edit_buffer: String = String::new(),
                edit_cursor_pos: usize = 0,
            },
            ModbusDashboard {
                selected_port: usize,

                cursor: usize,
                editing_field?: crate::protocol::status::types::modbus::EditingField,
                input_buffer: String,
                edit_choice_index: Option<usize>,
                edit_confirmed: bool,

                master_cursor: usize,
                master_field_selected: bool,
                master_field_editing: bool,
                master_edit_field?: crate::protocol::status::types::modbus::MasterEditField,
                master_edit_index: Option<usize>,
                master_input_buffer: String,
                poll_round_index: usize,
                in_flight_reg_index: Option<usize>,
            },
            ModbusLog {
                selected_port: usize,
            },
            About {
                view_offset: usize,
            }
        } = Entry { cursor: None },

        temporarily: {
            // Short-lived UI state. Only place truly transient values here.
            input_raw_buffer: String,
            input_mode: crate::protocol::status::types::ui::InputMode = crate::protocol::status::types::ui::InputMode::Ascii,

            // Scan results (transient)
            scan: {
                last_scan_time?: DateTime<Local>,
                last_scan_info: String = String::new(),
            },

            // Busy indicator for global spinner
            busy: {
                busy: bool = false,
                spinner_frame: u32 = 0,
            },

            // Per-port transient state
            per_port: {
                pending_sync_port?: String,
            },

            // Modal transient UI substructure
            modals: {
                mode_selector: {
                    active: bool = false,
                    selector: crate::protocol::status::types::ui::AppMode = crate::protocol::status::types::ui::AppMode::Modbus,
                },
            },

            // Global transient error storage (moved from page.error)
            error?: ErrorInfo {
                message: String,
                timestamp: DateTime<Local>,
            },
            // Config panel persistent-for-session editing state
            config_edit: {
                /// Whether the config panel is currently in edit mode.
                active: bool = false,
                /// Port name being edited (if any)
                port?: String,
                /// Index of the selected field in the KV list
                field_index: usize = 0,
                /// Optional canonical field key/name for the field being edited
                field_key?: String,
                /// Input buffer containing the in-progress edit value
                buffer: String = String::new(),
                /// Cursor position inside the buffer
                cursor_pos: usize = 0,
            },
        }
    }
}
