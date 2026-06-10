pub mod handlers;
pub mod master;
pub mod slave;

use anyhow::{anyhow, Error, Result};
use parking_lot::Mutex as ParkingMutex;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use clap::ArgMatches;
use rmodbus::server::{context::ModbusContext, storage::ModbusStorageSmall};

use crate::{
    api::modbus::{ModbusHook, ModbusResponse},
    protocol::status::types::{
        cli::OutputSink,
        modbus::{RegisterMode, StationConfig},
    },
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModbusIpcLogPayload<'a> {
    pub port: &'a str,
    pub direction: &'a str,
    pub frame: &'a [u8],
    pub station_id: Option<u8>,
    pub register_mode: Option<RegisterMode>,
    pub start_address: Option<u16>,
    pub quantity: Option<u16>,
    pub success: Option<bool>,
    pub error: Option<String>,
    pub config_index: Option<u16>,
}

pub(crate) fn emit_modbus_ipc_log(
    ipc: &mut Option<crate::cli::actions::IpcConnections>,
    payload: ModbusIpcLogPayload,
) {
    if let Some(ipc_conn) = ipc {
        let _ = ipc_conn
            .status
            .send(&crate::protocol::ipc::IpcMessage::ModbusData {
                port_name: payload.port.to_string(),
                direction: payload.direction.to_string(),
                data: format_hex_bytes(payload.frame),
                timestamp: None,
                station_id: payload.station_id,
                register_mode: payload.register_mode.map(|m| format!("{m:?}")),
                start_address: payload.start_address,
                quantity: payload.quantity,
                success: payload.success,
                error: payload.error,
                config_index: payload.config_index,
            });
    }
}

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

pub struct CliModbusHook {
    ipc: Arc<Mutex<Option<crate::cli::actions::IpcConnections>>>,
    output_sink: Option<OutputSink>,
}

impl CliModbusHook {
    #[must_use]
    pub fn new(matches: &ArgMatches) -> Self {
        let ipc = crate::cli::actions::setup_ipc(matches);
        let output_sink = matches
            .get_one::<String>("output")
            .and_then(|s| s.parse::<OutputSink>().ok());

        Self {
            ipc: Arc::new(Mutex::new(ipc)),
            output_sink,
        }
    }
}

impl ModbusHook for CliModbusHook {
    fn on_after_response(&self, _port: &str, response: &ModbusResponse) -> Result<()> {
        // Output to sink
        if let Some(sink) = &self.output_sink {
            let json = serde_json::to_string(response)?;
            sink.write(&json)?;
        }

        // Send IPC message
        // Note: We don't have the raw frame here easily unless we pass it in ModbusResponse or Hook.
        // The original emit_modbus_ipc_log took raw frame.
        // ModbusResponse has parsed values.
        // If we want to log raw frame, we need to change ModbusResponse or Hook.
        // For now, let's skip raw frame logging or reconstruct it?
        // Or maybe ModbusHook should receive raw frame?

        Ok(())
    }

