/// CLI serializable status structures shared across the application.
///
/// These types originated in `src/cli/status/serializable.rs` but are now
/// part of the shared protocol layer so both CLI and test utilities can
/// reuse them without duplicating definitions.
use anyhow::{anyhow, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};

use super::RegisterMode;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliMode {
    SlaveListen,
    SlavePoll,
    MasterProvide,
}

/// Output sink for CLI operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputSink {
    Stdout,
    File { path: String },
    Pipe { path: String },
}

impl std::str::FromStr for OutputSink {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("file:") {
            Ok(OutputSink::File {
                path: path.to_string(),
            })
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(OutputSink::Pipe {
                path: name.to_string(),
            })
        } else {
            // Default to stdout if not specified or invalid?
            // The original code returned error for invalid format, but handled "stdout" implicitly or via Option.
            // Here we implement FromStr for the explicit formats.
            Err(anyhow!(
                "Invalid output format. Use file:<path> or pipe:<name>"
            ))
        }
    }
}

impl OutputSink {
    /// Write output to the sink
    pub fn write(&self, data: &str) -> Result<()> {
        use std::io::Write;
        match self {
            OutputSink::Stdout => {
                println!("{data}");
                Ok(())
            }
            OutputSink::File { path } => {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                writeln!(file, "{data}")?;
                Ok(())
            }
            OutputSink::Pipe { path } => {
                let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
                writeln!(file, "{data}")?;
                Ok(())
            }
        }
    }
}

impl CliStatus {
    /// Create a new CLI status for slave listen mode
    pub fn new_slave_listen(
        port_name: String,
        station_id: u8,
        register_mode: RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        Self {
            port_name,
            station_id,
            register_mode,
            register_address,
            register_length,
            mode: CliMode::SlaveListen,
            timestamp: Local::now().to_rfc3339(),
        }
    }

    /// Create a new CLI status for slave poll mode
    pub fn new_slave_poll(
        port_name: String,
        station_id: u8,
        register_mode: RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        Self {
            port_name,
            station_id,
            register_mode,
            register_address,
            register_length,
            mode: CliMode::SlavePoll,
            timestamp: Local::now().to_rfc3339(),
        }
    }

    /// Create a new CLI status for master provide mode
    pub fn new_master_provide(
        port_name: String,
        station_id: u8,
        register_mode: RegisterMode,
        register_address: u16,
        register_length: u16,
    ) -> Self {
        Self {
            port_name,
            station_id,
            register_mode,
            register_address,
            register_length,
            mode: CliMode::MasterProvide,
            timestamp: Local::now().to_rfc3339(),
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| anyhow!("Failed to serialize CLI status: {e}"))
    }
}
