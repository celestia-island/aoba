use serde::{Deserialize, Serialize};
use std::fmt;

pub use crate::protocol::status::types::modbus::{
    RegisterMap, RegisterRange, StationConfig, StationMode,
};

/// Communication method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMethod {
    /// IPC communication
    Ipc,
    /// Standard input/output communication
    Stdio,
    /// File-based communication
    File,
}

/// Persistence mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceMode {
    /// Persistent run
    Persistent,
    /// Temporary run
    Temporary,
    /// One-shot run
    OneShot,
}

/// Register type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// Coil registers (read/write bits)
    Coils,
    /// Discrete input registers (read-only bits)
    DiscreteInputs,
    /// Holding registers (read/write words)
    Holding,
    /// Input registers (read-only words)
    Input,
}

impl fmt::Display for RegisterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterType::Coils => write!(f, "coils"),
            RegisterType::DiscreteInputs => write!(f, "discrete_inputs"),
            RegisterType::Holding => write!(f, "holding"),
            RegisterType::Input => write!(f, "input"),
        }
    }
}

/// Communication thread parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationParams {
    /// Communication method
    pub mode: CommunicationMethod,
    /// Dynamically pull data from external source
    pub dynamic_pull: bool,
    /// Wait time (seconds) - deprecated, use request_interval_ms instead
    #[serde(default)]
    pub wait_time: Option<f64>,
    /// Timeout (seconds) - deprecated, use timeout_ms instead
    #[serde(default)]
    pub timeout: Option<f64>,
    /// Request interval time in milliseconds (replaces wait_time)
    #[serde(default = "default_request_interval_ms")]
    pub request_interval_ms: u32,
    /// Timeout waiting time in milliseconds (replaces timeout)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u32,
    /// Persistence mode
    pub persistence: PersistenceMode,
}

fn default_request_interval_ms() -> u32 {
    1000
}

fn default_timeout_ms() -> u32 {
    3000
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusBootConfig {
    /// Port name
    pub port_name: String,
    /// Port communication configuration (baud rate)
    pub baud_rate: u32,
    /// Communication thread parameters
    pub communication_params: CommunicationParams,
    /// List of station configurations
    pub stations: Vec<StationConfig>,
}

impl Default for CommunicationParams {
    fn default() -> Self {
        Self {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            request_interval_ms: 1000,
            timeout_ms: 3000,
            persistence: PersistenceMode::Persistent,
        }
    }
}

impl ModbusBootConfig {
    /// Parse configuration from a JSON string
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Read configuration from a file
    pub fn from_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(file_path)?;
        Self::from_json(&content).map_err(|e| e.into())
    }

    /// Convert to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = ModbusBootConfig {
            port_name: "COM1".to_string(),
            baud_rate: 9600,
            communication_params: CommunicationParams::default(),
            stations: vec![
                StationConfig {
                    station_id: 1,
                    mode: StationMode::Master,
                    map: RegisterMap {
                        holding: vec![RegisterRange {
                            address_start: 0,
                            length: 10,
                            initial_values: vec![100, 200, 300],
                        }],
                        ..Default::default()
                    },
                },
                StationConfig {
                    station_id: 2,
                    mode: StationMode::Slave,
                    map: RegisterMap {
                        input: vec![RegisterRange {
                            address_start: 100,
                            length: 5,
                            initial_values: Vec::new(),
                        }],
                        ..Default::default()
                    },
                },
            ],
        };

        let json = config.to_json().unwrap();
        println!("{json}");

        let parsed_config = ModbusBootConfig::from_json(&json).unwrap();
        assert_eq!(parsed_config.port_name, "COM1");
        assert_eq!(parsed_config.stations.len(), 2);
        assert_eq!(parsed_config.stations[0].station_id, 1);
        assert_eq!(parsed_config.stations[1].station_id, 2);

        // Test postcard serialization of stations array (what's actually used in IPC)
        // We only test that serialization works, as the actual deserialization happens
        // in the CLI subprocess with the same version of structures
        let postcard_bytes = postcard::to_allocvec(&config.stations).unwrap();
        assert!(!postcard_bytes.is_empty());
        assert!(postcard_bytes.len() > 10); // Should have some reasonable size
    }
}
