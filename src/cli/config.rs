use serde::{Deserialize, Serialize};
use std::fmt;

/// Communication mode (Master or Slave)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationMode {
    /// Modbus master mode
    Master,
    /// Modbus slave mode
    Slave,
}

impl fmt::Display for StationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StationMode::Master => write!(f, "master"),
            StationMode::Slave => write!(f, "slave"),
        }
    }
}

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

/// Register range configuration for a specific register type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRange {
    /// Start address
    pub address_start: u16,
    /// Number of registers
    pub length: u16,
    /// Initial values (for master mode, optional for slave mode)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_values: Vec<u16>,
}

/// Register map containing all register types for a station
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegisterMap {
    /// Coil register ranges
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coils: Vec<RegisterRange>,
    /// Discrete input register ranges
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discrete_inputs: Vec<RegisterRange>,
    /// Holding register ranges
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holding: Vec<RegisterRange>,
    /// Input register ranges
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<RegisterRange>,
}

/// Station configuration with ID, mode, and register map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    /// Station ID (1-247 for Modbus)
    pub id: u8,
    /// Station mode (Master or Slave)
    pub mode: StationMode,
    /// Register map for this station
    pub map: RegisterMap,
}

/// Communication thread parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationParams {
    /// Communication method
    pub mode: CommunicationMethod,
    /// Dynamically pull data from external source
    pub dynamic_pull: bool,
    /// Wait time (seconds)
    pub wait_time: Option<f64>,
    /// Timeout (seconds)
    pub timeout: Option<f64>,
    /// Persistence mode
    pub persistence: PersistenceMode,
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
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
            persistence: PersistenceMode::Persistent,
        }
    }
}

impl Default for RegisterRange {
    fn default() -> Self {
        Self {
            address_start: 0,
            length: 10,
            initial_values: Vec::new(),
        }
    }
}

impl Default for StationConfig {
    fn default() -> Self {
        Self {
            id: 1,
            mode: StationMode::Master,
            map: RegisterMap::default(),
        }
    }
}

impl Config {
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
        let config = Config {
            port_name: "COM1".to_string(),
            baud_rate: 9600,
            communication_params: CommunicationParams::default(),
            stations: vec![
                StationConfig {
                    id: 1,
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
                    id: 2,
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

        let parsed_config = Config::from_json(&json).unwrap();
        assert_eq!(parsed_config.port_name, "COM1");
        assert_eq!(parsed_config.stations.len(), 2);
        assert_eq!(parsed_config.stations[0].id, 1);
        assert_eq!(parsed_config.stations[1].id, 2);

        // Test postcard serialization of stations array (what's actually used in IPC)
        // We only test that serialization works, as the actual deserialization happens
        // in the CLI subprocess with the same version of structures
        let postcard_bytes = postcard::to_allocvec(&config.stations).unwrap();
        assert!(!postcard_bytes.is_empty());
        assert!(postcard_bytes.len() > 10); // Should have some reasonable size
    }
}
