pub mod master;
pub mod slave;

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::io::Write;

/// Convert a byte slice into an uppercase hexadecimal string separated by spaces.
pub(crate) fn format_hex_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Emit a Modbus IPC log message to the TUI if IPC connections are active.
pub(crate) fn emit_modbus_ipc_log(
    ipc_connections: &mut Option<crate::actions::IpcConnections>,
    port: &str,
    direction: &str,
    frame: &[u8],
    station_id: Option<u8>,
    register_mode: Option<aoba_protocol::status::types::modbus::RegisterMode>,
    start_address: Option<u16>,
    quantity: Option<u16>,
    success: Option<bool>,
    error: Option<String>,
    config_index: Option<u16>,
) {
    if let Some(ref mut ipc) = ipc_connections {
        let _ = ipc
            .status
            .send(&aoba_protocol::ipc::IpcMessage::ModbusData {
                port_name: port.to_string(),
                direction: direction.to_string(),
                data: format_hex_bytes(frame),
                timestamp: None,
                station_id,
                register_mode: register_mode.map(|mode| format!("{mode:?}")),
                start_address,
                quantity,
                success,
                error,
                config_index,
            });
    }
}

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
) -> Result<aoba_protocol::status::types::modbus::RegisterMode> {
    use aoba_protocol::status::types::modbus::RegisterMode;
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
    reg_mode: aoba_protocol::status::types::modbus::RegisterMode,
) -> Result<Vec<u16>> {
    use rmodbus::server::context::ModbusContext;

    let storage = storage.lock().unwrap();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            aoba_protocol::status::types::modbus::RegisterMode::Holding => {
                storage.get_holding(addr)?
            }
            aoba_protocol::status::types::modbus::RegisterMode::Input => storage.get_input(addr)?,
            aoba_protocol::status::types::modbus::RegisterMode::Coils => {
                if storage.get_coil(addr)? {
                    1
                } else {
                    0
                }
            }
            aoba_protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
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
