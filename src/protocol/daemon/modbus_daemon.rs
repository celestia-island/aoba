use anyhow::{anyhow, Result};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use crate::protocol::{
    modbus::*,
    status::{
        crc16_modbus, read_status,
        types::{self, port::PortLogEntry},
        with_port_read, with_port_write,
    },
};

/// Handle modbus communication for all active ports
pub fn handle_modbus_communication() -> Result<()> {
    let now = std::time::Instant::now();

    log::trace!("handle_modbus_communication called");

    // Get all ports that are currently active
    let active_ports = read_status(|status| {
        let mut ports = Vec::new();
        log::info!(
            "Checking {} total ports for modbus activity",
            status.ports.map.len()
        );
        for (port_name, port_arc) in &status.ports.map {
            if let Ok(port_data) = port_arc.read() {
                log::info!(
                    "  Port {}: state={:?}",
                    port_name,
                    match &port_data.state {
                        types::port::PortState::Free => "Free",
                        types::port::PortState::OccupiedByThis { owner: _ } => "OccupiedByThis",
                        types::port::PortState::OccupiedByOther => "OccupiedByOther",
                    }
                );

                if port_data.state.runtime_handle().is_some() {
                    let types::port::PortConfig::Modbus { mode, stations } = &port_data.config;
                    log::info!(
                        "    Config: Modbus with {} stations in {:?} mode",
                        stations.len(),
                        if mode.is_master() { "Master" } else { "Slave" }
                    );

                    if !stations.is_empty() {
                        ports.push((
                            port_name.clone(),
                            port_arc.clone(),
                            mode.clone(),
                            stations.clone(),
                        ));
                        log::trace!(
                            "Found active port {} with {} stations in {:?} mode",
                            port_name,
                            stations.len(),
                            if mode.is_master() { "Master" } else { "Slave" }
                        );
                    } else {
                        log::info!("    ⚠️ Port has no stations configured");
                    }
                }
            }
        }
        Ok(ports)
    })?;

    log::trace!(
        "handle_modbus_communication found {} active ports",
        active_ports.len()
    );

    for (port_name, port_arc, global_mode, stations) in active_ports {
        log::trace!(
            "Processing modbus communication for {} ({} mode, {} stations)",
            port_name,
            if global_mode.is_master() {
                "Master"
            } else {
                "Slave"
            },
            stations.len()
        );

        // Process each port's modbus communication
        // NOTE: The naming is counter-intuitive but kept for backwards compatibility:
        // - "Master" mode acts as a Modbus Slave (Server): listens and responds with data from storage
        // - "Slave" mode acts as a Modbus Master (Client): sends requests to query/write data
        //
        // For proper Modbus Master/Slave behavior, we swap the handler calls:
        match &global_mode {
            types::modbus::ModbusConnectionMode::Master => {
                // Master should respond to requests (like a Modbus Slave/Server)
                handle_slave_response_mode(&port_name, &port_arc, &stations, &global_mode, now)?;
            }
            types::modbus::ModbusConnectionMode::Slave { .. } => {
                // Slave should send requests (like a Modbus Master/Client)
                handle_master_query_mode(&port_name, &port_arc, &stations, &global_mode, now)?;
            }
        }
    }

    Ok(())
}

