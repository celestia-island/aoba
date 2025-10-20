/// TUI-specific status structure for E2E testing
///
/// This module defines a serializable status structure specifically for TUI,
/// which can be easily converted to JSON for E2E test validation.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TuiStatus {
    pub ports: Vec<TuiPort>,
    pub page: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuiPort {
    pub name: String,
    pub enabled: bool,
    pub state: String,
    pub modbus_masters: Vec<TuiModbusMaster>,
    pub modbus_slaves: Vec<TuiModbusSlave>,
    pub log_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuiModbusMaster {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: usize,
}

#[derive(Debug, Clone, Serialize)]
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
            types::{port::PortState, Page},
            with_port_read,
        };

        read_status(|status| {
            let mut ports = Vec::new();

            for port_name in &status.ports.order {
                if let Some(port_arc) = status.ports.map.get(port_name) {
                    if let Some(Ok(port_data)) = with_port_read(port_arc, |port| {
                        let enabled = matches!(port.state, PortState::OccupiedByThis { .. });
                        let state = match &port.state {
                            PortState::Free => "Free".to_string(),
                            PortState::OccupiedByThis { .. } => "OccupiedByThis".to_string(),
                            PortState::OccupiedByOther => "OccupiedByOther".to_string(),
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
                Page::Entry { .. } => "Entry",
                Page::ConfigPanel { .. } => "ConfigPanel",
                Page::ModbusDashboard { .. } => "ModbusDashboard",
                Page::LogPanel { .. } => "LogPanel",
                Page::About { .. } => "About",
            };

            Ok(TuiStatus {
                ports,
                page: page.to_string(),
                timestamp: chrono::Local::now().to_rfc3339(),
            })
        })
    }
}
