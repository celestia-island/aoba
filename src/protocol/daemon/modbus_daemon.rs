use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

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
                        ports.push((port_name.clone(), port_arc.clone(), *mode, stations.clone()));
                    }
                }
            }
        }
        Ok(ports)
    })?;

    for (port_name, port_arc, global_mode, stations) in active_ports {
        // Process each port's modbus communication
        match global_mode {
            types::modbus::ModbusConnectionMode::Master => {
                handle_master_mode(&port_name, &port_arc, &stations, now)?;
            }
            types::modbus::ModbusConnectionMode::Slave => {
                handle_slave_mode(&port_name, &port_arc, &stations, now)?;
            }
        }
    }

    Ok(())
}

/// Handle master mode - passively listen and respond to incoming slave requests
pub fn handle_master_mode(
    port_name: &str,
    port_arc: &Arc<RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
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
                    raw: format!("Master RX: {}", hex_frame),
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
                if let Ok(response) = generate_modbus_master_response(&frame, stations) {
                    // Send the response
                    if let Err(e) =
                        runtime
                            .cmd_tx
                            .send(crate::protocol::runtime::RuntimeCommand::Write(
                                response.clone(),
                            ))
                    {
                        log::warn!("Failed to send modbus master response for {port_name}: {e}");
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
                        raw: format!("Master TX: {}", hex_response),
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
                    log::debug!("Could not generate response for modbus request: {hex_frame}");
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

/// Handle slave mode - periodically send requests and wait for responses with timeout
pub fn handle_slave_mode(
    port_name: &str,
    port_arc: &Arc<RwLock<types::port::PortData>>,
    stations: &[types::modbus::ModbusRegisterItem],
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

    // Process each station for slave polling (1 second interval by default)
    for (index, station) in stations.iter().enumerate() {
        if now >= station.next_poll_at {
            // Time to send a new request
            let request_result = generate_modbus_request_with_cache(station, port_arc);

            match request_result {
                Ok(request_bytes) => {
                    // Send the request
                    if let Err(e) =
                        runtime
                            .cmd_tx
                            .send(crate::protocol::runtime::RuntimeCommand::Write(
                                request_bytes.clone(),
                            ))
                    {
                        log::warn!(
                            "Failed to send modbus slave request for {port_name} station {}: {e}",
                            station.station_id
                        );
                        continue;
                    }

                    // Log the sent frame
                    let hex_frame = request_bytes
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let log_entry = PortLogEntry {
                        when: chrono::Local::now(),
                        raw: format!("Slave TX: {}", hex_frame),
                        parsed: None,
                    };

                    // Add log entry to port logs and update station state
                    // First update logs
                    with_port_write(port_arc, |port| {
                        port.logs.push(log_entry);
                        // Keep only the last 1000 log entries
                        if port.logs.len() > 1000 {
                            let excess = port.logs.len() - 1000;
                            port.logs.drain(0..excess);
                        }
                    });

                    // Then update station polling state separately to avoid overlapping mutable borrows
                    with_port_write(port_arc, |port| {
                        // Update station polling state (1 second interval by default)
                        let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
                        if let Some(station) = stations.get_mut(index) {
                            station.req_total = station.req_total.saturating_add(1);
                            station.next_poll_at = now + Duration::from_millis(1000); // 1 second interval
                            station.last_request_time = Some(now); // Track request time for timeout
                        }
                    });

                    log::info!(
                        "Sent modbus slave request for {port_name} station {}: {hex_frame}",
                        station.station_id
                    );
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

    // Process incoming responses with 3-second timeout logic
    while let Ok(event) = runtime.evt_rx.try_recv() {
        match event {
            crate::protocol::runtime::RuntimeEvent::FrameReceived(frame) => {
                let hex_frame = frame
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                // Find matching station and update success counter for valid responses within timeout
                let mut response_handled = false;
                with_port_write(port_arc, |port| {
                    let types::port::PortConfig::Modbus { stations, .. } = &mut port.config;
                    for station in stations.iter_mut() {
                        if let Some(last_request_time) = station.last_request_time {
                            // Check if response is within 3-second timeout
                            if now.duration_since(last_request_time) <= Duration::from_secs(3) {
                                station.req_success = station.req_success.saturating_add(1);
                                station.last_request_time = None; // Clear timeout tracking
                                response_handled = true;
                                break;
                            }
                        }
                    }
                });

                if response_handled {
                    log::info!("Received valid modbus slave response for {port_name}: {hex_frame}");
                } else {
                    log::debug!("Received modbus response outside timeout window for {port_name}: {hex_frame}");
                }
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
        port.last_modbus_request = Some(modbus_request);
    });

    Ok(raw)
}

/// Generate a modbus request for polling (legacy function for compatibility)
pub fn generate_modbus_request(station: &types::modbus::ModbusRegisterItem) -> Result<Vec<u8>> {
    let length = station.register_length.min(125); // Limit to max modbus length
    let address = station.register_address;
    let slave_id = station.station_id;

    match station.register_mode {
        types::modbus::RegisterMode::Coils => {
            let (_, raw) = generate_pull_get_coils_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::DiscreteInputs => {
            let (_, raw) = generate_pull_get_discrete_inputs_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::Holding => {
            let (_, raw) = generate_pull_get_holdings_request(slave_id, address, length)?;
            Ok(raw)
        }
        types::modbus::RegisterMode::Input => {
            let (_, raw) = generate_pull_get_inputs_request(slave_id, address, length)?;
            Ok(raw)
        }
    }
}

/// Generate a modbus master response to an incoming request (same logic as slave response)
pub fn generate_modbus_master_response(
    request: &[u8],
    stations: &[types::modbus::ModbusRegisterItem],
) -> Result<Vec<u8>> {
    use rmodbus::server::context::ModbusContext;
    use rmodbus::server::{storage::ModbusStorageSmall, ModbusFrame};
    use rmodbus::ModbusProto;

    if request.len() < 2 {
        return Err(anyhow!("Request too short"));
    }

    let slave_id = request[0];

    // Find a station configuration that matches the slave ID
    let _station = stations
        .iter()
        .find(|s| s.station_id == slave_id)
        .ok_or_else(|| anyhow!("No station configured for slave ID {}", slave_id))?;

    // Create a simple storage context with some default values
    let mut context = ModbusStorageSmall::new();

    // Set some example values for demonstration
    for i in 0..100 {
        let _ = context.set_coil(i, i % 2 == 0);
        let _ = context.set_discrete(i, i % 3 == 0);
        let _ = context.set_holding(i, i as u16 * 10);
        let _ = context.set_input(i, i as u16 * 20);
    }

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