/// Handle responses to incoming requests (Modbus Slave/Server behavior)
/// This function listens for incoming requests and responds with data from storage.
/// Despite the confusing naming, this is called for "Master" mode which acts as a Modbus Slave.
pub fn handle_slave_response_mode(
    port_name: &str,
    port_arc: &Arc<RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
    _global_mode: &types::modbus::ModbusConnectionMode,
    now: Instant,
) -> Result<()> {
    log::trace!(
        "handle_slave_response_mode called for {} with {} stations",
        port_name,
        stations.len()
    );

    // Get runtime handle for receiving requests and sending responses
    let runtime_handle = with_port_read(port_arc, |port| port.state.runtime_handle().cloned());

    let Some(runtime_opt) = runtime_handle else {
        log::info!("Master mode {port_name}: runtime handle unavailable (read lock failed)");
        return Ok(());
    };

    let Some(runtime) = runtime_opt else {
        log::warn!("Master mode {port_name}: runtime handle missing despite OccupiedByThis state");
        return Ok(());
    };

    log::info!("Master mode {port_name}: checking for runtime events");

    // Process incoming requests from external slaves and generate responses
    let mut event_count = 0;
    loop {
        match runtime.evt_rx.try_recv() {
            Ok(event) => {
                event_count += 1;
                log::info!("Master mode {port_name}: processing event #{event_count}: {event:?}");
                match event {
                    crate::protocol::runtime::RuntimeEvent::FrameReceived(frame) => {
                        let hex_frame = frame
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ");

                        // Log the received request
                        let log_entry = PortLogEntry {
                            when: chrono::Local::now(),
                            raw: format!("Slave RX (request): {hex_frame}"),
                            parsed: None,
                        };

                        with_port_write(port_arc, |port| {
                            port.logs.push(log_entry);
                            if port.logs.len() > 1000 {
                                let excess = port.logs.len() - 1000;
                                port.logs.drain(0..excess);
                            }
                        });

                        // Parse the incoming frame to identify station ID and register address for throttling
                        if frame.len() < 2 {
                            log::debug!("Frame too short to parse for throttling check");
                            continue;
                        }

                        let station_id = frame[0];
                        let _function_code = frame[1];

                        // Extract register address from frame (bytes 2-3 for most function codes)
                        let register_address = if frame.len() >= 4 {
                            u16::from_be_bytes([frame[2], frame[3]])
                        } else {
                            0
                        };

                        // Check throttling: has enough time passed since last response to this station/register?
                        let should_throttle = with_port_read(port_arc, |port| {
                    let types::port::PortConfig::Modbus { stations, .. } = &port.config;

                    // Find matching station by station_id and register_address
                    for station in stations {
                        if station.station_id == station_id
                            && station.register_address == register_address
                        {
                            if let Some(last_response_time) = station.last_response_time {
                                let elapsed = now.duration_since(last_response_time);
                                if elapsed < Duration::from_secs(1) {
                                    log::debug!(
                                        "Throttling response to station {} register {}: only {}ms since last response (need 1000ms)",
                                        station_id,
                                        register_address,
                                        elapsed.as_millis()
                                    );
                                    return true;
                                }
                            }
                        }
                    }
                    false
                })
                .unwrap_or(false);

                        if should_throttle {
                            // Skip sending response due to throttling
                            log::debug!("Skipped response due to 1-second throttling");
                            continue;
                        }

                        // Try to parse and respond to the request
                        if let Ok(response) = generate_modbus_master_response(&frame, port_arc) {
                            // Send the response
                            if let Err(err) = runtime.cmd_tx.send(
                                crate::protocol::runtime::RuntimeCommand::Write(response.clone()),
                            ) {
                                let warn_msg = format!(
                            "Failed to send Modbus master response for port {port_name}: {err}"
                        );
                                log::warn!("{warn_msg}");

                                // Also write to port logs
                                let log_entry = PortLogEntry {
                                    when: chrono::Local::now(),
                                    raw: warn_msg.clone(),
                                    parsed: None,
                                };
                                with_port_write(port_arc, |port| {
                                    port.logs.push(log_entry);
                                    if port.logs.len() > 1000 {
                                        let excess = port.logs.len() - 1000;
                                        port.logs.drain(0..excess);
                                    }
                                });

                                continue;
                            }

                            // Update last_response_time for this station/register combination
                            with_port_write(port_arc, |port| {
                                let types::port::PortConfig::Modbus { stations, .. } =
                                    &mut port.config;

                                for station in stations.iter_mut() {
                                    if station.station_id == station_id
                                        && station.register_address == register_address
                                    {
                                        station.last_response_time = Some(now);
                                        break;
                                    }
                                }
                            });

                            // Log the sent response
                            let hex_response = response
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<Vec<_>>()
                                .join(" ");

                            let log_entry = PortLogEntry {
                                when: chrono::Local::now(),
                                raw: format!("Slave TX (response): {hex_response}"),
                                parsed: None,
                            };

                            with_port_write(port_arc, |port| {
                                port.logs.push(log_entry);
                                if port.logs.len() > 1000 {
                                    let excess = port.logs.len() - 1000;
                                    port.logs.drain(0..excess);
                                }
                            });

                            log::info!(
                                "Sent modbus master response for {port_name}: {hex_response}"
                            );
                        } else {
                            log::warn!(
                        "Failed to generate modbus response for port {port_name} (station_id={}, stations configured: {})",
                        station_id,
                        stations.len()
                    );
                        }
                    }
                    crate::protocol::runtime::RuntimeEvent::Error(error) => {
                        log::warn!("Modbus runtime error for {port_name}: {error}");
                    }
                    _ => {}
                }
            }
            Err(flume::TryRecvError::Empty) => {
                log::info!(
                    "Master mode {port_name}: runtime event queue empty after {event_count} events"
                );
                break;
            }
            Err(flume::TryRecvError::Disconnected) => {
                log::warn!("Master mode {port_name}: runtime event channel disconnected");
                break;
            }
        }
    }

    if event_count > 0 {
        log::info!("Master mode {port_name}: processed {event_count} events");
    } else {
        log::trace!("Master mode {port_name}: no events to process");
    }

    Ok(())
}

