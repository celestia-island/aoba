/// TUI status module
///
/// This module provides the TUI-specific status tree and read/write helpers,
/// along with the serializable snapshot structures used by E2E tooling.
pub mod types;

use anyhow::Result;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use yuuka::derive_struct;

use crate::tui::status::types::port;

/// Serializable snapshot helpers for E2E tooling.
pub mod serializable {
    /// TUI-specific status structure for E2E testing
    ///
    /// This module defines a serializable status structure specifically for TUI,
    /// which can be easily converted to JSON for E2E test validation.
    use anyhow::{anyhow, Result};
    use serde::{Deserialize, Serialize};

    pub use crate::tui::status::types::port::PortState;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TuiStatus {
        pub ports: Vec<TuiPort>,
        pub page: TuiPage,
        pub timestamp: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cursor: Option<PageCursor>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub temporaries: Option<TemporariesSnapshot>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TuiPort {
        pub name: String,
        pub enabled: bool,
        pub state: PortState,
        pub modbus_masters: Vec<TuiModbusMaster>,
        pub modbus_slaves: Vec<TuiModbusSlave>,
        pub log_count: usize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum TuiPage {
        Entry,
        ConfigPanel,
        ModbusDashboard,
        LogPanel,
        About,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TuiModbusMaster {
        pub station_id: u8,
        pub register_type: String,
        pub start_address: u16,
        pub register_count: usize,
        #[serde(default)]
        pub registers: Vec<u16>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TuiModbusSlave {
        pub station_id: u8,
        pub register_type: String,
        pub start_address: u16,
        pub register_count: usize,
        #[serde(default)]
        pub registers: Vec<u16>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum PageCursor {
        Entry {
            position: String,
        },
        ConfigPanel {
            cursor: String,
            view_offset: usize,
        },
        ModbusDashboard {
            cursor: String,
            view_offset: usize,
        },
        LogPanel {
            selected_item: Option<usize>,
        },
        About {
            #[serde(skip_serializing_if = "Option::is_none")]
            view_offset: Option<usize>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TemporariesSnapshot {
        pub config_edit: ConfigEditSnapshot,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ConfigEditSnapshot {
        pub active: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub port: Option<String>,
        pub field_index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub field_key: Option<String>,
        pub buffer: String,
        pub cursor_pos: usize,
    }

    /// Convert from global Status to TuiStatus for serialization
    impl TuiStatus {
        pub fn from_status(status: &super::Status) -> Self {
            use crate::tui::status::types::port::PortConfig;

            let mut ports = Vec::new();

            for port_name in &status.ports.order {
                if let Some(port) = status.ports.map.get(port_name) {
                    let enabled = matches!(port.state, PortState::OccupiedByThis);
                    let state = port.state.clone();

                    let mut modbus_masters = Vec::new();
                    let mut modbus_slaves = Vec::new();

                    let PortConfig::Modbus { mode, stations } = &port.config;
                    for station in stations {
                        if mode.is_master() {
                            modbus_masters.push(TuiModbusMaster {
                                station_id: station.station_id,
                                register_type: format!("{:?}", station.register_mode),
                                start_address: station.register_address,
                                register_count: station.register_length as usize,
                                registers: Vec::new(), // Empty for real TUI, filled by E2E tests
                            });
                        } else {
                            modbus_slaves.push(TuiModbusSlave {
                                station_id: station.station_id,
                                register_type: format!("{:?}", station.register_mode),
                                start_address: station.register_address,
                                register_count: station.register_length as usize,
                                registers: Vec::new(), // Empty for real TUI, filled by E2E tests
                            });
                        }
                    }

                    ports.push(TuiPort {
                        name: port.port_name.clone(),
                        enabled,
                        state,
                        modbus_masters,
                        modbus_slaves,
                        log_count: port.logs.len(),
                    });
                }
            }

            let cursor = match &status.page {
                super::Page::Entry { cursor, .. } => {
                    cursor.as_ref().map(|entry_cursor| PageCursor::Entry {
                        position: format!("{:?}", entry_cursor),
                    })
                }
                super::Page::ConfigPanel {
                    cursor,
                    view_offset,
                    ..
                } => Some(PageCursor::ConfigPanel {
                    cursor: format!("{:?}", cursor),
                    view_offset: *view_offset,
                }),
                super::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } => Some(PageCursor::ModbusDashboard {
                    cursor: format!("{:?}", cursor),
                    view_offset: *view_offset,
                }),
                super::Page::LogPanel { selected_item, .. } => Some(PageCursor::LogPanel {
                    selected_item: *selected_item,
                }),
                super::Page::About { view_offset } => Some(PageCursor::About {
                    view_offset: Some(*view_offset),
                }),
            };

            let temporaries = TemporariesSnapshot {
                config_edit: ConfigEditSnapshot {
                    active: status.temporarily.config_edit.active,
                    port: status.temporarily.config_edit.port.clone(),
                    field_index: status.temporarily.config_edit.field_index,
                    field_key: status.temporarily.config_edit.field_key.clone(),
                    buffer: status.temporarily.config_edit.buffer.clone(),
                    cursor_pos: status.temporarily.config_edit.cursor_pos,
                },
            };

            let page = match &status.page {
                super::Page::Entry { .. } => TuiPage::Entry,
                super::Page::ConfigPanel { .. } => TuiPage::ConfigPanel,
                super::Page::ModbusDashboard { .. } => TuiPage::ModbusDashboard,
                super::Page::LogPanel { .. } => TuiPage::LogPanel,
                super::Page::About { .. } => TuiPage::About,
            };

            TuiStatus {
                ports,
                page,
                timestamp: chrono::Local::now().to_rfc3339(),
                cursor,
                temporaries: Some(temporaries),
            }
        }

        pub fn from_global_status() -> Result<Self> {
            super::read_status(|status| Ok(Self::from_status(status)))
        }

        pub fn to_json(&self) -> Result<String> {
            serde_json::to_string_pretty(self)
                .map_err(|err| anyhow!("Failed to serialize TUI status: {err}"))
        }
    }
}

pub use serializable::TuiStatus;

derive_struct! {
    pub Status {
        ports: {
            order: Vec<String> = vec![],
            map: HashMap<String, port::PortData> = HashMap::new(),
        },

        page: enum Page {
            Entry {
                cursor?: crate::tui::status::types::cursor::EntryCursor,
                view_offset: usize = 0,
            },
            ConfigPanel {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::tui::status::types::cursor::ConfigPanelCursor = crate::tui::status::types::cursor::ConfigPanelCursor::EnablePort,
            },
            ModbusDashboard {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::tui::status::types::cursor::ModbusDashboardCursor = crate::tui::status::types::cursor::ModbusDashboardCursor::AddLine,
            },
            LogPanel {
                selected_port: usize,
                input_mode: crate::tui::status::types::ui::InputMode = crate::tui::status::types::ui::InputMode::Ascii,
                selected_item: Option<usize> = None,
            },
            About {
                view_offset: usize,
            }
        } = Entry { cursor: None, view_offset: 0 },

        temporarily: {
            // Short-lived UI state. Only place truly transient values here.
            input_raw_buffer: crate::tui::status::types::ui::InputRawBuffer = crate::tui::status::types::ui::InputRawBuffer::None,

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
                    selector: crate::tui::status::types::ui::AppMode = crate::tui::status::types::ui::AppMode::Modbus,
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

pub use {ErrorInfo, Page, Status};

/// Global TUI status instance
static TUI_STATUS: OnceCell<Arc<RwLock<Status>>> = OnceCell::new();

impl Status {
    /// Convert the in-memory status into a serializable snapshot. Kept async to
    /// preserve the existing call sites that await the conversion.
    pub async fn to_serializable(&self) -> serializable::TuiStatus {
        serializable::TuiStatus::from_status(self)
    }
}

/// Initialize the TUI status instance. This should be called once at application startup.
pub fn init_status(status: Arc<RwLock<Status>>) -> Result<()> {
    crate::protocol::status::init_status_generic(&TUI_STATUS, status)
}

/// TUI-specific read-only accessor for `Status`.
///
/// This is a wrapper around the generic read_status function that uses the TUI status tree.
pub fn read_status<R, F>(f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    crate::protocol::status::read_status_generic(&TUI_STATUS, f)
}

/// TUI-specific write accessor for `Status`.
///
/// This is a wrapper around the generic write_status function that uses the TUI status tree.
pub fn write_status<R, F>(f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    crate::protocol::status::write_status_generic(&TUI_STATUS, f)
}
