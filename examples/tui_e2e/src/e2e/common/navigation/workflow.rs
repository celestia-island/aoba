use anyhow::{anyhow, Result};
use expectrl::Expect;

use super::super::config::{RegisterModeExt, StationConfig};
use super::super::station::{
    configure_register_count, configure_register_type, configure_start_address,
    configure_station_id, create_station, ensure_connection_mode, focus_create_station_button,
    focus_station, initialize_slave_registers, save_configuration_and_verify,
};
use super::super::validation::check_station_config;
use ci_utils::*;

fn is_default_master_station(station: &TuiModbusMaster) -> bool {
    station.station_id == 1
        && station.register_type == "Holding"
        && station.start_address == 0
        && station.register_count <= 1
}

fn is_default_slave_station(station: &TuiModbusSlave) -> bool {
    station.station_id == 1
        && station.register_type == "Holding"
        && station.start_address == 0
        && station.register_count <= 1
}

/// Configure a single Modbus station with retries and validation.
///
/// This orchestrates the full workflow of toggling the connection mode, creating
/// a station (or reusing the default one), updating its fields, populating slave
/// registers when requested, and persisting the result.
pub async fn configure_tui_station<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
    config: &StationConfig,
) -> Result<()> {
    log::info!("⚙️  Configuring TUI station: {config:?}");

    let mut status = read_tui_status()?;
    if status.ports.iter().all(|p| p.name != port1) {
        return Err(anyhow!("Port {} not found in TUI status", port1));
    }

    ensure_connection_mode(session, cap, config.is_master()).await?;

    status = read_tui_status()?;

    let port = status
        .ports
        .iter()
        .find(|p| p.name == port1)
        .ok_or_else(|| anyhow!("Port {} not found in TUI status", port1))?;

    let (reuse_existing, existing_index) = if config.is_master() {
        match port.modbus_masters.len() {
            0 => (false, None),
            1 if is_default_master_station(&port.modbus_masters[0]) => {
                log::info!("♻️  Reusing initial master station at index 0");
                (true, Some(0))
            }
            _ => (false, None),
        }
    } else {
        match port.modbus_slaves.len() {
            0 => (false, None),
            1 if is_default_slave_station(&port.modbus_slaves[0]) => {
                log::info!("♻️  Reusing initial slave station at index 0");
                (true, Some(0))
            }
            _ => (false, None),
        }
    };

    if !reuse_existing {
        focus_create_station_button(session, cap).await?;
    }

    let station_index = if reuse_existing {
        existing_index.unwrap()
    } else {
        create_station(session, cap, port1, config.is_master()).await?
    };

    focus_station(session, cap, port1, station_index, config.is_master()).await?;

    configure_station_id(
        session,
        cap,
        port1,
        station_index,
        config.station_id,
        config.is_master(),
    )
    .await?;

    configure_register_type(
        session,
        cap,
        port1,
        station_index,
        config.register_mode(),
        config.is_master(),
    )
    .await?;

    configure_start_address(
        session,
        cap,
        port1,
        station_index,
        config.start_address(),
        config.is_master(),
    )
    .await?;

    configure_register_count(
        session,
        cap,
        port1,
        station_index,
        config.register_count(),
        config.is_master(),
    )
    .await?;

    if !config.is_master() {
        if let Some(values) = config.register_values() {
            initialize_slave_registers(session, cap, &values, config.register_mode()).await?;
        }

        focus_create_station_button(session, cap).await?;
    }

    save_configuration_and_verify(session, cap, port1).await?;

    let final_checks = check_station_config(
        port1,
        station_index,
        config.is_master(),
        config.station_id,
        config.register_mode().status_value(),
        config.start_address(),
        config.register_count(),
    );
    execute_cursor_actions(session, cap, &final_checks, "verify_station_config").await?;

    log::info!("✅ Station configured and verified");
    Ok(())
}