/// Handle sending requests and processing responses (Modbus Master/Client behavior)
/// This function periodically sends requests and waits for responses with timeout.
/// Despite the confusing naming, this is called for "Slave" mode which acts as a Modbus Master.
///
/// Key behaviors:
/// - Process ONE station at a time (sequential, not parallel)
/// - Send request and wait for response or 3-second timeout
/// - On success: move to next station immediately (respecting 1-second minimum interval)
/// - On timeout: stay on current station and retry on next poll
/// - Only move to next station after successful response (not after sending request)
pub fn handle_master_query_mode(
    port_name: &str,
    port_arc: &Arc<RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
    global_mode: &types::modbus::ModbusConnectionMode,
    now: Instant,
) -> Result<()> {
    log::info!(
        "🔄 handle_master_query_mode called for {port_name} with {} stations",
        stations.len()
    );

    // Get runtime handle for sending requests
    let runtime_handle = with_port_read(port_arc, |port| port.state.runtime_handle().cloned());

    let Some(Some(runtime)) = runtime_handle else {
        log::warn!("⚠️  handle_master_query_mode: No runtime handle for {port_name}");
        return Ok(());
    };

    log::info!("✅ handle_master_query_mode: Runtime handle obtained for {port_name}");

    // Get the current station index from the global mode
    let current_index = match global_mode {
        types::modbus::ModbusConnectionMode::Slave {
            current_request_at_station_index,
        } => *current_request_at_station_index,
        _ => 0,
    };

    log::info!(
        "📍 Current station index: {current_index} (total stations: {})",
        stations.len()
    );

    // Process incoming responses FIRST (before sending new requests)
    while let Ok(event) = runtime.evt_rx.try_recv() {
        match event {
            crate::protocol::runtime::RuntimeEvent::FrameReceived(frame) => {
                let hex_frame = frame
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                // Log the received response (Master RX)
                let log_entry = PortLogEntry {
                    when: chrono::Local::now(),
                    raw: format!("Master RX (response): {hex_frame}"),
                    parsed: None,
                };

                let pending_info = find_pending_station(port_arc);
                let parsed_values = if let Some(info) = pending_info {
                    match parse_master_response_values(&info, &frame) {
                        Ok(values) => values,
                        Err(err) => {
                            log::warn!(
                                "Failed to parse response for {port_name} station {}: {err}",
                                info.station_id
                            );
                            None
                        }
                    }
                } else {
                    log::debug!(
                        "Received response for {port_name} but no pending station was recorded"
                    );
                    None
                };

                with_port_write(port_arc, |port| {
                    port.logs.push(log_entry);
                    if port.logs.len() > 1000 {
                        let excess = port.logs.len() - 1000;
                        port.logs.drain(0..excess);
                    }

                    let types::port::PortConfig::Modbus { mode, stations } = &mut port.config;
                    if let Some(info) = pending_info {
                        if let Some(station) = stations.get_mut(info.index) {
                            station.req_success = station.req_success.saturating_add(1);
                            station.last_request_time = None;

                            if let Some(values) = parsed_values.as_ref() {
                                let required_len =
                                    station.register_length.max(values.len() as u16) as usize;
                                if station.last_values.len() < required_len {
                                    station.last_values.resize(required_len, 0);
                                }
                                for (idx, value) in values.iter().enumerate() {
                                    if let Some(slot) = station.last_values.get_mut(idx) {
                                        *slot = *value;
                                    }
                                }
                            }
                        } else {
                            log::debug!(
                                "Pending station index {} no longer available on {port_name}",
                                info.index
                            );
                        }

                        if let types::modbus::ModbusConnectionMode::Slave {
                            current_request_at_station_index,
                        } = mode
                        {
                            if !stations.is_empty() {
                                *current_request_at_station_index =
                                    (info.index + 1) % stations.len();
                            }
                        }
                    } else {
                        log::debug!(
                            "Received response for {port_name} but no matching pending request found"
                        );
                    }
                });
                log::info!("Received modbus slave response for {port_name}: {hex_frame}");
            }
            crate::protocol::runtime::RuntimeEvent::Error(error) => {
                log::warn!("Modbus runtime error for {port_name}: {error}");
            }
            _ => {}
        }
    }

    // Check for timeouts - on timeout, clear the request so it can be retried
    // but DON'T move to next station (stay on current station and retry)
    let mut logs_to_add: Vec<PortLogEntry> = Vec::new();
    with_port_write(port_arc, |port| {
        let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
        for (idx, station) in stations.iter_mut().enumerate() {
            if let Some(last_request_time) = station.last_request_time {
                // Check if request has timed out (3 seconds)
                if now.duration_since(last_request_time) > Duration::from_secs(3) {
                    // Prepare timeout log entry to add later
                    let log_entry = PortLogEntry {
                        when: chrono::Local::now(),
                        raw: format!(
                            "Slave Request Timeout: Station {} (idx {}) - will retry (3s timeout exceeded)",
                            station.station_id,
                            idx
                        ),
                        parsed: None,
                    };

                    logs_to_add.push(log_entry);

                    station.last_request_time = None; // Clear timeout tracking so we can retry
                    log::warn!(
                        "⏱️  Request timeout for {port_name} station {} (idx {}), staying on same station to retry",
                        station.station_id,
                        idx
                    );
                }
            }
        }
    });

    if !logs_to_add.is_empty() {
        with_port_write(port_arc, |port| {
            for log_entry in logs_to_add.drain(..) {
                port.logs.push(log_entry);
            }
            if port.logs.len() > 1000 {
                let excess = port.logs.len() - 1000;
                port.logs.drain(0..excess);
            }
        });
    }

    // Decide if a request should be sent for the current station
    let send_plan = with_port_write(port_arc, |port| {
        let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
        if let Some(station) = stations.get_mut(current_index) {
                log::info!(
                    "🔍 Slave mode: checking station {} (index {}), next_poll_at: {:?}, last_request_time: {:?}, now: {:?}",
                    station.station_id,
                    current_index,
                    station.next_poll_at,
                    station.last_request_time,
                    now
                );

                if now < station.next_poll_at {
                    log::info!(
                        "⏳ Slave mode: NOT YET time for station {} - waiting {:?} more",
                        station.station_id,
                        station.next_poll_at.duration_since(now)
                    );
                    return None;
                }

                if let Some(sent_at) = station.last_request_time {
                    log::info!(
                        "⏳ Slave mode: waiting for response from station {} (index {}) - request sent at {:?}",
                        station.station_id,
                        current_index,
                        sent_at
                    );
                    return None;
                }

                if !station.pending_requests.is_empty() {
                    let bytes = station.pending_requests.clone();
                    station.pending_requests.clear();
                    station.last_request_time = Some(now);
                    station.next_poll_at = now + Duration::from_secs(1);
                    station.req_total = station.req_total.saturating_add(1);
                    log::info!(
                        "📤 Sending pending write request for station {} ({} bytes)",
                        station.station_id,
                        bytes.len()
                    );
                    return Some((bytes, true));
                }

                match generate_modbus_request_bytes(&*station) {
                    Ok(bytes) => {
                        station.last_request_time = Some(now);
                        station.next_poll_at = now + Duration::from_secs(1);
                        station.req_total = station.req_total.saturating_add(1);
                        Some((bytes, false))
                    }
                    Err(err) => {
                        log::warn!(
                            "Failed to generate modbus request for station {}: {err}",
                            station.station_id
                        );
                        None
                    }
                }
        } else {
            log::debug!(
                "Slave mode: no station found at index {} (total stations: {})",
                current_index,
                stations.len()
            );
            None
        }
    })
    .flatten();

    if let Some((request_bytes, was_pending_write)) = send_plan {
        let send_payload = request_bytes.clone();
        if let Err(err) = runtime
            .cmd_tx
            .send(crate::protocol::runtime::RuntimeCommand::Write(
                send_payload,
            ))
        {
            let warn_msg = format!(
                "Failed to send modbus slave request for {port_name} station index {current_index}: {err}"
            );
            log::warn!("{warn_msg}");

            // On failure, roll back state and requeue pending writes if necessary.
            with_port_write(port_arc, |port| {
                let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
                if let Some(station) = stations.get_mut(current_index) {
                    station.last_request_time = None;
                    if was_pending_write {
                        station.pending_requests = request_bytes;
                    }
                }

                port.logs.push(PortLogEntry {
                    when: chrono::Local::now(),
                    raw: warn_msg.clone(),
                    parsed: None,
                });
                if port.logs.len() > 1000 {
                    let excess = port.logs.len() - 1000;
                    port.logs.drain(0..excess);
                }
            });
        } else {
            let hex_frame = request_bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");

            let log_entry = PortLogEntry {
                when: chrono::Local::now(),
                raw: format!("Master TX (request): {hex_frame}"),
                parsed: None,
            };

            with_port_write(port_arc, |port| {
                port.logs.push(log_entry);
                if port.logs.len() > 1000 {
                    let excess = port.logs.len() - 1000;
                    port.logs.drain(0..excess);
                }
            });

            log::info!(
                "📤 Sent modbus slave request for {port_name} (station index {current_index}): {hex_frame}"
            );
        }
    }

    Ok(())
}

