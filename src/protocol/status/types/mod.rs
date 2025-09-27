pub mod cursor;
pub mod modbus;
pub mod port;
pub mod ui;

use chrono::{DateTime, Local};
use std::collections::HashMap;
use yuuka::derive_struct;

use ui::InputMode;

derive_struct! {
    pub Status {
        ports: {
            order: Vec<String> = vec![],
            map: HashMap<String, std::sync::Arc<std::sync::RwLock<port::PortData>>>
                = HashMap::new(),
        },

        page: enum Page {
            Entry {
                cursor?: cursor::EntryCursor,
            },
            ConfigPanel {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: cursor::ConfigPanelCursor = cursor::ConfigPanelCursor::EnablePort,
            },
            ModbusDashboard {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: cursor::ModbusDashboardCursor = cursor::ModbusDashboardCursor::AddLine,
            },
            LogPanel {
                selected_port: usize,
                input_mode: InputMode = InputMode::Ascii,
                view_offset: usize = 0,
                selected_item: Option<usize> = None,
            },
            About {
                view_offset: usize,
            }
        } = Entry { cursor: None },

        temporarily: {
            // Short-lived UI state. Only place truly transient values here.
            input_raw_buffer: ui::InputRawBuffer = ui::InputRawBuffer::None,

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
                    selector: ui::AppMode = ui::AppMode::Modbus,
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
