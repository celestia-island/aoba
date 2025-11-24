/// Core Modbus communication logic - Pure functions without channel dependencies
///
/// This module provides the fundamental Modbus protocol operations:
/// - Slave: listen for requests and generate responses
/// - Master: send requests and parse responses
///
/// These functions are pure and don't depend on specific communication channels.
#[allow(clippy::manual_div_ceil)]
use anyhow::{anyhow, Result};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use crate::protocol::status::types::modbus::{ModbusResponse, RegisterMode, ResponseRegisterMode};

/// Listen for one Modbus request and respond (Slave/Server logic)
///
/// This is a pure function that handles the Modbus protocol without any channel dependencies.
/// It reads from the port, processes the request, sends a response, and returns the data.
pub fn slave_process_one_request(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    _reg_mode: RegisterMode,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
) -> Result<ModbusResponse> {
    let residual_buffer = Arc::new(Mutex::new(Vec::new()));
    let params = SlaveRequestParams {
        port_arc,
        station_id,
        register_address,
        register_length,
        reg_mode: _reg_mode,
        storage,
        hooks: &[],
        port_name: "".to_string(),
        residual_buffer,
    };
    slave_process_one_request_with_hooks(&params)
}

/// Slave request processing parameters
pub struct SlaveRequestParams<'a> {
    pub port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    pub station_id: u8,
    pub register_address: u16,
    pub register_length: u16,
    pub reg_mode: RegisterMode,
    pub storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    pub hooks: &'a [Arc<dyn super::ModbusHook>],
    pub port_name: String,
    pub residual_buffer: Arc<Mutex<Vec<u8>>>,
}

