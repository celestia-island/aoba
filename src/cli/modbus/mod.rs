pub mod master;
pub mod slave;

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{io::Write, time::Duration};

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
#[derive(Clone, Debug)]
pub(crate) struct ModbusIpcLogPayload<'a> {
    pub port: &'a str,
    pub direction: &'a str,
    pub frame: &'a [u8],
    pub station_id: Option<u8>,
    pub register_mode: Option<crate::protocol::status::types::modbus::RegisterMode>,
    pub start_address: Option<u16>,
    pub quantity: Option<u16>,
    pub success: Option<bool>,
    pub error: Option<String>,
    pub config_index: Option<u16>,
}

/// Emit a Modbus IPC log message to the TUI if IPC connections are active.
pub(crate) fn emit_modbus_ipc_log(
    ipc_connections: &mut Option<crate::cli::actions::IpcConnections>,
    payload: ModbusIpcLogPayload<'_>,
) {
    if let Some(ref mut ipc) = ipc_connections {
        let _ = ipc
            .status
            .send(&crate::protocol::ipc::IpcMessage::ModbusData {
                port_name: payload.port.to_string(),
                direction: payload.direction.to_string(),
                data: format_hex_bytes(payload.frame),
                timestamp: None,
                station_id: payload.station_id,
                register_mode: payload.register_mode.map(|mode| format!("{mode:?}")),
                start_address: payload.start_address,
                quantity: payload.quantity,
                success: payload.success,
                error: payload.error,
                config_index: payload.config_index,
            });
    }
}

/// Open a serial port with the requested timeout, enabling exclusive access on Unix systems.
pub(crate) fn open_serial_port(
    port: &str,
    baud_rate: u32,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>> {
    let builder = serialport::new(port, baud_rate).timeout(timeout);

    #[cfg(unix)]
    {
        let mut handle = builder
            .open_native()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;
        handle
            .set_exclusive(true)
            .map_err(|err| anyhow!("Failed to acquire exclusive access to {port}: {err}"))?;
        Ok(Box::new(handle))
    }

    #[cfg(not(unix))]
    {
        builder
            .open()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))
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
    Manual,
    File(String),
    Pipe(String),
    TransparentForward(String), // port name
    MqttServer(String),         // URL
    HttpServer(String),         // URL
    IpcPipe(String),            // pipe path
}

impl std::str::FromStr for DataSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "manual" {
            Ok(DataSource::Manual)
        } else if let Some(path) = s.strip_prefix("file:") {
            Ok(DataSource::File(path.to_string()))
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(DataSource::Pipe(name.to_string()))
        } else if let Some(port) = s.strip_prefix("transparent:") {
            Ok(DataSource::TransparentForward(port.to_string()))
        } else if let Some(url) = s.strip_prefix("mqtt://") {
            Ok(DataSource::MqttServer(format!("mqtt://{}", url)))
        } else if let Some(url) = s.strip_prefix("mqtts://") {
            Ok(DataSource::MqttServer(format!("mqtts://{}", url)))
        } else if let Some(url) = s.strip_prefix("http://") {
            Ok(DataSource::HttpServer(format!("http://{}", url)))
        } else if let Some(url) = s.strip_prefix("https://") {
            Ok(DataSource::HttpServer(format!("https://{}", url)))
        } else if let Some(path) = s.strip_prefix("ipc:") {
            Ok(DataSource::IpcPipe(path.to_string()))
        } else {
            Err(anyhow!(
                "Invalid data source format. Use: manual, transparent:<port>, mqtt://<url>, http://<url>, ipc:<path>, or file:<path>"
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
