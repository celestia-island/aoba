//! Configuration persistence module
//! 
//! Provides functionality to save and load port configurations to/from disk.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::protocol::status::types::modbus::ModbusRegisterItem;
use crate::protocol::status::types::port::PortConfig;

/// Represents a persisted port configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPortConfig {
    pub name: String,
    pub config: SerializablePortConfig,
}

/// Serializable version of PortConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializablePortConfig {
    Modbus {
        mode: String, // "Master" or "Slave"
        stations: Vec<SerializableStation>,
    },
}

/// Serializable version of ModbusRegisterItem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableStation {
    pub station_id: u8,
    pub register_mode: String, // "Holding", "Input", "Coils", "Discrete"
    pub register_address: u16,
    pub register_length: u16,
    pub last_values: Vec<u16>,
}

/// Get the path to the configuration file
fn get_config_path() -> Result<PathBuf> {
    // Use /tmp for test environments, or ~/.config/aoba for production
    let config_dir = if std::env::var("TUI_E2E_TEST").is_ok() {
        PathBuf::from("/tmp/aoba_test_config")
    } else {
        dirs::config_dir()
            .context("Failed to get config directory")?
            .join("aoba")
    };

    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
    Ok(config_dir.join("ports.json"))
}

/// Save port configurations to disk
pub fn save_port_configs(configs: &HashMap<String, PortConfig>) -> Result<()> {
    let path = get_config_path()?;
    
    let persisted: Vec<PersistedPortConfig> = configs
        .iter()
        .map(|(name, config)| {
            let serializable_config = match config {
                PortConfig::Modbus { mode, stations } => {
                    let mode_str = if mode.is_master() { "Master" } else { "Slave" };
                    let serializable_stations = stations
                        .iter()
                        .map(|station| SerializableStation {
                            station_id: station.station_id,
                            register_mode: format!("{:?}", station.register_mode),
                            register_address: station.register_address,
                            register_length: station.register_length,
                            last_values: station.last_values.clone(),
                        })
                        .collect();
                    
                    SerializablePortConfig::Modbus {
                        mode: mode_str.to_string(),
                        stations: serializable_stations,
                    }
                }
            };
            
            PersistedPortConfig {
                name: name.clone(),
                config: serializable_config,
            }
        })
        .collect();
    
    let json = serde_json::to_string_pretty(&persisted)
        .context("Failed to serialize port configs")?;
    
    fs::write(&path, json)
        .with_context(|| format!("Failed to write config to {:?}", path))?;
    
    log::debug!("💾 Saved {} port configurations to {:?}", configs.len(), path);
    Ok(())
}

/// Load port configurations from disk
pub fn load_port_configs() -> Result<HashMap<String, PortConfig>> {
    let path = get_config_path()?;
    
    if !path.exists() {
        log::debug!("📂 No saved config found at {:?}", path);
        return Ok(HashMap::new());
    }
    
    let json = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {:?}", path))?;
    
    let persisted: Vec<PersistedPortConfig> = serde_json::from_str(&json)
        .context("Failed to deserialize port configs")?;
    
    let mut configs = HashMap::new();
    
    for p in persisted {
        let config = match p.config {
            SerializablePortConfig::Modbus { mode, stations } => {
                use crate::protocol::status::types::modbus::{ModbusConnectionMode, RegisterMode};
                
                let mode_enum = if mode == "Master" {
                    ModbusConnectionMode::default_master()
                } else {
                    ModbusConnectionMode::default_slave()
                };
                
                let register_items: Vec<ModbusRegisterItem> = stations
                    .iter()
                    .map(|s| {
                        let register_mode = match s.register_mode.as_str() {
                            "Holding" => RegisterMode::Holding,
                            "Input" => RegisterMode::Input,
                            "Coils" => RegisterMode::Coils,
                            "DiscreteInputs" => RegisterMode::DiscreteInputs,
                            _ => RegisterMode::Holding, // Default fallback
                        };
                        
                        ModbusRegisterItem {
                            station_id: s.station_id,
                            register_mode,
                            register_address: s.register_address,
                            register_length: s.register_length,
                            last_values: s.last_values.clone(),
                            req_success: 0,
                            req_total: 0,
                            next_poll_at: std::time::Instant::now(),
                            last_request_time: None,
                            last_response_time: None,
                            pending_requests: Vec::new(),
                        }
                    })
                    .collect();
                
                PortConfig::Modbus {
                    mode: mode_enum,
                    stations: register_items,
                }
            }
        };
        
        configs.insert(p.name, config);
    }
    
    log::info!("📂 Loaded {} port configurations from {:?}", configs.len(), path);
    Ok(configs)
}