/// Generate a modbus request for polling using the lightweight station state
pub fn generate_modbus_request_bytes(
    station: &types::modbus::ModbusRegisterItem,
) -> Result<Vec<u8>> {
    let length = station.register_length.min(125); // Limit to max modbus length
    let address = station.register_address;
    let slave_id = station.station_id;

    let (_, raw) = match station.register_mode {
        types::modbus::RegisterMode::Coils => {
            generate_pull_get_coils_request(slave_id, address, length)?
        }
        types::modbus::RegisterMode::DiscreteInputs => {
            generate_pull_get_discrete_inputs_request(slave_id, address, length)?
        }
        types::modbus::RegisterMode::Holding => {
            generate_pull_get_holdings_request(slave_id, address, length)?
        }
        types::modbus::RegisterMode::Input => {
            generate_pull_get_inputs_request(slave_id, address, length)?
        }
    };

    Ok(raw)
}

/// Generate a modbus master response to an incoming request (same logic as slave response)
pub fn generate_modbus_master_response(
    request: &[u8],
    port_arc: &Arc<RwLock<types::port::PortData>>,
) -> Result<Vec<u8>> {
    if request.len() < 6 {
        return Err(anyhow!("Request too short"));
    }

    let slave_id = request[0];
    let function_code = request[1];
    let register_address = u16::from_be_bytes([request[2], request[3]]);
    let quantity = u16::from_be_bytes([request[4], request[5]]);
    if quantity == 0 {
        return Err(anyhow!("Invalid Modbus quantity (0)"));
    }

    let expected_mode = match function_code {
        0x01 => types::modbus::RegisterMode::Coils,
        0x02 => types::modbus::RegisterMode::DiscreteInputs,
        0x03 => types::modbus::RegisterMode::Holding,
        0x04 => types::modbus::RegisterMode::Input,
        other => {
            return Err(anyhow!(
                "Unsupported Modbus function 0x{other:02X} for auto response generation"
            ));
        }
    };

    let snapshot = with_port_read(port_arc, |port| {
        let types::port::PortConfig::Modbus { stations, .. } = &port.config;
        stations
            .iter()
            .find(|station| {
                if station.station_id != slave_id {
                    return false;
                }
                if station.register_mode != expected_mode {
                    return false;
                }
                let station_start = station.register_address;
                let station_end = station_start.saturating_add(station.register_length);
                let request_end =
                    register_address.checked_add(quantity).unwrap_or(u16::MAX);
                register_address >= station_start && request_end <= station_end
            })
            .map(|station| StationSnapshot {
                register_address: station.register_address,
                last_values: station.last_values.clone(),
            })
    })
    .flatten()
    .ok_or_else(|| {
        anyhow!(
            "No matching station configuration for request (id={}, func=0x{function_code:02X}, addr={}, qty={})",
            slave_id,
            register_address,
            quantity
        )
    })?;

    let offset = register_address
        .checked_sub(snapshot.register_address)
        .ok_or_else(|| anyhow!("Request address out of configured range"))?
        as usize;
    let quantity_usize = quantity as usize;

    let mut response = Vec::new();
    response.push(slave_id);
    response.push(function_code);

    match function_code {
        0x01 | 0x02 => {
            let byte_count = ((quantity + 7) / 8) as usize;
            response.push(byte_count as u8);
            let mut data = vec![0u8; byte_count];
            for idx in 0..quantity_usize {
                let value = snapshot.last_values.get(offset + idx).copied().unwrap_or(0);
                if value != 0 {
                    data[idx / 8] |= 1 << (idx % 8);
                }
            }
            response.extend_from_slice(&data);
        }
        0x03 | 0x04 => {
            response.push((quantity * 2) as u8);
            for idx in 0..quantity_usize {
                let value = snapshot.last_values.get(offset + idx).copied().unwrap_or(0);
                response.push((value >> 8) as u8);
                response.push((value & 0xFF) as u8);
            }
        }
        _ => unreachable!("Unsupported function already filtered above"),
    }

    let crc = crc16_modbus(&response);
    response.push((crc & 0xFF) as u8);
    response.push((crc >> 8) as u8);

    Ok(response)
}

