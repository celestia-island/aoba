use anyhow::Result;
use std::{fs, io::Write, path::PathBuf};

use aoba_protocol::status::types::modbus::{ModbusRegisterItem, RegisterMode};

fn create_cli_data_source_path(
    port_name: &str,
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
) -> PathBuf {
    let sanitized: String = port_name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();
    let fallback = if sanitized.is_empty() {
        "port".to_string()
    } else {
        sanitized
    };

    let type_code = match register_mode {
        RegisterMode::Coils => 1,
        RegisterMode::DiscreteInputs => 2,
        RegisterMode::Holding => 3,
        RegisterMode::Input => 4,
    };

    let mut path = std::env::temp_dir();
    path.push(format!(
        "aoba_cli_{fallback}_s{station_id}_t{type_code:02}_a{start_address:04X}.jsonl"
    ));
    path
}

pub fn write_cli_data_snapshot(path: &PathBuf, values: &[u16], truncate: bool) -> Result<()> {
    let payload = serde_json::json!({ "values": values });
    let serialized = serde_json::to_string(&payload)?;

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }

    let mut file = options.open(path)?;
    writeln!(file, "{serialized}")?;
    Ok(())
}

pub fn station_values_for_cli(station: &ModbusRegisterItem) -> Vec<u16> {
    let target_len = station.register_length as usize;
    if target_len == 0 {
        return Vec::new();
    }

    let mut values = station.last_values.clone();
    values.resize(target_len, 0);
    values
}

pub fn register_mode_to_cli_arg(mode: RegisterMode) -> &'static str {
    match mode {
        RegisterMode::Coils => "coils",
        RegisterMode::DiscreteInputs => "discrete",
        RegisterMode::Holding => "holding",
        RegisterMode::Input => "input",
    }
}

pub fn cli_mode_to_port_mode(
    mode: &aoba_cli::status::CliMode,
) -> aoba_protocol::status::types::port::PortSubprocessMode {
    use aoba_protocol::status::types::port::PortSubprocessMode;

    match mode {
        aoba_cli::status::CliMode::SlaveListen => PortSubprocessMode::SlaveListen,
        aoba_cli::status::CliMode::SlavePoll => PortSubprocessMode::SlavePoll,
        aoba_cli::status::CliMode::MasterProvide => PortSubprocessMode::MasterProvide,
    }
}

pub fn initialize_cli_data_source(
    port_name: &str,
    stations: &[ModbusRegisterItem],
) -> Result<(PathBuf, u16, u16, u16)> {
    use anyhow::anyhow;

    if stations.is_empty() {
        return Err(anyhow!(
            "No stations provided for data source initialization"
        ));
    }

    let first = &stations[0];
    let station_id = first.station_id;
    let register_mode = first.register_mode;

    let mut min_addr = u16::MAX;
    let mut max_addr = 0u16;

    for station in stations {
        if station.station_id != station_id {
            log::warn!(
                "initialize_cli_data_source: skipping station with different ID {} (expected {})",
                station.station_id,
                station_id
            );
            continue;
        }
        if station.register_mode != register_mode {
            log::warn!(
                "initialize_cli_data_source: skipping station with different register mode (expected {register_mode:?})"
            );
            continue;
        }

        let start = station.register_address;
        let end = start + station.register_length;

        if start < min_addr {
            min_addr = start;
        }
        if end > max_addr {
            max_addr = end;
        }
    }

    let total_length = max_addr - min_addr;
    log::info!(
        "initialize_cli_data_source: merging {} stations for {port_name}, station_id={}, type={:?}, address range: 0x{:04X}-0x{:04X} (length={})",
        stations.len(),
        station_id,
        register_mode,
        min_addr,
        max_addr,
        total_length
    );

    let mut merged_data = vec![0u16; total_length as usize];

    for station in stations {
        if station.station_id != station_id || station.register_mode != register_mode {
            continue;
        }

        let start_offset = (station.register_address - min_addr) as usize;
        let station_values = station_values_for_cli(station);

        log::debug!(
            "  Merging station at 0x{:04X}, length={}, into offset {}",
            station.register_address,
            station_values.len(),
            start_offset
        );

        for (i, &value) in station_values.iter().enumerate() {
            let target_idx = start_offset + i;
            if target_idx < merged_data.len() {
                merged_data[target_idx] = value;
            }
        }
    }

    let path = create_cli_data_source_path(port_name, station_id, register_mode, min_addr);

    if let Err(err) = write_cli_data_snapshot(&path, &merged_data, true) {
        log::error!(
            "initialize_cli_data_source: failed to write merged snapshot for {port_name}: {err}"
        );
        return Err(err);
    }

    log::info!(
        "initialize_cli_data_source: created merged data source at {} (station_id={}, addr=0x{:04X}, length={})",
        path.display(),
        station_id,
        min_addr,
        total_length
    );

    Ok((path, station_id as u16, min_addr, total_length))
}