/// Process one slave request with hook support, sliding window parsing, and frame reassembly
pub fn slave_process_one_request_with_hooks(
    params: &SlaveRequestParams<'_>,
) -> Result<ModbusResponse> {
    use rmodbus::{server::ModbusFrame, ModbusProto};

    let mut residual = params.residual_buffer.lock().unwrap();
    let residual_len = residual.len();

    // Read request from port with retry for complete frame
    // Modbus RTU frames are typically 8+ bytes, but may arrive in fragments
    let mut buffer = vec![0u8; 256];
    let mut total_bytes = 0;

    if residual_len > 0 {
        let copy_len = residual_len.min(buffer.len());
        buffer[..copy_len].copy_from_slice(&residual[..copy_len]);
        total_bytes = copy_len;
        residual.clear();
    }
    drop(residual);

    let mut port = params.port_arc.lock().unwrap();

    // First read - get initial data
    let bytes_read = port.read(&mut buffer[total_bytes..])?;
    if bytes_read == 0 && total_bytes == 0 {
        drop(port);
        return Err(anyhow!("No data received"));
    }
    total_bytes += bytes_read;

    // Wait a bit for remaining data (Modbus RTU inter-frame delay)
    // At 57600 baud, 8 bytes takes ~1.4ms, give it up to 10ms
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Try to read any remaining bytes
    if let Ok(additional) = port.read(&mut buffer[total_bytes..]) {
        total_bytes += additional;
    }

    drop(port);

    let original_data = buffer[..total_bytes].to_vec();

    // Pre-check: avoid the slave parsing response frames (echoes of our own sent frames)
    // A slave should only handle request frames: function codes 0x01/0x03/0x0F/0x10 are request types
    // Response frames have the same function codes but different frame structure; we can quickly filter by frame length
    if total_bytes >= 2 {
        let _func_code = original_data[1];
        // Response frames are usually longer (>15 bytes) and contain larger payloads
        // Request frames are usually shorter (8-13 bytes)
        // Lower threshold to 20 bytes to more aggressively filter short response frames
        if total_bytes > 20 {
            // Clear buffer; do not attempt parsing
            let mut residual = params.residual_buffer.lock().unwrap();
            residual.clear();
            return Err(anyhow!(
                "Skipped response frame echo ({} bytes)",
                total_bytes
            ));
        }
    }

    // Sliding-window parsing: try parsing starting from each possible offset
    // This handles cases where data may not start at buffer[0]
    let mut last_parse_error: Option<anyhow::Error> = None;

    for start_offset in 0..total_bytes {
        // Minimum Modbus RTU frame is 8 bytes (addr 1 + func 1 + data N>=2 + CRC 2)
        if total_bytes - start_offset < 8 {
            break;
        }

        let request_data = original_data[start_offset..].to_vec();

        // Apply hooks for each candidate frame
        let mut hooked_data = request_data.clone();
        for hook in params.hooks {
            if let Err(_e) = hook.on_after_receive_request(&params.port_name, &mut hooked_data) {}
        }

        // Try parsing this candidate frame
        let mut response = Vec::new();
        let mut frame = ModbusFrame::new(
            params.station_id,
            &hooked_data,
            ModbusProto::Rtu,
            &mut response,
        );

        match frame.parse() {
            Ok(()) => {
                // Parse succeeded: found a valid frame

                // Save any remaining unused data into the residual buffer
                // frame.len is the parsed frame length (if rmodbus provides it)
                // Otherwise we must estimate: function code determines length
                // For a full frame, finding the CRC allows exact length calculation
                // Simplification: assume frame starts at start_offset and consumes the valid data
                // A more precise approach computes size using function code and data length

                // Compute actual bytes consumed (addr + func + length bytes + data + CRC)
                // For write requests 0x0F/0x10, format is: addr(1) + func(1) + start addr(2) + quantity(2) + byte count(1) + data(N) + CRC(2)
                // Simplify: skip complex calculation and clear residuals (we process one request at a time)
                let remaining_start = start_offset + hooked_data.len();
                if remaining_start < total_bytes {
                    let remaining = original_data[remaining_start..].to_vec();
                    if !remaining.is_empty() {
                        let mut residual = params.residual_buffer.lock().unwrap();
                        *residual = remaining.clone();
                    }
                } else {
                    // No remaining data; ensure residual buffer is empty
                    let mut residual = params.residual_buffer.lock().unwrap();
                    residual.clear();
                }

                // Continue processing this valid frame (rest of the code unchanged)
                let actual_mode = match frame.func {
                    rmodbus::consts::ModbusFunction::GetHoldings => RegisterMode::Holding,
                    rmodbus::consts::ModbusFunction::GetInputs => RegisterMode::Input,
                    rmodbus::consts::ModbusFunction::GetCoils => RegisterMode::Coils,
                    rmodbus::consts::ModbusFunction::GetDiscretes => RegisterMode::DiscreteInputs,
                    rmodbus::consts::ModbusFunction::SetCoilsBulk => RegisterMode::Coils,
                    rmodbus::consts::ModbusFunction::SetHoldingsBulk => RegisterMode::Holding,
                    _ => {
                        return Err(anyhow!(
                            "Unsupported function code: 0x{:02X} ({:?})",
                            frame.func as u8,
                            frame.func
                        ));
                    }
                };

                // Parse the actual starting address and length from the request frame
                // Modbus RTU request format: [addr(1), func(1), addr(2), quantity(2), CRC(2)]
                let actual_address = if hooked_data.len() >= 6 {
                    u16::from_be_bytes([hooked_data[2], hooked_data[3]])
                } else {
                    params.register_address // fallback to default value
                };

                let actual_length = if hooked_data.len() >= 6 {
                    u16::from_be_bytes([hooked_data[4], hooked_data[5]])
                } else {
                    params.register_length // fallback to default value
                };

                // Generate response (code continues)
                let response_bytes = match actual_mode {
                    RegisterMode::Holding => {
                        crate::protocol::modbus::build_slave_holdings_response(
                            &mut frame,
                            &mut params.storage.lock().unwrap(),
                        )?
                    }
                    RegisterMode::Input => crate::protocol::modbus::build_slave_inputs_response(
                        &mut frame,
                        &mut params.storage.lock().unwrap(),
                    )?,
                    RegisterMode::Coils => crate::protocol::modbus::build_slave_coils_response(
                        &mut frame,
                        &mut params.storage.lock().unwrap(),
                    )?,
                    RegisterMode::DiscreteInputs => {
                        crate::protocol::modbus::build_slave_discrete_inputs_response(
                            &mut frame,
                            &mut params.storage.lock().unwrap(),
                        )?
                    }
                };

                if let Some(resp) = response_bytes {
                    let mut port = params.port_arc.lock().unwrap();
                    port.write_all(&resp)?;
                    port.flush()?;
                }

                // Extract values and return (using actual request address and length)
                let values = extract_values_from_storage(
                    &params.storage,
                    actual_address,
                    actual_length,
                    actual_mode,
                )?;

                return Ok(ModbusResponse {
                    station_id: params.station_id,
                    register_address: actual_address,
                    register_mode: match actual_mode {
                        RegisterMode::Coils => crate::protocol::status::types::modbus::ResponseRegisterMode::Coils,
                        RegisterMode::DiscreteInputs => crate::protocol::status::types::modbus::ResponseRegisterMode::DiscreteInputs,
                        RegisterMode::Holding => crate::protocol::status::types::modbus::ResponseRegisterMode::Holding,
                        RegisterMode::Input => crate::protocol::status::types::modbus::ResponseRegisterMode::Input,
                    },
                    values,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            }
            Err(e) => {
                // Parse failed at this position; try the next offset
                last_parse_error = Some(anyhow!("{:?}", e));
                continue;
            }
        }
    }

    // Sliding-window traversal complete: no valid frame found
    log::warn!(
        "No valid Modbus frame found in {} bytes (tried {} offsets)",
        total_bytes,
        total_bytes
    );

    // Improvement strategy: clear the residual buffer on parse failures
    // Reason: if data cannot be parsed, residuals accumulate and worsen the issue
    // After clearing, wait (by caller control) to allow buffer stabilization
    {
        let mut residual = params.residual_buffer.lock().unwrap();
        if !residual.is_empty() {
            log::warn!(
                "Clearing {} residual bytes (parse failed too many times)",
                residual.len()
            );
            residual.clear();
        }
    }

    // Return a special marker error to tell the caller to pause polling
    if let Some(err) = last_parse_error {
        Err(anyhow!("PARSE_FAILED: {}", err))
    } else {
        Err(anyhow!(
            "PARSE_FAILED: No valid frame found in {} bytes",
            total_bytes
        ))
    }
}

/// Parse response values from raw bytes
fn parse_response_values(
    response: &[u8],
    register_length: u16,
    reg_mode: RegisterMode,
) -> Result<Vec<u16>> {
    match reg_mode {
        RegisterMode::Holding | RegisterMode::Input => {
            if response.len() < 5 {
                return Err(anyhow!(
                    "Response too short (len={}, need >=5)",
                    response.len()
                ));
            }
            let byte_count = response[2] as usize;
            let mut values = Vec::new();
            for i in 0..(byte_count / 2) {
                let offset = 3 + i * 2;
                if offset + 1 < response.len() {
                    let value = u16::from_be_bytes([response[offset], response[offset + 1]]);
                    values.push(value);
                } else {
                    log::warn!(
                        "Incomplete register at offset {}: response too short",
                        offset
                    );
                }
            }
            Ok(values)
        }
        RegisterMode::Coils | RegisterMode::DiscreteInputs => {
            if response.len() < 5 {
                return Err(anyhow!("Response too short"));
            }
            let byte_count = response[2] as usize;
            let mut values = Vec::new();
            for byte_idx in 0..byte_count {
                if 3 + byte_idx < response.len() {
                    let byte_val = response[3 + byte_idx];
                    for bit_idx in 0..8 {
                        if values.len() >= register_length as usize {
                            break;
                        }
                        let bit_value = if (byte_val & (1 << bit_idx)) != 0 {
                            1
                        } else {
                            0
                        };
                        values.push(bit_value);
                    }
                }
            }
            values.truncate(register_length as usize);
            Ok(values)
        }
    }
}

/// Write coils to Modbus slave (Master/Client logic for writing)
///
/// This function sends a write command to the slave and waits for confirmation.
/// It supports writing multiple coils using function code 0x0F (Write Multiple Coils).
///
/// # Parameters
///
/// * `port_arc` - Shared serial port handle
/// * `station_id` - Target slave station ID
/// * `start_address` - Starting coil address
/// * `coil_values` - Boolean values to write (true = ON, false = OFF)
///
/// # Returns
///
/// Returns the raw request frame that was sent (before any hook transformations)
pub fn master_write_coils(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    start_address: u16,
    coil_values: &[bool],
) -> Result<Vec<u8>> {
    // Generate write request
    let mut request =
        crate::protocol::modbus::generate_pull_set_coils_request(station_id, coil_values.to_vec())?;

    let mut request_frame = Vec::new();
    request.generate_set_coils_bulk(start_address, coil_values, &mut request_frame)?;

    // Send request
    {
        let mut port = port_arc.lock().unwrap();
        port.write_all(&request_frame)?;
        port.flush()?;
    }

    // Read confirmation response
    let mut buffer = vec![0u8; 256];
    let bytes_read = {
        let mut port = port_arc.lock().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50)); // Wait for slave to process
        port.read(&mut buffer)?
    };

    if bytes_read < 8 {
        return Err(anyhow!(
            "Invalid write response (too short: {} bytes)",
            bytes_read
        ));
    }

    let response = &buffer[..bytes_read];

    // Verify response (function code should match, or be error code)
    if response[1] == 0x0F {
        log::info!(
            "Successfully wrote {} coils at address 0x{:04X}",
            coil_values.len(),
            start_address
        );
    } else if response[1] & 0x80 != 0 {
        let error_code = response[2];
        return Err(anyhow!("Modbus exception: error code 0x{:02X}", error_code));
    }

    Ok(request_frame)
}

