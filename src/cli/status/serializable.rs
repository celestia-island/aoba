/// CLI-specific status structure for E2E testing
///
/// This module defines a serializable status structure specifically for CLI subprocesses,
/// which can be easily converted to JSON for E2E test validation.
use serde::{Deserialize, Serialize};

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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliMode {
    SlaveListen,
    SlavePoll,
    MasterProvide,
}

/// Register mode for modbus operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
            .map_err(|e| anyhow::anyhow!("Failed to serialize CLI status: {e}"))
    }
}
