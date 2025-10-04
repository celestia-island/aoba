use anyhow::{anyhow, Result};
use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use rmodbus::{
    server::{context::ModbusContext, ModbusFrame},
    ModbusProto,
};

use crate::protocol::{
    modbus::*,
    status::{
        read_status,
        types::{self, port::PortLogEntry},
        with_port_read, with_port_write,
    },
};

/// Handle modbus communication for all active ports
pub fn handle_modbus_communication() -> Result<()> {
    let now = std::time::Instant::now();

    // Get all ports that are currently active
    let active_ports = read_status(|status| {
        let mut ports = Vec::new();
        for (port_name, port_arc) in &status.ports.map {
            if let Ok(port_data) = port_arc.read() {
                if let types::port::PortState::OccupiedByThis { runtime: _, .. } = &port_data.state
                {
                    let types::port::PortConfig::Modbus { mode, stations } = &port_data.config;
                    if !stations.is_empty() {
                        ports.push((
                            port_name.clone(),
                            port_arc.clone(),
                            mode.clone(),
                            stations.clone(),
                        ));
                        log::debug!(
                            "Found active port {} with {} stations in {:?} mode",
                            port_name,
                            stations.len(),
                            if mode.is_master() { "Master" } else { "Slave" }
                        );
                    }
                }
            }
        }
        Ok(ports)
    })?;

    for (port_name, port_arc, global_mode, stations) in active_ports {
        // Process each port's modbus communication
        // NOTE: The naming is counter-intuitive but kept for backwards compatibility:
        // - "Master" mode acts as a Modbus Slave (Server): listens and responds with data from storage
        // - "Slave" mode acts as a Modbus Master (Client): sends requests to query/write data
        //
        // For proper Modbus Master/Slave behavior, we swap the handler calls:
        match &global_mode {
            types::modbus::ModbusConnectionMode::Master { .. } => {
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
    global_mode: &types::modbus::ModbusConnectionMode,
    _now: Instant,
) -> Result<()> {
    // Get runtime handle for receiving requests and sending responses
    let runtime_handle = with_port_read(port_arc, |port| {
        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
            Some(runtime.clone())
        } else {
            None
        }
    });

    let Some(Some(runtime)) = runtime_handle else {
        return Ok(());
    };

    // Process incoming requests from external slaves and generate responses
    while let Ok(event) = runtime.evt_rx.try_recv() {
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

                // Try to parse and respond to the request
                if let Ok(response) = generate_modbus_master_response(&frame, stations, global_mode)
                {
                    // Send the response
                    if let Err(e) =
                        runtime
                            .cmd_tx
                            .send(crate::protocol::runtime::RuntimeCommand::Write(
                                response.clone(),
                            ))
                    {
                        let warn_msg = format!(
                            "Failed to send Modbus master response for port {port_name}: {e}"
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

                    log::info!("Sent modbus master response for {port_name}: {hex_response}");
                } else {
                    log::debug!(
                        "Could not generate a response for the Modbus request: {hex_frame}"
                    );
                }
            }
            crate::protocol::runtime::RuntimeEvent::Error(error) => {
                log::warn!("Modbus runtime error for {port_name}: {error}");
            }
            _ => {}
        }
    }

    Ok(())
}

/// Handle sending requests and processing responses (Modbus Master/Client behavior)
/// This function periodically sends requests and waits for responses with timeout.
/// Despite the confusing naming, this is called for "Slave" mode which acts as a Modbus Master.
pub fn handle_master_query_mode(
    port_name: &str,
    port_arc: &Arc<RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
    global_mode: &types::modbus::ModbusConnectionMode,
    now: Instant,
) -> Result<()> {
    // Get runtime handle for sending requests
    let runtime_handle = with_port_read(port_arc, |port| {
        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
            Some(runtime.clone())
        } else {
            None
        }
    });

    let Some(Some(runtime)) = runtime_handle else {
        return Ok(());
    };

    // Get the current station index from the global mode
    let current_index = match global_mode {
        types::modbus::ModbusConnectionMode::Slave {
            current_request_at_station_index,
            storage: _,
        } => *current_request_at_station_index,
        _ => 0,
    };

    // Only process the current station
    if let Some(station) = stations.get(current_index) {
        log::debug!(
            "Slave mode: checking station {} (index {}), next_poll_at: {:?}, now: {:?}",
            station.station_id,
            current_index,
            station.next_poll_at,
            now
        );

        if now >= station.next_poll_at {
            log::debug!(
                "Slave mode: time to send request for station {}",
                station.station_id
            );

            // Decide whether to send: skip if a previous request is still pending
            let mut should_send = true;
            if let Some(last_rt) = station.last_request_time {
                if now.duration_since(last_rt) <= Duration::from_secs(3) {
                    log::debug!(
                        "Skipping send for station {} because previous request is still pending",
                        station.station_id
                    );
                    should_send = false;
                }
            }

            if should_send {
                match generate_modbus_request_with_cache(station, port_arc) {
                    Ok(request_bytes) => {
                        // Try to send the request
                        if let Err(e) =
                            runtime
                                .cmd_tx
                                .send(crate::protocol::runtime::RuntimeCommand::Write(
                                    request_bytes.clone(),
                                ))
                        {
                            let warn_msg = format!(
                                "Failed to send modbus slave request for {port_name} station {}: {e}",
                                station.station_id
                            );
                            log::warn!("{warn_msg}");

                            // Also write this warning into the port logs so the UI can show it
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

                                // Ensure we clear station.last_request_time if we had set it earlier
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &mut port.config;
                                if let Some(station_mut) = stations.get_mut(current_index) {
                                    station_mut.last_request_time = None;
                                }
                            });
                        } else {
                            // Log the sent frame
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

                                // Update the station's next poll time (1 second interval)
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &mut port.config;
                                if let Some(station_mut) = stations.get_mut(current_index) {
                                    station_mut.next_poll_at = now + Duration::from_secs(1);
                                    station_mut.last_request_time = Some(now);
                                }
                            });

                            // Move to next station for the next poll cycle in a separate write
                            with_port_write(port_arc, |port| {
                                let types::port::PortConfig::Modbus { mode, stations } =
                                    &mut port.config;
                                if let types::modbus::ModbusConnectionMode::Slave {
                                    current_request_at_station_index,
                                    storage: _,
                                } = mode
                                {
                                    *current_request_at_station_index =
                                        (current_index + 1) % stations.len();
                                }
                            });

                            log::info!(
                                "Sent modbus slave request for {port_name} station {}: {hex_frame}",
                                station.station_id
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to generate modbus slave request for {port_name} station {}: {e}",
                            station.station_id
                        );
                    }
                }
            }
        }
    } else {
        log::debug!(
            "Slave mode: no station found at index {} (total stations: {})",
            current_index,
            stations.len()
        );
    }

    // Process incoming responses with 3-second timeout logic
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

                // Parse response and update storage (do this outside the port write lock)
                let request_arc_opt =
                    with_port_read(port_arc, |port| port.last_modbus_request.clone())
                        .and_then(|x| x); // Flatten Option<Option<...>> to Option<...>

                // Get storage and current station info from global mode
                let (storage_opt, station_info_opt) = match global_mode {
                    types::modbus::ModbusConnectionMode::Slave { storage, .. } => {
                        // Get the station info for the current request
                        let station_opt = with_port_read(port_arc, |port| {
                            let types::port::PortConfig::Modbus { stations, .. } = &port.config;
                            // Find station that has a pending request (last_request_time is set)
                            stations
                                .iter()
                                .find(|s| s.last_request_time.is_some())
                                .map(|s| (s.register_mode, s.register_address, s.register_length))
                        })
                        .and_then(|x| x);
                        (Some(storage.clone()), station_opt)
                    }
                    _ => (None, None),
                };

                // Parse and update storage if available
                if let (
                    Some(storage),
                    Some(request_arc),
                    Some((register_mode, start_address, length)),
                ) = (storage_opt, request_arc_opt, station_info_opt)
                {
                    if let Ok(mut request) = request_arc.lock() {
                        if request.parse_ok(&frame).is_ok() {
                            // Successfully parsed response - now update storage with the values
                            if let Ok(mut context) = storage.lock() {
                                match register_mode {
                                    types::modbus::RegisterMode::Holding => {
                                        // Parse holding register values from response
                                        if let Ok(values) =
                                            crate::protocol::modbus::parse_pull_get_holdings(
                                                &mut request,
                                                frame.to_vec(),
                                            )
                                        {
                                            // Write values to storage
                                            for (offset, &value) in values.iter().enumerate() {
                                                let addr =
                                                    start_address.wrapping_add(offset as u16);
                                                match context.set_holding(addr, value) {
                                                    Ok(_) => {
                                                        log::debug!(
                                                            "✓ Set holding register at addr {addr} (0x{addr:04X}) = {value} (0x{value:04X})"
                                                        );
                                                    }
                                                    Err(e) => {
                                                        log::warn!(
                                                            "Failed to set holding register at {addr}: {e}"
                                                        );
                                                    }
                                                }
                                            }
                                            log::info!(
                                                "📥 Slave updated {} holding registers starting at address {start_address} (0x{start_address:04X}): {:?}",
                                                values.len(),
                                                values
                                            );
                                            
                                            // Verify the values were written correctly by reading them back
                                            log::debug!("🔍 Verifying written values:");
                                            for (offset, &expected_value) in values.iter().enumerate() {
                                                let addr = start_address.wrapping_add(offset as u16);
                                                if let Ok(actual_value) = context.get_holding(addr) {
                                                    if actual_value == expected_value {
                                                        log::debug!("  ✓ Addr {addr}: {actual_value} (correct)");
                                                    } else {
                                                        log::warn!("  ✗ Addr {addr}: {actual_value} (expected {expected_value})");
                                                    }
                                                } else {
                                                    log::warn!("  ✗ Addr {addr}: Failed to read back");
                                                }
                                            }
                                        } else {
                                            log::warn!("Failed to parse holding register response");
                                        }
                                    }
                                    types::modbus::RegisterMode::Input => {
                                        // Parse input register values from response
                                        if let Ok(values) =
                                            crate::protocol::modbus::parse_pull_get_inputs(
                                                &mut request,
                                                frame.to_vec(),
                                            )
                                        {
                                            // Write values to storage
                                            for (offset, &value) in values.iter().enumerate() {
                                                let addr =
                                                    start_address.wrapping_add(offset as u16);
                                                if let Err(e) = context.set_input(addr, value) {
                                                    log::warn!(
                                                        "Failed to set input register at {addr}: {e}"
                                                    );
                                                }
                                            }
                                            log::info!(
                                                "Updated {} input registers starting at address {start_address}: {:?}",
                                                values.len(),
                                                values
                                            );
                                        }
                                    }
                                    types::modbus::RegisterMode::Coils => {
                                        // Parse coil values from response
                                        if let Ok(values) =
                                            crate::protocol::modbus::parse_pull_get_coils(
                                                &mut request,
                                                frame.to_vec(),
                                                length,
                                            )
                                        {
                                            // Write values to storage
                                            for (offset, &value) in values.iter().enumerate() {
                                                let addr =
                                                    start_address.wrapping_add(offset as u16);
                                                if let Err(e) = context.set_coil(addr, value) {
                                                    log::warn!("Failed to set coil at {addr}: {e}");
                                                }
                                            }
                                            log::info!(
                                                "Updated {} coils starting at address {start_address}",
                                                values.len()
                                            );
                                        }
                                    }
                                    types::modbus::RegisterMode::DiscreteInputs => {
                                        // Parse discrete input values from response
                                        if let Ok(values) =
                                            crate::protocol::modbus::parse_pull_get_discrete_inputs(
                                                &mut request,
                                                frame.to_vec(),
                                                length,
                                            )
                                        {
                                            // Write values to storage
                                            for (offset, &value) in values.iter().enumerate() {
                                                let addr =
                                                    start_address.wrapping_add(offset as u16);
                                                if let Err(e) = context.set_discrete(addr, value) {
                                                    log::warn!(
                                                        "Failed to set discrete input at {addr}: {e}"
                                                    );
                                                }
                                            }
                                            log::info!(
                                                "Updated {} discrete inputs starting at address {start_address}",
                                                values.len()
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                with_port_write(port_arc, |port| {
                    port.logs.push(log_entry);
                    if port.logs.len() > 1000 {
                        let excess = port.logs.len() - 1000;
                        port.logs.drain(0..excess);
                    }

                    // Find matching station and update success counter for valid responses within timeout
                    let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
                    for station in stations.iter_mut() {
                        if let Some(last_request_time) = station.last_request_time {
                            // Check if response is within 3-second timeout
                            if now.duration_since(last_request_time) <= Duration::from_secs(3) {
                                station.req_success = station.req_success.saturating_add(1);
                                station.last_request_time = None; // Clear timeout tracking
                                break;
                            }
                        }
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

    // Check for timeouts and log failed requests
    let mut logs_to_add: Vec<PortLogEntry> = Vec::new();
    with_port_write(port_arc, |port| {
        let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
        for station in stations.iter_mut() {
            if let Some(last_request_time) = station.last_request_time {
                // Check if request has timed out (3 seconds)
                if now.duration_since(last_request_time) > Duration::from_secs(3) {
                    // Prepare timeout log entry to add later
                    let log_entry = PortLogEntry {
                        when: chrono::Local::now(),
                        raw: format!(
                            "Slave Request Timeout: Station {} (3s timeout exceeded)",
                            station.station_id
                        ),
                        parsed: None,
                    };

                    logs_to_add.push(log_entry);

                    station.last_request_time = None; // Clear timeout tracking
                    log::warn!(
                        "Request timeout for {port_name} station {}",
                        station.station_id
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

    Ok(())
}

/// Generate a modbus request for polling and cache the ModbusRequest object
pub fn generate_modbus_request_with_cache(
    station: &types::modbus::ModbusRegisterItem,
    port_arc: &Arc<RwLock<types::port::PortData>>,
) -> Result<Vec<u8>> {
    let length = station.register_length.min(125); // Limit to max modbus length
    let address = station.register_address;
    let slave_id = station.station_id;

    let (modbus_request, raw) = match station.register_mode {
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

    // Cache the ModbusRequest object in PortData
    with_port_write(port_arc, |port| {
        port.last_modbus_request = Some(Arc::new(Mutex::new(modbus_request)));
    });

    Ok(raw)
}

/// Generate a modbus master response to an incoming request (same logic as slave response)
pub fn generate_modbus_master_response(
    request: &[u8],
    stations: &[types::modbus::ModbusRegisterItem],
    global_mode: &types::modbus::ModbusConnectionMode,
) -> Result<Vec<u8>> {
    if request.len() < 2 {
        return Err(anyhow!("Request too short"));
    }

    let slave_id = request[0];

    // Find a station configuration that matches the slave ID
    let _station = stations
        .iter()
        .find(|s| s.station_id == slave_id)
        .ok_or_else(|| anyhow!("No station configured for slave ID {}", slave_id))?;

    // Use the storage from the global mode instead of creating a new one
    let storage = match global_mode {
        types::modbus::ModbusConnectionMode::Master { storage } => storage.clone(),
        _ => return Err(anyhow!("Invalid mode for master response generation")),
    };

    let mut context = storage
        .lock()
        .map_err(|e| anyhow!("Failed to lock storage: {}", e))?;

    let mut response = Vec::new();
    let mut frame = ModbusFrame::new(slave_id, request, ModbusProto::Rtu, &mut response);
    frame.parse()?;

    // Use the existing modbus helper functions to build responses
    match frame.func {
        rmodbus::consts::ModbusFunction::GetCoils => {
            if let Ok(Some(ret)) = build_slave_coils_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow!("Failed to build coils response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetDiscretes => {
            if let Ok(Some(ret)) = build_slave_discrete_inputs_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow!("Failed to build discrete inputs response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetHoldings => {
            if let Ok(Some(ret)) = build_slave_holdings_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow!("Failed to build holdings response"))
            }
        }
        rmodbus::consts::ModbusFunction::GetInputs => {
            if let Ok(Some(ret)) = build_slave_inputs_response(&mut frame, &mut context) {
                Ok(ret)
            } else {
                Err(anyhow!("Failed to build inputs response"))
            }
        }
        _ => Err(anyhow!(
            "Unsupported modbus function code: {:?}",
            frame.func
        )),
    }
}
