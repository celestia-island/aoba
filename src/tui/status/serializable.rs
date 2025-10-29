/// TUI-specific status structure for E2E testing
///
/// This module defines a serializable status structure specifically for TUI,
/// which can be easily converted to JSON for E2E test validation.
use serde::{Deserialize, Serialize};

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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
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
        use crate::protocol::status::types::port::PortState as ProtocolPortState;

        super::read_status(|status| {
            let mut ports = Vec::new();

            for port_name in &status.ports.order {
                if let Some(port) = status.ports.map.get(port_name) {
                    let enabled = matches!(port.state, ProtocolPortState::OccupiedByThis);
                    let state = match &port.state {
                        ProtocolPortState::Free => {
                            crate::tui::status::serializable::PortState::Free
                        }
                        ProtocolPortState::OccupiedByThis => {
                            crate::tui::status::serializable::PortState::OccupiedByThis
                        }
                        ProtocolPortState::OccupiedByOther => {
                            crate::tui::status::serializable::PortState::OccupiedByOther
                        }
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

            let page = match &status.page {
                super::Page::Entry { .. } => TuiPage::Entry,
                super::Page::ConfigPanel { .. } => TuiPage::ConfigPanel,
                super::Page::ModbusDashboard { .. } => TuiPage::ModbusDashboard,
                super::Page::LogPanel { .. } => TuiPage::LogPanel,
                super::Page::About { .. } => TuiPage::About,
            };

            Ok(TuiStatus {
                ports,
                page,
                timestamp: chrono::Local::now().to_rfc3339(),
            })
        })
    }
}