    fn on_error(&self, port: &str, error: &Error) {
        // Log error via IPC?
        if let Ok(mut ipc) = self.ipc.lock() {
            if let Some(ref mut ipc_conn) = *ipc {
                let _ = ipc_conn
                    .status
                    .send(&crate::protocol::ipc::IpcMessage::PortError {
                        port_name: port.to_string(),
                        error: error.to_string(),
                        timestamp: None,
                    });
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
pub fn parse_data_line(
    line: &str,
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
    register_length: u16,
) -> Result<Vec<u16>> {
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow!("Empty line"));
    }

    match serde_json::from_str::<Vec<StationConfig>>(line) {
        Ok(stations) => {
            return extract_values_from_station_configs(
                &stations,
                station_id,
                register_mode,
                start_address,
                register_length,
            );
        }
        Err(_e) => {}
    }

    // Fallback: legacy format {"values": [...]}
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

    Err(anyhow!("Invalid data format: could not parse as Vec<StationConfig> or legacy {{\"values\": [...]}} format"))
}

pub(crate) fn extract_values_from_station_configs(
    stations: &[StationConfig],
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
    register_length: u16,
) -> Result<Vec<u16>> {
    if register_length == 0 {
        return Ok(Vec::new());
    }

    let station = stations
        .iter()
        .find(|station| station.station_id == station_id)
        .ok_or_else(|| anyhow!("Station {station_id} not found in data source payload"))?;

    let ranges = match register_mode {
        RegisterMode::Coils => &station.map.coils,
        RegisterMode::DiscreteInputs => &station.map.discrete_inputs,
        RegisterMode::Holding => &station.map.holding,
        RegisterMode::Input => &station.map.input,
    };

    let range = ranges
            .iter()
            .find(|range| {
                let end_address = range
                    .address_start
                    .saturating_add(range.length.saturating_sub(1));
                range.address_start <= start_address
                    && end_address
                        >= start_address.saturating_add(register_length.saturating_sub(1))
            })
            .ok_or_else(|| {
                anyhow!(
                    "Register range for station {station_id} does not cover address {start_address} (len {register_length})"
                )
            })?;

    if start_address < range.address_start {
        return Err(anyhow!(
                "Register range for station {station_id} starts at {} but requested address {start_address}",
                range.address_start
            ));
    }

    let offset = (start_address - range.address_start) as usize;
    let total_available = range.length.saturating_sub(offset as u16) as usize;

    if total_available < register_length as usize {
        return Err(anyhow!(
            "Register range length {} too small for requested length {} at offset {}",
            range.length,
            register_length,
            offset
        ));
    }

    let mut values = Vec::with_capacity(register_length as usize);
    let mut iter = range.initial_values.iter().skip(offset);
    for _ in 0..register_length {
        values.push(iter.next().copied().unwrap_or(0));
    }

    Ok(values)
}

/// Extract values from modbus storage
pub fn extract_values_from_storage(
    storage: &Arc<ParkingMutex<ModbusStorageSmall>>,
    start_addr: u16,
    length: u16,
    reg_mode: RegisterMode,
) -> Result<Vec<u16>> {
    let storage = storage.lock();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            RegisterMode::Holding => storage.get_holding(addr)?,
            RegisterMode::Input => storage.get_input(addr)?,
            RegisterMode::Coils => {
                u16::from(storage.get_coil(addr)?)
            }
            RegisterMode::DiscreteInputs => {
                u16::from(storage.get_discrete(addr)?)
            }
        };
        values.push(value);
    }

    Ok(values)
}

/// Write values to Modbus storage based on register mode.
///
/// This eliminates the repeated `match reg_mode { Holding | Input | Coils | DiscreteInputs }`
/// pattern that was duplicated ~36 times across the codebase.
pub fn set_registers_in_storage(
    storage: &ParkingMutex<ModbusStorageSmall>,
    reg_mode: RegisterMode,
    start_address: u16,
    values: &[u16],
) -> Result<()> {
    let mut ctx = storage.lock();
    for (i, &val) in values.iter().enumerate() {
        let addr = start_address + i as u16;
        match reg_mode {
            RegisterMode::Holding => ctx.set_holding(addr, val)?,
            RegisterMode::Input => ctx.set_input(addr, val)?,
            RegisterMode::Coils => ctx.set_coil(addr, val != 0)?,
            RegisterMode::DiscreteInputs => ctx.set_discrete(addr, val != 0)?,
        }
    }
    Ok(())
}

/// Record a changed register range for debounce bypass.
///
/// Bounds the internal vec to 1000 entries to prevent unbounded growth.
pub fn record_changed_range(
    changed_ranges: &ParkingMutex<Vec<(u16, u16, Instant)>>,
    address: u16,
    length: u16,
) {
    let mut cr = changed_ranges.lock();
    cr.push((address, length, Instant::now()));
    while cr.len() > 1000 {
        cr.remove(0);
    }
}

/// Build a `StationConfig` snapshot by reading current values from `storage`.
///
/// This clones the provided `station` and replaces each `RegisterRange`'s
/// `initial_values` with the values read from `storage` for that range.
pub fn build_station_snapshot_from_storage(
    storage: &Arc<ParkingMutex<ModbusStorageSmall>>,
    station: &StationConfig,
) -> Result<StationConfig> {
    let mut sc = station.clone();

    for range in &mut sc.map.holding {
        let vals = extract_values_from_storage(
            storage,
            range.address_start,
            range.length,
            RegisterMode::Holding,
        )?;
        range.initial_values = vals;
    }

    for range in &mut sc.map.coils {
        let vals = extract_values_from_storage(
            storage,
            range.address_start,
            range.length,
            RegisterMode::Coils,
        )?;
        range.initial_values = vals;
    }

    for range in &mut sc.map.discrete_inputs {
        let vals = extract_values_from_storage(
            storage,
            range.address_start,
            range.length,
            RegisterMode::DiscreteInputs,
        )?;
        range.initial_values = vals;
    }

    for range in &mut sc.map.input {
        let vals = extract_values_from_storage(
            storage,
            range.address_start,
            range.length,
            RegisterMode::Input,
        )?;
        range.initial_values = vals;
    }

    Ok(sc)
}
