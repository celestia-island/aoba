use serde::{Deserialize, Serialize};
use std::fmt;

/// Communication mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMode {
    /// Modbus master mode
    Master,
    /// Modbus slave mode
    Slave,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// Holding registers
    Holding,
    /// Input registers
    Input,
    /// Coil registers
    Coils,
    /// Discrete input registers
    Discrete,
}

impl fmt::Display for RegisterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterType::Holding => write!(f, "holding"),
            RegisterType::Input => write!(f, "input"),
            RegisterType::Coils => write!(f, "coils"),
            RegisterType::Discrete => write!(f, "discrete"),
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
    /// Wait time (seconds)
    pub wait_time: Option<f64>,
    /// Timeout (seconds)
    pub timeout: Option<f64>,
    /// Persistence mode
    pub persistence: PersistenceMode,
}

/// Modbus register information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRegister {
    /// Station ID
    pub station_id: u8,
    /// Register type
    pub register_type: RegisterType,
    /// Start address
    pub start_address: u16,
    /// Number of registers
    pub length: u16,
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Port name
    pub port_name: String,
    /// Port communication configuration (baud rate)
    pub baud_rate: u32,
    /// Port communication mode
    pub communication_mode: CommunicationMode,
    /// Communication thread parameters
    pub communication_params: CommunicationParams,
    /// List of Modbus configurations
    pub modbus_configs: Vec<ModbusRegister>,
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

impl Default for ModbusRegister {
    fn default() -> Self {
        Self {
            station_id: 1,
            register_type: RegisterType::Holding,
            start_address: 0,
            length: 10,
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
            communication_mode: CommunicationMode::Master,
            communication_params: CommunicationParams::default(),
            modbus_configs: vec![
                ModbusRegister {
                    station_id: 1,
                    register_type: RegisterType::Holding,
                    start_address: 0,
                    length: 10,
                },
                ModbusRegister {
                    station_id: 2,
                    register_type: RegisterType::Input,
                    start_address: 100,
                    length: 5,
                },
            ],
        };

        let json = config.to_json().unwrap();
        println!("{json}");

        let parsed_config = Config::from_json(&json).unwrap();
        assert_eq!(parsed_config.port_name, "COM1");
        assert_eq!(parsed_config.modbus_configs.len(), 2);
        assert_eq!(parsed_config.modbus_configs[0].station_id, 1);
        assert_eq!(parsed_config.modbus_configs[1].station_id, 2);
    }
}
