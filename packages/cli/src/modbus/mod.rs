pub mod master;
pub mod slave;

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::io::Write;

/// Response structure for modbus operations
#[derive(Serialize, Clone)]
pub struct ModbusResponse {
    pub station_id: u8,
    pub register_address: u16,
    pub register_mode: String,
    pub values: Vec<u16>,
    pub timestamp: String,
}

/// Data source for master mode
#[derive(Clone)]
pub enum DataSource {
    File(String),
    Pipe(String),
}

impl std::str::FromStr for DataSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("file:") {
            Ok(DataSource::File(path.to_string()))
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(DataSource::Pipe(name.to_string()))
        } else {
            Err(anyhow!(
                "Invalid data source format. Use file:<path> or pipe:<name>"
            ))
        }
    }
}

/// Output sink for slave mode
pub enum OutputSink {
    Stdout,
    File(String),
    Pipe(String),
}

impl std::str::FromStr for OutputSink {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("file:") {
            Ok(OutputSink::File(path.to_string()))
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(OutputSink::Pipe(name.to_string()))
        } else {
            Err(anyhow!(
                "Invalid output format. Use file:<path> or pipe:<name>"
            ))
        }
    }
}

impl OutputSink {
    /// Write output to the sink
    pub fn write(&self, data: &str) -> Result<()> {
        match self {
            OutputSink::Stdout => {
                println!("{data}");
                Ok(())
            }
            OutputSink::File(path) => {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                writeln!(file, "{data}")?;
                Ok(())
            }
            OutputSink::Pipe(path) => {
                let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
                writeln!(file, "{data}")?;
                Ok(())
            }
        }
    }
}

/// Parse register mode from string
pub fn parse_register_mode(
    mode: &str,
) -> Result<crate::protocol::status::types::modbus::RegisterMode> {
    use crate::protocol::status::types::modbus::RegisterMode;
    match mode.to_lowercase().as_str() {
        "holding" => Ok(RegisterMode::Holding),
        "input" => Ok(RegisterMode::Input),
        "coils" => Ok(RegisterMode::Coils),
        "discrete" => Ok(RegisterMode::DiscreteInputs),
        _ => Err(anyhow!("Invalid register mode: {mode}")),
    }
}

/// Parse a data line in JSON format
pub fn parse_data_line(line: &str) -> Result<Vec<u16>> {
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow!("Empty line"));
    }

    // Try to parse as JSON object with "values" field
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(values) = json.get("values") {
            if let Some(arr) = values.as_array() {
                let mut result = Vec::new();
                for val in arr {
                    if let Some(num) = val.as_u64() {
                        result.push(num as u16);
                    }
                }
                return Ok(result);
            }
        }
    }

    Err(anyhow!("Invalid data format"))
}

/// Extract values from modbus storage
pub fn extract_values_from_storage(
    storage: &std::sync::Arc<std::sync::Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    start_addr: u16,
    length: u16,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
) -> Result<Vec<u16>> {
    use rmodbus::server::context::ModbusContext;

    let storage = storage.lock().unwrap();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                storage.get_holding(addr)?
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => {
                storage.get_input(addr)?
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                if storage.get_coil(addr)? {
                    1
                } else {
                    0
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                if storage.get_discrete(addr)? {
                    1
                } else {
                    0
                }
            }
        };
        values.push(value);
    }

    Ok(values)
}
