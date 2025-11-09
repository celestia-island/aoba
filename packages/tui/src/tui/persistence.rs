//! Configuration persistence module for TUI
//!
//! Provides functionality to save and load port configurations to/from disk.
//!
//! **IMPORTANT**: This module is designed EXCLUSIVELY for TUI use. CLI processes
//! should NOT use this persistence layer to avoid communication conflicts and race
//! conditions. The configuration file is stored in the working directory as
//! `aoba_tui_config.json` to ensure cross-platform compatibility.
//!
//! ## --no-config-cache flag
//!
//! When TUI is started with `--no-config-cache`, all save/load operations are
//! skipped. This is useful for E2E tests to ensure clean state without cache
//! interference. Call `set_no_cache(true)` early in TUI startup to enable this.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::tui::status::{modbus::ModbusRegisterItem, port::PortConfig};

/// Global flag to disable config cache (set via --no-config-cache)
static NO_CONFIG_CACHE: AtomicBool = AtomicBool::new(false);

/// Set the no-cache flag (should be called early in TUI startup)
///
/// # Parameters
/// - `enabled`: true to disable cache, false to enable cache
///
/// # Example
/// ```rust,ignore
/// // In TUI startup (src/tui/mod.rs):
/// let no_cache = matches.get_flag("no-config-cache");
/// persistence::set_no_cache(no_cache);
/// ```
pub fn set_no_cache(enabled: bool) {
    NO_CONFIG_CACHE.store(enabled, Ordering::SeqCst);
    if enabled {
        log::info!("🚫 Config cache disabled (--no-config-cache)");
    }
}

/// Get the current no-cache flag value
fn is_no_cache() -> bool {
    NO_CONFIG_CACHE.load(Ordering::SeqCst)
}

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
///
/// This configuration file is **TUI-only** and should NOT be used by CLI processes
/// to avoid communication conflicts. The file is stored in the working directory
/// for cross-platform compatibility.
fn get_config_path() -> Result<PathBuf> {
    // Get the current working directory (where the program binary is located)
    let config_dir = std::env::current_dir().context("Failed to get current working directory")?;

    // Store config in working directory for cross-platform compatibility
    // File name includes "tui" prefix to clearly indicate it's TUI-only
    Ok(config_dir.join("aoba_tui_config.json"))
}

/// Save port configurations to disk (TUI-only)
///
/// This function saves TUI port configurations to the working directory.
/// CLI processes should NOT call this function.
///
/// Save is automatically skipped if `--no-config-cache` flag is set.
///
/// # Returns
/// - `Ok(())` if save succeeded or was skipped
/// - `Err` if save failed
pub fn save_port_configs(configs: &HashMap<String, PortConfig>) -> Result<()> {
    if is_no_cache() {
        log::debug!("⏭️  Skipping config save (--no-config-cache enabled)");
        return Ok(());
    }

    let path = get_config_path()?;

    let persisted: Vec<PersistedPortConfig> = configs
        .iter()
        .map(|(name, config)| {
            let serializable_config = match config {
                PortConfig::Modbus { mode, stations } => {
                    let mode_str = if mode.is_master() { "Master" } else { "Slave" };
                    let persist_values = mode.is_master();
                    let serializable_stations = stations
                        .iter()
                        .map(|station| SerializableStation {
                            station_id: station.station_id,
                            register_mode: format!("{:?}", station.register_mode),
                            register_address: station.register_address,
                            register_length: station.register_length,
                            last_values: if persist_values {
                                station.last_values.clone()
                            } else {
                                Vec::new()
                            },
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

    let json =
        serde_json::to_string_pretty(&persisted).context("Failed to serialize port configs")?;

    fs::write(&path, json).with_context(|| format!("Failed to write config to {path:?}"))?;

    log::debug!(
        "💾 Saved {} port configurations to {:?}",
        configs.len(),
        path
    );
    Ok(())
}

/// Load port configurations from disk (TUI-only)
///
/// This function loads TUI port configurations from the working directory.
/// CLI processes should NOT call this function.
///
/// Load is automatically skipped if `--no-config-cache` flag is set.
///
/// # Returns
/// - `Ok(HashMap)` with loaded configs, or empty HashMap if skipped/not found
/// - `Err` if load failed
pub fn load_port_configs() -> Result<HashMap<String, PortConfig>> {
    if is_no_cache() {
        log::debug!("⏭️  Skipping config load (--no-config-cache enabled)");
        return Ok(HashMap::new());
    }

    let path = get_config_path()?;

    if !path.exists() {
        log::debug!("📂 No saved config found at {path:?}");
        return Ok(HashMap::new());
    }

    let json = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {path:?}"))?;

    let persisted: Vec<PersistedPortConfig> =
        serde_json::from_str(&json).context("Failed to deserialize port configs")?;

    let mut configs = HashMap::new();

    for p in persisted {
        let config = match p.config {
            SerializablePortConfig::Modbus { mode, stations } => {
                use crate::tui::status::modbus::{ModbusConnectionMode, RegisterMode};

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

    log::info!(
        "📂 Loaded {} port configurations from {:?}",
        configs.len(),
        path
    );
    Ok(configs)
}
