/// TUI status module
///
/// This module provides the TUI-specific status tree and read/write helpers,
/// along with the serializable snapshot structures used by E2E tooling.
pub mod cursor;
pub mod ui;

use crate::protocol::status::{init_status_generic, read_status_generic, write_status_generic};

// Re-export the protocol status types at the `status` level so callers can use
// `crate::tui::status::modbus`, `crate::tui::status::port`, `crate::tui::status::cli`.
pub use crate::protocol::status::types::{cli, modbus, port};

use anyhow::Result;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use yuuka::derive_struct;

/// Serializable snapshot helpers for E2E tooling.
pub mod serializable {
    /// TUI-specific status structure for E2E testing
    ///
    /// This module defines a serializable status structure specifically for TUI,
    /// which can be easily converted to JSON for E2E test validation.
    use anyhow::{anyhow, Result};
    use serde::{
        de::{Deserializer, Error as _},
        Deserialize, Serialize,
    };
    use std::{
        collections::{HashMap, HashSet},
        convert::TryFrom,
        time::Instant,
    };

    use crate::tui::status::{
        cursor::{ConfigPanelCursor, Cursor, ModbusDashboardCursor},
        modbus::{ModbusConnectionMode, ModbusRegisterItem, RegisterMode},
        port::{PortConfig, PortData, PortState, PortStatusIndicator},
    };

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TuiStatus {
        #[serde(default, deserialize_with = "deserialize_ports")]
        pub ports: Vec<TuiPort>,
        #[serde(default)]
        pub port_order: Vec<String>,
        pub page: TuiPage,
        pub timestamp: String,
        #[serde(default)]
        pub page_state: PageState,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TuiPort {
        pub name: String,
        pub enabled: bool,
        pub state: PortState,
        pub modbus_masters: Vec<TuiModbusStation>,
        pub modbus_slaves: Vec<TuiModbusStation>,
        pub log_count: usize,
    }

    impl Default for TuiPort {
        fn default() -> Self {
            Self {
                name: String::new(),
                enabled: false,
                state: PortState::Free,
                modbus_masters: Vec::new(),
                modbus_slaves: Vec::new(),
                log_count: 0,
            }
        }
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
    pub struct TuiModbusStation {
        pub station_id: u8,
        pub register_type: String,
        pub start_address: u16,
        pub register_count: usize,
        #[serde(default)]
        pub registers: Vec<u16>,
    }

    pub type TuiModbusMaster = TuiModbusStation;
    pub type TuiModbusSlave = TuiModbusStation;

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    #[serde(default)]
    pub struct PageState {
        pub config_panel: Option<ConfigPanelState>,
        pub modbus_dashboard: Option<ModbusDashboardState>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ConfigPanelState {
        #[serde(
            default = "default_config_panel_cursor",
            deserialize_with = "deserialize_config_panel_cursor"
        )]
        pub cursor: ConfigPanelCursor,
        pub selected_port: usize,
        pub view_offset: Option<usize>,
    }

    impl Default for ConfigPanelState {
        fn default() -> Self {
            Self {
                cursor: ConfigPanelCursor::EnablePort,
                selected_port: 0,
                view_offset: None,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ModbusDashboardState {
        #[serde(
            default = "default_modbus_dashboard_cursor",
            deserialize_with = "deserialize_modbus_cursor"
        )]
        pub cursor: ModbusDashboardCursor,
        pub selected_port: usize,
        pub view_offset: usize,
    }

    impl Default for ModbusDashboardState {
        fn default() -> Self {
            Self {
                cursor: ModbusDashboardCursor::AddLine,
                selected_port: 0,
                view_offset: 0,
            }
        }
    }

    /// Convert from global Status to TuiStatus for serialization
    impl TuiStatus {
        pub fn from_status(status: &super::Status) -> Self {
            let mut ports = Vec::new();

            for port_name in &status.ports.order {
                if let Some(port) = status.ports.map.get(port_name) {
                    let enabled = matches!(port.state, PortState::OccupiedByThis);
                    let state = port.state.clone();

                    let mut modbus_masters = Vec::new();
                    let mut modbus_slaves = Vec::new();

                    let PortConfig::Modbus {
                        mode,
                        master_source: _,
                        stations,
                    } = &port.config;
                    for station in stations {
                        let station_snapshot = TuiModbusStation {
                            station_id: station.station_id,
                            register_type: format!("{:?}", station.register_mode),
                            start_address: station.register_address,
                            register_count: station.register_length as usize,
                            registers: station.last_values.clone(),
                        };

                        if mode.is_master() {
                            modbus_masters.push(station_snapshot);
                        } else {
                            modbus_slaves.push(station_snapshot);
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

            let page_state = match &status.page {
                super::Page::ConfigPanel {
                    selected_port,
                    view_offset,
                    cursor,
                } => PageState {
                    config_panel: Some(ConfigPanelState {
                        cursor: *cursor,
                        selected_port: *selected_port,
                        view_offset: Some(*view_offset),
                    }),
                    ..PageState::default()
                },
                super::Page::ModbusDashboard {
                    selected_port,
                    view_offset,
                    cursor,
                } => PageState {
                    modbus_dashboard: Some(ModbusDashboardState {
                        cursor: *cursor,
                        selected_port: *selected_port,
                        view_offset: *view_offset,
                    }),
                    ..PageState::default()
                },
                _ => PageState::default(),
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
                port_order: status.ports.order.clone(),
                page,
                timestamp: chrono::Local::now().to_rfc3339(),
                page_state,
            }
        }

        pub fn apply_to_status(&self, status: &mut super::Status) -> Result<()> {
            status.ports.order.clear();
            status.ports.map.clear();

            let mut ports_by_name: HashMap<&str, &TuiPort> = HashMap::new();
            for port in &self.ports {
                ports_by_name.insert(port.name.as_str(), port);
            }

            let mut processed = HashSet::new();

            for name in &self.port_order {
                if processed.contains(name) {
                    continue;
                }
                if let Some(port) = ports_by_name.get(name.as_str()) {
                    let data = convert_port(port)?;
                    status.ports.order.push(name.clone());
                    status.ports.map.insert(name.clone(), data);
                    processed.insert(name.clone());
                }
            }

            for port in &self.ports {
                if processed.contains(&port.name) {
                    continue;
                }
                let data = convert_port(port)?;
                status.ports.order.push(port.name.clone());
                status.ports.map.insert(port.name.clone(), data);
                processed.insert(port.name.clone());
            }

            status.page = resolve_page(&self.page, &self.page_state)?;
            Ok(())
        }

        pub fn from_global_status() -> Result<Self> {
            super::read_status(|status| Ok(Self::from_status(status)))
        }

        pub fn to_json(&self) -> Result<String> {
            serde_json::to_string_pretty(self)
                .map_err(|err| anyhow!("Failed to serialize TUI status: {err}"))
        }
    }

    fn convert_port(port: &TuiPort) -> Result<PortData> {
        // Determine port state: if enabled but not marked as occupied, mark as OccupiedByThis
        let mut state = port.state.clone();
        if port.enabled && !matches!(state, PortState::OccupiedByThis) {
            state = PortState::OccupiedByThis;
        }

        // Build station list
        let mut stations = Vec::new();
        if !port.modbus_slaves.is_empty() && port.modbus_masters.is_empty() {
            for station in &port.modbus_slaves {
                stations.push(convert_station(station)?);
            }
            let config = PortConfig::Modbus {
                mode: ModbusConnectionMode::default_slave(),
                master_source: Default::default(),
                stations,
            };
            let status_indicator = match &state {
                PortState::OccupiedByThis => PortStatusIndicator::Running,
                _ => PortStatusIndicator::NotStarted,
            };

            let data = PortData {
                port_name: port.name.clone(),
                state,
                status_indicator,
                config,
                ..PortData::default()
            };

            Ok(data)
        } else {
            for station in &port.modbus_masters {
                stations.push(convert_station(station)?);
            }
            let config = PortConfig::Modbus {
                mode: ModbusConnectionMode::default_master(),
                master_source: Default::default(),
                stations,
            };
            let status_indicator = match &state {
                PortState::OccupiedByThis => PortStatusIndicator::Running,
                _ => PortStatusIndicator::NotStarted,
            };

            let data = PortData {
                port_name: port.name.clone(),
                state,
                status_indicator,
                config,
                ..PortData::default()
            };

            Ok(data)
        }
    }

    fn convert_station(station: &TuiModbusStation) -> Result<ModbusRegisterItem> {
        let register_mode = RegisterMode::try_from(station.register_type.as_str())
            .map_err(|_| anyhow!("Unsupported register type: {}", station.register_type))?;

        let register_length = u16::try_from(station.register_count).map_err(|_| {
            anyhow!(
                "register_count exceeds u16 range: {}",
                station.register_count
            )
        })?;

        let mut last_values = vec![0u16; station.register_count];
        for (index, value) in station.registers.iter().enumerate() {
            if index >= last_values.len() {
                break;
            }
            last_values[index] = *value;
        }

        Ok(ModbusRegisterItem {
            station_id: station.station_id,
            register_mode,
            register_address: station.start_address,
            register_length,
            last_values,
            req_success: 0,
            req_total: 0,
            next_poll_at: Instant::now(),
            last_request_time: None,
            last_response_time: None,
            pending_requests: Vec::new(),
        })
    }

    fn resolve_page(page: &TuiPage, state: &PageState) -> Result<super::Page> {
        let resolved = match page {
            TuiPage::Entry => super::Page::Entry {
                cursor: None,
                view_offset: 0,
            },
            TuiPage::ConfigPanel => {
                let config = state.config_panel.clone().unwrap_or_default();
                super::Page::ConfigPanel {
                    selected_port: config.selected_port,
                    view_offset: config
                        .view_offset
                        .unwrap_or_else(|| config.cursor.view_offset()),
                    cursor: config.cursor,
                }
            }
            TuiPage::ModbusDashboard => {
                let modbus = state.modbus_dashboard.clone().unwrap_or_default();
                super::Page::ModbusDashboard {
                    selected_port: modbus.selected_port,
                    view_offset: modbus.view_offset,
                    cursor: modbus.cursor,
                }
            }
            TuiPage::LogPanel => super::Page::LogPanel {
                selected_port: 0,
                input_mode: crate::tui::status::ui::InputMode::Ascii,
                selected_item: None,
            },
            TuiPage::About => super::Page::About { view_offset: 0 },
        };

        Ok(resolved)
    }

    fn deserialize_ports<'de, D>(deserializer: D) -> Result<Vec<TuiPort>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum PortsHelper {
            List(Vec<TuiPort>),
            Map(HashMap<String, TuiPort>),
        }

        let helper = Option::<PortsHelper>::deserialize(deserializer)?;
        Ok(match helper {
            Some(PortsHelper::List(list)) => list,
            Some(PortsHelper::Map(map)) => map
                .into_iter()
                .map(|(name, mut port)| {
                    if port.name.is_empty() {
                        port.name = name;
                    }
                    port
                })
                .collect(),
            None => Vec::new(),
        })
    }

    fn default_config_panel_cursor() -> ConfigPanelCursor {
        ConfigPanelCursor::EnablePort
    }

    fn deserialize_config_panel_cursor<'de, D>(
        deserializer: D,
    ) -> Result<ConfigPanelCursor, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ConfigCursorHelper {
            Text(String),
            Object {
                cursor: Option<String>,
                kind: Option<String>,
            },
        }

        let helper = Option::<ConfigCursorHelper>::deserialize(deserializer)?;
        let cursor = match helper {
            None => default_config_panel_cursor(),
            Some(ConfigCursorHelper::Text(text)) => {
                config_panel_cursor_from_str(&text).map_err(D::Error::custom)?
            }
            Some(ConfigCursorHelper::Object { cursor, kind }) => {
                let name = cursor.or(kind).ok_or_else(|| {
                    D::Error::custom(
                        "Config panel cursor object must include `cursor` or `kind` field",
                    )
                })?;
                config_panel_cursor_from_str(&name).map_err(D::Error::custom)?
            }
        };

        Ok(cursor)
    }

    fn config_panel_cursor_from_str(name: &str) -> Result<ConfigPanelCursor> {
        let normalized = name.replace([' ', '_'], "").to_ascii_lowercase();
        let cursor = match normalized.as_str() {
            "enableport" => ConfigPanelCursor::EnablePort,
            "protocolmode" => ConfigPanelCursor::ProtocolMode,
            "protocolconfig" | "enterbusinessconfiguration" => ConfigPanelCursor::ProtocolConfig,
            "baudrate" => ConfigPanelCursor::BaudRate,
            "databits" => ConfigPanelCursor::DataBits { custom_mode: false },
            "parity" => ConfigPanelCursor::Parity,
            "stopbits" => ConfigPanelCursor::StopBits,
            "viewcommunicationlog" | "enterlogpage" => ConfigPanelCursor::ViewCommunicationLog,
            other => {
                return Err(anyhow!("Unsupported config panel cursor value: {other}"));
            }
        };

        Ok(cursor)
    }

    fn default_modbus_dashboard_cursor() -> ModbusDashboardCursor {
        ModbusDashboardCursor::AddLine
    }

    fn deserialize_modbus_cursor<'de, D>(deserializer: D) -> Result<ModbusDashboardCursor, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Option::<ModbusCursorHelper>::deserialize(deserializer)?;
        helper
            .unwrap_or_default()
            .try_into()
            .map_err(D::Error::custom)
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum ModbusCursorHelper {
        Direct(ModbusDashboardCursor),
        Named {
            kind: String,
            #[serde(default)]
            station_index: Option<usize>,
            #[serde(default)]
            slave_index: Option<usize>,
            #[serde(default)]
            register_index: Option<usize>,
        },
    }

    impl Default for ModbusCursorHelper {
        fn default() -> Self {
            ModbusCursorHelper::Direct(ModbusDashboardCursor::AddLine)
        }
    }

    impl TryFrom<ModbusCursorHelper> for ModbusDashboardCursor {
        type Error = anyhow::Error;

        fn try_from(value: ModbusCursorHelper) -> Result<Self> {
            match value {
                ModbusCursorHelper::Direct(cursor) => Ok(cursor),
                ModbusCursorHelper::Named {
                    kind,
                    station_index,
                    slave_index,
                    register_index,
                } => {
                    let index = station_index.or(slave_index).unwrap_or(0);
                    let reg_index = register_index.unwrap_or(0);
                    match kind.to_ascii_lowercase().as_str() {
                        "addline" => Ok(ModbusDashboardCursor::AddLine),
                        "modbusmode" => Ok(ModbusDashboardCursor::ModbusMode),
                        "stationid" => Ok(ModbusDashboardCursor::StationId { index }),
                        "registermode" => Ok(ModbusDashboardCursor::RegisterMode { index }),
                        "registerstartaddress" => {
                            Ok(ModbusDashboardCursor::RegisterStartAddress { index })
                        }
                        "registerlength" => Ok(ModbusDashboardCursor::RegisterLength { index }),
                        "register" => Ok(ModbusDashboardCursor::Register {
                            slave_index: index,
                            register_index: reg_index,
                        }),
                        other => Err(anyhow!(
                            "Unsupported Modbus dashboard cursor value: {other}"
                        )),
                    }
                }
            }
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
                cursor?: crate::tui::status::cursor::EntryCursor,
                view_offset: usize = 0,
            },
            ConfigPanel {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::tui::status::cursor::ConfigPanelCursor = crate::tui::status::cursor::ConfigPanelCursor::EnablePort,
            },
            ModbusDashboard {
                selected_port: usize,
                view_offset: usize = 0,
                cursor: crate::tui::status::cursor::ModbusDashboardCursor = crate::tui::status::cursor::ModbusDashboardCursor::AddLine,
            },
            LogPanel {
                selected_port: usize,
                input_mode: crate::tui::status::ui::InputMode = crate::tui::status::ui::InputMode::Ascii,
                selected_item: Option<usize> = None,
            },
            About {
                view_offset: usize,
            }
        } = Entry { cursor: None, view_offset: 0 },

        temporarily: {
            // Short-lived UI state. Only place truly transient values here.
            input_raw_buffer: crate::tui::status::ui::InputRawBuffer = crate::tui::status::ui::InputRawBuffer::None,

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
                    selector: crate::tui::status::ui::AppMode = crate::tui::status::ui::AppMode::Modbus,
                },
            },

            // Global transient error storage (moved from page.error)
            error?: ErrorInfo {
                message: String,
                timestamp: chrono::DateTime<chrono::Local>,
            },
            // Last dismissed error metadata (used to debounce re-display)
            dismissed_error_message?: String,
            dismissed_error_timestamp?: chrono::DateTime<chrono::Local>,
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
    init_status_generic(&TUI_STATUS, status)
}

/// TUI-specific read-only accessor for `Status`.
///
/// This is a wrapper around the generic read_status function that uses the TUI status tree.
pub fn read_status<R, F>(f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    read_status_generic(&TUI_STATUS, f)
}

/// TUI-specific write accessor for `Status`.
///
/// This is a wrapper around the generic write_status function that uses the TUI status tree.
pub fn write_status<R, F>(f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    write_status_generic(&TUI_STATUS, f)
}
