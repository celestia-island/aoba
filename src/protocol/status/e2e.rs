/// E2E testing serializable status structures
///
/// This module contains the canonical serializable status structures used for
/// E2E testing. These types are used both for:
/// 1. Dumping status from TUI/CLI processes (src/tui/mod.rs, src/cli/modbus/*.rs)
/// 2. Reading status in E2E tests (examples/ci_utils/src/status_monitor.rs)
///
/// Having a single source of truth ensures consistency between dumping and parsing.
use serde::{Deserialize, Serialize};

// ============================================================================
// TUI Status Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiStatus {
    pub ports: Vec<TuiPort>,
    pub page: TuiPage,
    pub timestamp: String,
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
#[serde(tag = "type")]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiModbusSlave {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

/// Convert from global Status to TuiStatus for serialization
impl TuiStatus {
    pub fn from_global_status() -> anyhow::Result<Self> {
        use crate::protocol::status::{
            read_status,
            types::{port::PortState as GlobalPortState, Page},
            with_port_read,
        };

        read_status(|status| {
            let mut ports = Vec::new();

            for port_name in &status.ports.order {
                if let Some(port_arc) = status.ports.map.get(port_name) {
                    if let Some(Ok(port_data)) = with_port_read(port_arc, |port| {
                        let enabled = matches!(port.state, GlobalPortState::OccupiedByThis { .. });
                        let state = match &port.state {
                            GlobalPortState::Free => PortState::Free,
                            GlobalPortState::OccupiedByThis { .. } => PortState::OccupiedByThis,
                            GlobalPortState::OccupiedByOther => PortState::OccupiedByOther,
                        };

                        let mut modbus_masters = Vec::new();
                        let mut modbus_slaves = Vec::new();

                        use crate::protocol::status::types::port::PortConfig;
                        let PortConfig::Modbus { mode, stations } = &port.config;
                        for station in stations {
                            if mode.is_master() {
                                modbus_masters.push(TuiModbusMaster {
                                    station_id: station.station_id,
                                    register_type: format!("{:?}", station.register_mode),
                                    start_address: station.register_address,
                                    register_count: station.register_length as usize,
                                });
                            } else {
                                modbus_slaves.push(TuiModbusSlave {
                                    station_id: station.station_id,
                                    register_type: format!("{:?}", station.register_mode),
                                    start_address: station.register_address,
                                    register_count: station.register_length as usize,
                                });
                            }
                        }

                        Ok::<_, anyhow::Error>(TuiPort {
                            name: port.port_name.clone(),
                            enabled,
                            state,
                            modbus_masters,
                            modbus_slaves,
                            log_count: port.logs.len(),
                        })
                    }) {
                        ports.push(port_data);
                    }
                }
            }

            let page = match &status.page {
                Page::Entry { .. } => TuiPage::Entry,
                Page::ConfigPanel { .. } => TuiPage::ConfigPanel,
                Page::ModbusDashboard { .. } => TuiPage::ModbusDashboard,
                Page::LogPanel { .. } => TuiPage::LogPanel,
                Page::About { .. } => TuiPage::About,
            };

            Ok(TuiStatus {
                ports,
                page,
                timestamp: chrono::Local::now().to_rfc3339(),
            })
        })
    }
}

// ============================================================================
// CLI Status Structures
// ============================================================================

/// CLI subprocess status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub port_name: String,
    pub station_id: u8,
    pub register_mode: RegisterMode,
    pub register_address: u16,
    pub register_length: u16,
    pub mode: CliMode,
    pub timestamp: String,
}

/// CLI operation mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliMode {
    SlaveListen,
    SlavePoll,
    MasterProvide,
}

/// Register mode for modbus operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RegisterMode {
    Coil,
    Discrete,
    Input,
    Holding,
}

impl CliStatus {
    /// Create a new CLI status for slave listen mode
    pub fn new_slave_listen(
        port_name: String,
        station_id: u8,
        register_mode: crate::protocol::status::types::modbus::RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        let reg_mode = match register_mode {
            crate::protocol::status::types::modbus::RegisterMode::Coils => RegisterMode::Coil,
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                RegisterMode::Discrete
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => RegisterMode::Input,
            crate::protocol::status::types::modbus::RegisterMode::Holding => RegisterMode::Holding,
        };

        Self {
            port_name,
            station_id,
            register_mode: reg_mode,
            register_address,
            register_length,
            mode: CliMode::SlaveListen,
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Create a new CLI status for slave poll mode
    pub fn new_slave_poll(
        port_name: String,
        station_id: u8,
        register_mode: crate::protocol::status::types::modbus::RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        let reg_mode = match register_mode {
            crate::protocol::status::types::modbus::RegisterMode::Coils => RegisterMode::Coil,
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                RegisterMode::Discrete
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => RegisterMode::Input,
            crate::protocol::status::types::modbus::RegisterMode::Holding => RegisterMode::Holding,
        };

        Self {
            port_name,
            station_id,
            register_mode: reg_mode,
            register_address,
            register_length,
            mode: CliMode::SlavePoll,
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Create a new CLI status for master provide mode
    pub fn new_master_provide(
        port_name: String,
        station_id: u8,
        register_mode: crate::protocol::status::types::modbus::RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        let reg_mode = match register_mode {
            crate::protocol::status::types::modbus::RegisterMode::Coils => RegisterMode::Coil,
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                RegisterMode::Discrete
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => RegisterMode::Input,
            crate::protocol::status::types::modbus::RegisterMode::Holding => RegisterMode::Holding,
        };

        Self {
            port_name,
            station_id,
            register_mode: reg_mode,
            register_address,
            register_length,
            mode: CliMode::MasterProvide,
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> anyhow::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize CLI status: {}", e))
    }
}