fn extract_values_from_storage(
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    start_addr: u16,
    length: u16,
    reg_mode: RegisterMode,
) -> Result<Vec<u16>> {
    use rmodbus::server::context::ModbusContext;

    let storage = storage.lock().unwrap();
    let mut values = Vec::new();

    for i in 0..length {
        let addr = start_addr + i;
        let value = match reg_mode {
            RegisterMode::Holding => storage.get_holding(addr)?,
            RegisterMode::Input => storage.get_input(addr)?,
            RegisterMode::Coils => {
                if storage.get_coil(addr)? {
                    1
                } else {
                    0
                }
            }
            RegisterMode::DiscreteInputs => {
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

/// Internal helper: Execute a single poll (for backward compatibility with deprecated APIs)
///
/// This is used internally by deprecated master loop implementations.
/// New code should use `master_poll_once_and_stop` or `master_poll_loop` directly.
pub(super) fn execute_single_poll_internal(
    port_arc: &Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: RegisterMode,
) -> Result<ModbusResponse> {
    // Generate request
    let request_bytes = match reg_mode {
        RegisterMode::Holding => crate::protocol::modbus::generate_pull_get_holdings_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::Input => crate::protocol::modbus::generate_pull_get_inputs_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::Coils => crate::protocol::modbus::generate_pull_get_coils_request(
            station_id,
            register_address,
            register_length,
        )?,
        RegisterMode::DiscreteInputs => {
            crate::protocol::modbus::generate_pull_get_discrete_inputs_request(
                station_id,
                register_address,
                register_length,
            )?
        }
    };

    let request_frame = request_bytes.1;

    // Send request
    {
        let mut port = port_arc.lock().unwrap();
        port.write_all(&request_frame)?;
        port.flush()?;
    }

    // Calculate expected response length
    let expected_data_bytes = match reg_mode {
        RegisterMode::Holding | RegisterMode::Input => register_length as usize * 2,
        RegisterMode::Coils | RegisterMode::DiscreteInputs => (register_length as usize + 7) / 8,
    };
    let expected_frame_length = 3 + expected_data_bytes + 2;

    // Read response
    let mut buffer = vec![0u8; 256];
    let mut total_bytes = 0;

    {
        let mut port = port_arc.lock().unwrap();
        let bytes_read = port.read(&mut buffer[total_bytes..])?;
        if bytes_read == 0 {
            return Err(anyhow!("No response received"));
        }
        total_bytes += bytes_read;
    }

    if total_bytes < expected_frame_length {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut port = port_arc.lock().unwrap();
        let remaining_needed = expected_frame_length - total_bytes;
        if let Ok(additional) =
            port.read(&mut buffer[total_bytes..total_bytes + remaining_needed + 10])
        {
            if additional > 0 {
                total_bytes += additional;
            }
        }
    }

    let response = &buffer[..total_bytes];

    // Validate
    if total_bytes < 5 {
        return Err(anyhow!("Response too short: {} bytes", total_bytes));
    }

    if response[0] != station_id {
        return Err(anyhow!(
            "Station ID mismatch: expected {}, got {}",
            station_id,
            response[0]
        ));
    }

    let expected_func = match reg_mode {
        RegisterMode::Holding => 0x03,
        RegisterMode::Input => 0x04,
        RegisterMode::Coils => 0x01,
        RegisterMode::DiscreteInputs => 0x02,
    };

    if response[1] != expected_func {
        let mut flush_buffer = vec![0u8; 256];
        let mut port = port_arc.lock().unwrap();
        if let Ok(n) = port.read(&mut flush_buffer) {
            if n > 0 {
                log::warn!("Flushed {} residual bytes", n);
            }
        }
        return Err(anyhow!(
            "Function code mismatch: expected {:?} (0x{:02X}), got 0x{:02X}",
            reg_mode,
            expected_func,
            response[1]
        ));
    }

    let byte_count = response[2] as usize;
    let expected_byte_count = match reg_mode {
        RegisterMode::Holding | RegisterMode::Input => register_length as usize * 2,
        RegisterMode::Coils | RegisterMode::DiscreteInputs => (register_length as usize + 7) / 8,
    };

    if byte_count != expected_byte_count {
        return Err(anyhow!(
            "Byte count mismatch: expected {}, got {}",
            expected_byte_count,
            byte_count
        ));
    }

    let values = parse_response_values(response, register_length, reg_mode)?;

    if values.len() != register_length as usize {
        return Err(anyhow!(
            "Value count mismatch: expected {}, got {}",
            register_length,
            values.len()
        ));
    }

    Ok(ModbusResponse {
        station_id,
        register_address,
        register_mode: ResponseRegisterMode::from_register_mode(reg_mode),
        values,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Long-running master poll loop with channel communication (Unified Architecture)
///
/// This function holds the serial port connection and continuously polls the slave.
/// It sends responses via a channel and can receive control messages.
///
/// # Architecture Benefits
///
/// - **Efficient**: Port connection held throughout, minimal lock contention
/// - **Decoupled**: Communicates via channels, no direct dependencies
/// - **Controllable**: Can receive stop/config commands via control channel
/// - **Unified**: Single implementation for all polling scenarios (continuous or one-shot)
///
/// # Parameters
///
/// * `port_arc` - Shared serial port handle
/// * `station_id` - Target slave station ID
/// * `register_address` - Starting register address
/// * `register_length` - Number of registers to read
/// * `reg_mode` - Register type (Coils/Holding/Input/DiscreteInputs)
/// * `response_tx` - Channel to send successful poll responses
/// * `control_rx` - Optional channel to receive control commands (None = run forever)
/// * `poll_interval_ms` - Delay between polls in milliseconds
///
/// # Control Messages
///
/// If `control_rx` is provided, the loop can receive:
/// - `"stop"` - Gracefully exit the loop
/// - `"pause"` - Temporarily pause polling (future enhancement)
///
/// # Example (Continuous Polling)
///
/// ```rust,no_run
/// use flume::unbounded;
/// use std::sync::{Arc, Mutex};
///
/// let (response_tx, response_rx) = unbounded();
/// let (control_tx, control_rx) = unbounded();
///
/// let port = /* open serial port */;
/// let port_arc = Arc::new(Mutex::new(port));
///
/// // Spawn continuous polling
/// std::thread::spawn(move || {
///     master_poll_loop(
///         port_arc,
///         1, 0x00, 10,
///         RegisterMode::Holding,
///         response_tx,
///         Some(control_rx),
///         1000, // poll every 1 second
///     ).unwrap();
/// });
///
/// // Receive responses
/// while let Ok(response) = response_rx.recv() {
///     println!("Got response: {:?}", response);
/// }
///
/// // Stop gracefully
/// control_tx.send("stop".to_string()).unwrap();
/// ```
///
/// # Example (One-Shot Request)
///
/// For CLI or single-request scenarios, use `master_poll_once_and_stop` wrapper.
/// Master polling configuration parameters
pub struct MasterPollParams {
    pub port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    pub station_id: u8,
    pub register_address: u16,
    pub register_length: u16,
    pub reg_mode: RegisterMode,
    pub response_tx: flume::Sender<ModbusResponse>,
    pub control_rx: Option<flume::Receiver<String>>,
    pub poll_interval_ms: u64,
}

pub fn master_poll_loop(params: &MasterPollParams) -> Result<()> {
    log::info!(
        "Starting master poll loop: station={}, addr=0x{:04X}, len={}, mode={:?}",
        params.station_id,
        params.register_address,
        params.register_length,
        params.reg_mode
    );

    let poll_interval = std::time::Duration::from_millis(params.poll_interval_ms);
    let mut consecutive_errors = 0u32;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;

    loop {
        // Check for control messages (non-blocking)
        if let Some(ref control) = params.control_rx {
            if let Ok(cmd) = control.try_recv() {
                match cmd.as_str() {
                    "stop" => {
                        log::info!("🛑 Master poll loop received stop command, exiting gracefully");
                        break;
                    }
                    "pause" => {
                        log::info!("Master poll loop paused (not implemented yet)");
                        // Future: implement pause/resume logic
                    }
                    _ => {
                        log::warn!("Unknown control command: {}", cmd);
                    }
                }
            }
        }

        // ==================== Execute one poll (inline logic) ====================

        // Generate request frame
        let request_bytes = match params.reg_mode {
            RegisterMode::Holding => crate::protocol::modbus::generate_pull_get_holdings_request(
                params.station_id,
                params.register_address,
                params.register_length,
            ),
            RegisterMode::Input => crate::protocol::modbus::generate_pull_get_inputs_request(
                params.station_id,
                params.register_address,
                params.register_length,
            ),
            RegisterMode::Coils => crate::protocol::modbus::generate_pull_get_coils_request(
                params.station_id,
                params.register_address,
                params.register_length,
            ),
            RegisterMode::DiscreteInputs => {
                crate::protocol::modbus::generate_pull_get_discrete_inputs_request(
                    params.station_id,
                    params.register_address,
                    params.register_length,
                )
            }
        };

        let poll_result: Result<ModbusResponse> = match request_bytes {
            Ok(bytes) => {
                let request_frame = bytes.1;

                // Send request
                if let Err(e) = (|| -> Result<()> {
                    let mut port = params.port_arc.lock().unwrap();
                    port.write_all(&request_frame)?;
                    port.flush()?;
                    Ok(())
                })() {
                    Err(anyhow!("Failed to send request: {}", e))
                } else {
                    // Calculate expected response length
                    let expected_data_bytes = match params.reg_mode {
                        RegisterMode::Holding | RegisterMode::Input => {
                            params.register_length as usize * 2
                        }
                        RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                            (params.register_length as usize + 7) / 8
                        }
                    };
                    let expected_frame_length = 3 + expected_data_bytes + 2;

                    // Read response
                    let mut buffer = vec![0u8; 256];
                    let mut total_bytes = 0;

                    // First read
                    {
                        let mut port = params.port_arc.lock().unwrap();
                        match port.read(&mut buffer[total_bytes..]) {
                            Ok(0) => Err(anyhow!("No response received")),
                            Ok(bytes_read) => {
                                total_bytes += bytes_read;
                                Ok(())
                            }
                            Err(e) => Err(anyhow!("Failed to read response: {}", e)),
                        }
                    }?;

                    // Second read if incomplete
                    if total_bytes < expected_frame_length {
                        std::thread::sleep(std::time::Duration::from_millis(20));

                        let mut port = params.port_arc.lock().unwrap();
                        let remaining_needed = expected_frame_length - total_bytes;
                        if let Ok(additional) =
                            port.read(&mut buffer[total_bytes..total_bytes + remaining_needed + 10])
                        {
                            if additional > 0 {
                                total_bytes += additional;
                            }
                        }
                    }

                    let response = &buffer[..total_bytes];

                    // Validate response
                    if total_bytes < 5 {
                        Err(anyhow!("Response too short: {} bytes", total_bytes))
                    } else if response[0] != params.station_id {
                        Err(anyhow!(
                            "Station ID mismatch: expected {}, got {}",
                            params.station_id,
                            response[0]
                        ))
                    } else {
                        // Verify function code
                        let expected_func = match params.reg_mode {
                            RegisterMode::Holding => 0x03,
                            RegisterMode::Input => 0x04,
                            RegisterMode::Coils => 0x01,
                            RegisterMode::DiscreteInputs => 0x02,
                        };

                        if response[1] != expected_func {
                            let actual_func = response[1];
                            log::warn!(
                                "Function code mismatch: expected 0x{:02X} ({:?}), got 0x{:02X}",
                                expected_func,
                                params.reg_mode,
                                actual_func
                            );

                            // Flush RX buffer
                            let mut flush_buffer = vec![0u8; 256];
                            let mut port = params.port_arc.lock().unwrap();
                            if let Ok(n) = port.read(&mut flush_buffer) {
                                if n > 0 {
                                    log::warn!("Flushed {} residual bytes", n);
                                }
                            }

                            Err(anyhow!(
                                "Function code mismatch: expected {:?} (0x{:02X}), got 0x{:02X}",
                                params.reg_mode,
                                expected_func,
                                actual_func
                            ))
                        } else {
                            // Verify byte count
                            let byte_count = response[2] as usize;
                            let expected_byte_count = match params.reg_mode {
                                RegisterMode::Holding | RegisterMode::Input => {
                                    params.register_length as usize * 2
                                }
                                RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                                    (params.register_length as usize + 7) / 8
                                }
                            };

                            if byte_count != expected_byte_count {
                                Err(anyhow!(
                                    "Byte count mismatch: expected {}, got {}",
                                    expected_byte_count,
                                    byte_count
                                ))
                            } else {
                                // Parse values
                                match parse_response_values(
                                    response,
                                    params.register_length,
                                    params.reg_mode,
                                ) {
                                    Ok(values) => {
                                        if values.len() != params.register_length as usize {
                                            Err(anyhow!(
                                                "Value count mismatch: expected {}, got {}",
                                                params.register_length,
                                                values.len()
                                            ))
                                        } else {
                                            Ok(ModbusResponse {
                                                station_id: params.station_id,
                                                register_address: params.register_address,
                                                register_mode:
                                                    ResponseRegisterMode::from_register_mode(
                                                        params.reg_mode,
                                                    ),
                                                values,
                                                timestamp: chrono::Utc::now().to_rfc3339(),
                                            })
                                        }
                                    }
                                    Err(e) => Err(anyhow!("Failed to parse values: {}", e)),
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => Err(anyhow!("Failed to generate request: {}", e)),
        };

        // ==================== End of inline poll logic ====================

        // Handle poll result
        match poll_result {
            Ok(response) => {
                consecutive_errors = 0;

                // Send response via channel (non-blocking)
                if let Err(e) = params.response_tx.try_send(response) {
                    log::warn!("Failed to send response to channel: {}", e);
                    if matches!(e, flume::TrySendError::Disconnected(_)) {
                        log::error!("Response channel disconnected, stopping poll loop");
                        break;
                    }
                }
            }
            Err(err) => {
                consecutive_errors += 1;
                log::warn!(
                    "Poll error (#{}/{}): {}",
                    consecutive_errors,
                    MAX_CONSECUTIVE_ERRORS,
                    err
                );

                // Recovery logic
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    log::error!(
                        "Too many consecutive errors ({}), entering recovery mode",
                        consecutive_errors
                    );
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    consecutive_errors = 0;
                }

                if err.to_string().contains("Function code mismatch") {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }

        // Poll interval delay
        std::thread::sleep(poll_interval);
    }

    log::info!("Master poll loop exited cleanly");
    Ok(())
}

/// Execute a single Modbus poll request and stop immediately (Convenience Wrapper)
///
/// This is a convenience function for CLI or one-shot scenarios. It:
/// 1. Starts the poll loop
/// 2. Waits for the first successful response
/// 3. Sends a stop command
/// 4. Returns the response
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::{Arc, Mutex};
///
/// let port = /* open serial port */;
/// let port_arc = Arc::new(Mutex::new(port));
///
/// let response = master_poll_once_and_stop(
///     port_arc,
///     1,      // station_id
///     0x00,   // register_address
///     10,     // register_length
///     RegisterMode::Holding,
///     5000,   // 5 second timeout
/// )?;
///
/// println!("Values: {:?}", response.values);
/// ```
pub fn master_poll_once_and_stop(
    port_arc: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    reg_mode: RegisterMode,
    timeout_ms: u64,
) -> Result<ModbusResponse> {
    let (response_tx, response_rx) = flume::unbounded();
    let (control_tx, control_rx) = flume::unbounded();

    // Spawn poll loop in background
    let port_clone = port_arc.clone();
    let handle = std::thread::spawn(move || {
        let params = MasterPollParams {
            port_arc: port_clone,
            station_id,
            register_address,
            register_length,
            reg_mode,
            response_tx,
            control_rx: Some(control_rx),
            poll_interval_ms: 100, // Fast polling for one-shot
        };
        master_poll_loop(&params)
    });

    // Wait for first response with timeout
    let result = response_rx.recv_timeout(std::time::Duration::from_millis(timeout_ms));

    // Send stop command regardless of success/failure
    let _ = control_tx.send("stop".to_string());

    // Wait for thread to exit
    let _ = handle.join();

    // Return result
    result.map_err(|e| anyhow!("Timeout or channel error: {}", e))
}
