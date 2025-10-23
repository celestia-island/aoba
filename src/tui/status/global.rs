/// TUI global status structure
///
/// This module defines the global status structure used by the TUI application.
/// It is separate from the serializable E2E test status structures.
use std::collections::HashMap;
use yuuka::derive_struct;

use crate::protocol::status::types::port;

derive_struct! {
    pub Status {
        ports: {
            order: Vec<String> = vec![],
            map: HashMap<String, port::PortData> = HashMap::new(),
        },

        page: enum Page {
            Entry {
                cursor?: crate::protocol::status::types::cursor::EntryCursor,
                view_offset: usize = 0,
            },
            ConfigPanel {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::protocol::status::types::cursor::ConfigPanelCursor = crate::protocol::status::types::cursor::ConfigPanelCursor::EnablePort,
            },
            ModbusDashboard {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::protocol::status::types::cursor::ModbusDashboardCursor = crate::protocol::status::types::cursor::ModbusDashboardCursor::AddLine,
            },
            LogPanel {
                selected_port: usize,
                input_mode: crate::protocol::status::types::ui::InputMode = crate::protocol::status::types::ui::InputMode::Ascii,
                selected_item: Option<usize> = None,
            },
            About {
                view_offset: usize,
            }
        } = Entry { cursor: None, view_offset: 0 },

        temporarily: {
            // Short-lived UI state. Only place truly transient values here.
            input_raw_buffer: crate::protocol::status::types::ui::InputRawBuffer = crate::protocol::status::types::ui::InputRawBuffer::None,

            // Scan results (transient)
            scan: {
                last_scan_time?: chrono::DateTime<chrono::Local>,
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
                timestamp: chrono::DateTime<chrono::Local>,
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
