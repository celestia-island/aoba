//! Modbus RTU baud-rate auto-detection (probe).
//!
//! Provides [`probe_modbus_rtu_baud`] which sweeps a set of candidate baud
//! rates on a physical serial port, sending a single Modbus read-holding
//! request at each rate and returning the first baud rate that elicits a
//! valid response.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::{sync::Arc, time::Duration};

use crate::{api::utils::open_serial_port, protocol::status::types::modbus::RegisterMode};

/// Default candidate baud rates for Modbus RTU probes.
pub const DEFAULT_BAUD_RATES: &[u32] = &[2400, 4800, 9600, 19200, 38400, 57600, 115200];

/// Attempt to discover the baud rate of a Modbus RTU slave on `port`.
///
/// For each baud rate in `baud_rates`, the function opens the port, sends a
/// single *read-holding-registers* request (function code 0x03) for 1 register
/// at address 0 with the supplied `station_id`, and waits up to `timeout` for
/// a valid response.
///
/// Returns `Ok(Some(baud_rate))` on the first successful response, or
/// `Ok(None)` when no baud rate succeeds.
///
/// # Errors
///
/// Returns an error only when the port cannot be opened at all (e.g. does not
/// exist or is a virtual port).
pub fn probe_modbus_rtu_baud(
    port: &str,
    station_id: u8,
    baud_rates: &[u32],
    timeout: Duration,
) -> Result<Option<u32>> {
    if baud_rates.is_empty() {
        return Ok(None);
    }

    if crate::api::is_virtual_port(port) {
        return Err(anyhow!(
            "Port {} is a virtual port — baud-rate probing requires a physical serial port",
            port
        ));
    }

    for &baud in baud_rates {
        match try_probe_at_baud(port, station_id, baud, timeout) {
            Ok(true) => return Ok(Some(baud)),
            Ok(false) => continue,
            Err(e) => {
                log::debug!("probe at {} baud failed: {}", baud, e);
                continue;
            }
        }
    }

    Ok(None)
}

/// Send one read-holding request at `baud` and check for a valid response.
fn try_probe_at_baud(port: &str, station_id: u8, baud: u32, timeout: Duration) -> Result<bool> {
    let sync_port = open_serial_port(port, baud, timeout)?;
    let port_arc = Arc::new(Mutex::new(sync_port));

    let result = crate::api::modbus::core::execute_single_poll_internal(
        &port_arc,
        station_id,
        0,
        1,
        RegisterMode::Holding,
    );

    match result {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
