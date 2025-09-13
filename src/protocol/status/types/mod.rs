pub mod modbus;
pub mod port;
pub mod ui;

use chrono::{DateTime, Local};
use std::collections::HashMap;
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
        }
    }
}