#[derive(Clone)]
struct StationSnapshot {
    register_address: u16,
    last_values: Vec<u16>,
}

#[derive(Clone, Copy, Debug)]
struct PendingStationInfo {
    index: usize,
    station_id: u8,
    register_mode: types::modbus::RegisterMode,
    register_address: u16,
    register_length: u16,
}

impl PendingStationInfo {
    fn requested_quantity(&self) -> u16 {
        // Keep polling window capped to Modbus specification limits used elsewhere.
        self.register_length.min(125)
    }
}

fn find_pending_station(
    port_arc: &Arc<RwLock<types::port::PortData>>,
) -> Option<PendingStationInfo> {
    with_port_read(port_arc, |port| {
        let types::port::PortConfig::Modbus { stations, .. } = &port.config;
        stations.iter().enumerate().find_map(|(idx, station)| {
            station.last_request_time.map(|_| PendingStationInfo {
                index: idx,
                station_id: station.station_id,
                register_mode: station.register_mode,
                register_address: station.register_address,
                register_length: station.register_length,
            })
        })
    })
    .flatten()
}

fn parse_master_response_values(
    info: &PendingStationInfo,
    frame: &[u8],
) -> Result<Option<Vec<u16>>> {
    if frame.len() < 3 {
        return Err(anyhow!("Response frame too short"));
    }

    if frame[0] != info.station_id {
        return Ok(None);
    }

    match (info.register_mode, frame[1]) {
        (types::modbus::RegisterMode::Coils, 0x01) => {
            let count = info.requested_quantity();
            let (mut request, _) =
                generate_pull_get_coils_request(info.station_id, info.register_address, count)?;
            let values = parse_pull_get_coils(&mut request, frame.to_vec(), count)?;
            Ok(Some(bools_to_u16(values)))
        }
        (types::modbus::RegisterMode::DiscreteInputs, 0x02) => {
            let count = info.requested_quantity();
            let (mut request, _) = generate_pull_get_discrete_inputs_request(
                info.station_id,
                info.register_address,
                count,
            )?;
            let values = parse_pull_get_discrete_inputs(&mut request, frame.to_vec(), count)?;
            Ok(Some(bools_to_u16(values)))
        }
        (types::modbus::RegisterMode::Holding, 0x03) => {
            let count = info.requested_quantity();
            let (mut request, _) =
                generate_pull_get_holdings_request(info.station_id, info.register_address, count)?;
            let values = parse_pull_get_holdings(&mut request, frame.to_vec())?;
            Ok(Some(values))
        }
        (types::modbus::RegisterMode::Input, 0x04) => {
            let count = info.requested_quantity();
            let (mut request, _) =
                generate_pull_get_inputs_request(info.station_id, info.register_address, count)?;
            let values = parse_pull_get_inputs(&mut request, frame.to_vec())?;
            Ok(Some(values))
        }
        _ => Ok(None),
    }
}

fn bools_to_u16(values: Vec<bool>) -> Vec<u16> {
    values
        .into_iter()
        .map(|flag| if flag { 1 } else { 0 })
        .collect()
}
