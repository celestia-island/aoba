use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    core::{self, master_poll_loop, MasterPollParams},
    traits::{ModbusDataSource, ModbusMasterHandler},
    ModbusHook, ModbusPortConfig,
};
use crate::{
    api::utils::open_serial_port,
    protocol::status::types::modbus::{ModbusResponse, RegisterMode},
};

/// Handle to a running Modbus master that polls a slave station
pub struct ModbusMaster {
    receiver: flume::Receiver<ModbusResponse>,
    control_sender: Option<flume::Sender<String>>,
    _handle: tokio::task::JoinHandle<Result<()>>,
    /// Shared port handle for manual operations
    port_arc: Option<Arc<Mutex<Box<dyn serialport::SerialPort>>>>,
    /// Station ID for manual operations
    station_id: u8,
}

impl ModbusMaster {
    /// Create and start a new Modbus master using efficient loop architecture
    ///
    /// **NEW**: Uses `master_poll_loop` which holds the port connection efficiently.
    /// This is the recommended way to create a long-running master.
    ///
    /// # Parameters
    ///
    /// - `config`: Port configuration (station ID, address, baud rate, etc.)
    /// - `poll_interval_ms`: Delay between polls in milliseconds
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use aoba::api::modbus::{ModbusMaster, ModbusPortConfig, RegisterMode};
    ///
    /// let config = ModbusPortConfig {
    ///     port_name: "/dev/ttyUSB0".to_string(),
    ///     baud_rate: 9600,
    ///     station_id: 1,
    ///     register_address: 0x00,
    ///     register_length: 10,
    ///     register_mode: RegisterMode::Holding,
    ///     timeout_ms: 1000,
    ///     error_recovery_delay_ms: 1000,
    /// };
    ///
    /// let master = ModbusMaster::new_simple(config, 1000)?;
    ///
    /// // Receive responses
    /// while let Some(response) = master.try_recv() {
    ///     println!("Values: {:?}", response.values);
    /// }
    /// ```
    pub fn new_simple(config: ModbusPortConfig, poll_interval_ms: u64) -> Result<Self> {
        let (response_tx, response_rx) = flume::unbounded();
        let (control_tx, control_rx) = flume::unbounded();

        let port_name = config.port_name.clone();
        let baud_rate = config.baud_rate;
        let timeout_ms = config.timeout_ms;
        let station_id = config.station_id;
        let register_address = config.register_address;
        let register_length = config.register_length;
        let register_mode = config.register_mode;

        // Spawn polling task
        let handle = tokio::task::spawn_blocking(move || {
            // Open port once
            let port_handle = crate::api::utils::open_serial_port(
                &port_name,
                baud_rate,
                std::time::Duration::from_millis(timeout_ms),
            )?;
            let port_arc = Arc::new(Mutex::new(port_handle));

            // Run loop
            let params = MasterPollParams {
                port_arc,
                station_id,
                register_address,
                register_length,
                reg_mode: register_mode,
                response_tx,
                control_rx: Some(control_rx),
                poll_interval_ms,
            };
            master_poll_loop(&params)
        });

        Ok(Self {
            receiver: response_rx,
            control_sender: Some(control_tx),
            _handle: handle,
            port_arc: None,
            station_id,
        })
    }

    /// Create a new Modbus master with manual control capability
    ///
    /// This variant provides access to the port handle for manual single-shot operations.
    /// Does NOT start automatic polling - use `poll_once` or `write_coils` methods.
    pub fn new_manual(config: ModbusPortConfig) -> Result<Self> {
        let (_response_tx, response_rx) = flume::unbounded();
        let (control_tx, control_rx) = flume::unbounded();

        let port_name = config.port_name.clone();
        let baud_rate = config.baud_rate;
        let timeout_ms = config.timeout_ms;
        let station_id = config.station_id;

        // Open port once
        let port_handle = crate::api::utils::open_serial_port(
            &port_name,
            baud_rate,
            std::time::Duration::from_millis(timeout_ms),
        )?;
        let port_arc = Arc::new(Mutex::new(port_handle));
        let port_arc_clone = Arc::clone(&port_arc);

        // Spawn a minimal background task (just keeps the handle alive)
        let handle = tokio::task::spawn_blocking(move || {
            // Wait for stop command
            while let Ok(cmd) = control_rx.recv() {
                if cmd == "stop" {
                    break;
                }
            }
            Ok(())
        });

        Ok(Self {
            receiver: response_rx,
            control_sender: Some(control_tx),
            _handle: handle,
            port_arc: Some(port_arc_clone),
            station_id,
        })
    }

    /// Execute a single poll operation (manual mode only)
    ///
    /// Returns the response immediately without going through the receiver channel.
    pub fn poll_once(
        &self,
        register_mode: RegisterMode,
        address: u16,
        length: u16,
    ) -> Result<ModbusResponse> {
        let port_arc = self.port_arc.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Manual mode not available (created with automatic polling)")
        })?;

        core::execute_single_poll_internal(
            port_arc,
            self.station_id,
            address,
            length,
            register_mode,
        )
    }

    /// Write coils (01 function code) to the slave
    ///
    /// Returns Ok(()) if write was acknowledged successfully.
    ///
    /// **Note for 储氢罐 hardware**: The hardware requires byte-swapping for 11-coil writes.
    /// Apply `swap_coils_byte_order()` before calling this method if needed.
    pub fn write_coils(&self, address: u16, values: &[bool]) -> Result<()> {
        let port_arc = self.port_arc.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Manual mode not available (created with automatic polling)")
        })?;

        use crate::protocol::modbus::generate_pull_set_coils_request;
        use std::io::{Read, Write};

        // Generate write request
        let mut request = generate_pull_set_coils_request(self.station_id, values.to_vec())?;
        let mut frame = Vec::new();
        request.generate_set_coils_bulk(address, values, &mut frame)?;

        // Apply byte-swapping for 储氢罐 hardware (11 coils = 2 bytes)
        // Modbus frame: [station(1), func(1), addr(2), count(2), bytes(1), data(...), CRC(2)]
        if values.len() == 11 && frame.len() >= 9 && frame[6] == 2 {
            frame.swap(7, 8);
            log::debug!("🔄 Applied byte-swap for 11-coil write");
        }

        // Send request
        let mut port = port_arc.lock().unwrap();
        port.write_all(&frame)?;
        port.flush()?;

        // Read confirmation
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut buffer = vec![0u8; 256];
        let bytes_read = port.read(&mut buffer)?;

        if bytes_read < 8 {
            return Err(anyhow::anyhow!(
                "Incomplete write response: {} bytes",
                bytes_read
            ));
        }

        // Check response
        let response = &buffer[..bytes_read];
        if response[1] == 0x0F {
            // Success
            Ok(())
        } else if response[1] & 0x80 != 0 {
            // Exception
            Err(anyhow::anyhow!(
                "Modbus exception: error code 0x{:02X}",
                response[2]
            ))
        } else {
            Err(anyhow::anyhow!("Unexpected response"))
        }
    }

    /// Get a reference to the port handle for advanced operations (manual mode only)
    pub fn port_handle(&self) -> Option<&Arc<Mutex<Box<dyn serialport::SerialPort>>>> {
        self.port_arc.as_ref()
    }

    /// Try to receive without blocking (iterator-like interface)
    pub fn try_recv(&self) -> Option<ModbusResponse> {
        self.receiver.try_recv().ok()
    }

    /// Receive a response with timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Option<ModbusResponse> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Get the underlying receiver for advanced usage
    pub fn receiver(&self) -> &flume::Receiver<ModbusResponse> {
        &self.receiver
    }

    /// Send a control command to the master loop (if supported)
    ///
    /// Currently supported commands:
    /// - `"stop"` - Gracefully stop the polling loop
    /// - `"pause"` - Pause polling (not yet implemented)
    ///
    /// Returns `Ok(())` if command was sent, `Err` if no control channel exists (legacy API)
    pub fn send_control(&self, command: &str) -> Result<()> {
        if let Some(tx) = &self.control_sender {
            tx.send(command.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to send control command: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Control channel not available (created with legacy API)"
            ))
        }
    }

    /// Stop the master loop gracefully
    pub fn stop(&self) -> Result<()> {
        self.send_control("stop")
    }
}

/// Create and start a new Modbus master (legacy Builder API with middleware)
///
/// Accepts hooks and data sources as vectors for middleware pattern.
fn new_master_legacy(
    config: ModbusPortConfig,
    hooks: Vec<Arc<dyn ModbusHook>>,
    data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
) -> Result<ModbusMaster> {
    let (sender, receiver) = flume::unbounded();
    let station_id = config.station_id;

    let handle =
        tokio::spawn(async move { run_master_loop(config, hooks, data_sources, sender).await });

    Ok(ModbusMaster {
        receiver,
        control_sender: None,
        _handle: handle,
        port_arc: None,
        station_id,
    })
}

impl ModbusMaster {
    /// Create and start a multi-register Modbus master (new Builder API)
    ///
    /// Polls multiple register types on the same port with middleware support.
    pub fn new_multi_register(
        config: ModbusPortConfig,
        register_polls: Vec<super::RegisterPollConfig>,
        hooks: Vec<Arc<dyn ModbusHook>>,
        data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
    ) -> Result<Self> {
        let (sender, receiver) = flume::unbounded();
        let station_id = config.station_id;
        let handle = tokio::spawn(async move {
            run_multi_register_master_loop(config, register_polls, hooks, data_sources, sender)
                .await
        });

        Ok(Self {
            receiver,
            control_sender: None,
            _handle: handle,
            port_arc: None,
            station_id,
        })
    }

    /// Legacy constructor using hooks and data sources
    pub fn new(
        config: ModbusPortConfig,
        hooks: Vec<Arc<dyn ModbusHook>>,
        data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
    ) -> Result<Self> {
        new_master_legacy(config, hooks, data_sources)
    }
}

/// Generic master loop that uses a handler trait
///
/// This function is independent of communication channels.
/// It calls the handler's methods to process responses.
pub async fn run_master_loop_with_handler(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
    data_source: Option<Arc<Mutex<dyn ModbusDataSource>>>,
    handler: Arc<dyn ModbusMasterHandler>,
) -> Result<()> {
    log::info!(
        "Starting master loop with custom handler for {}",
        config.port_name
    );

    let port_handle = open_serial_port(
        &config.port_name,
        config.baud_rate,
        Duration::from_millis(config.timeout_ms),
    )?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        // Check if handler wants to continue
        if !handler.should_continue() {
            log::info!("Handler requested stop, exiting master loop");
            break;
        }

        if let Some(h) = &hooks {
            if let Err(e) = h.on_before_request(&config.port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        // Check if data source has new data to write
        if let Some(ds) = &data_source {
            match ds.lock().unwrap().next_data() {
                Ok(Some(_values)) => {
                    // TODO: Implement write operation based on register mode
                    // For now, we'll just log it
                }
                Ok(None) => {
                    // No data this cycle
                }
                Err(e) => {
                    log::warn!("Data source error: {}", e);
                }
            }
        }

        match core::execute_single_poll_internal(
            &port_arc,
            config.station_id,
            config.register_address,
            config.register_length,
            config.register_mode,
        ) {
            Ok(response) => {
                if let Some(h) = &hooks {
                    if let Err(e) = h.on_after_response(&config.port_name, &response) {
                        log::warn!("Hook on_after_response failed: {}", e);
                    }
                }

                // Use handler to process response
                if let Err(e) = handler.handle_response(&response) {
                    log::error!("Handler failed to process response: {}", e);
                    if let Some(h) = &hooks {
                        h.on_error(&config.port_name, &e);
                    }
                }
            }
            Err(err) => {
                // Determine retry policy from optional hook values
                let mut max_retries = 0u32;
                let mut retry_delay_ms = 500u64;
                if let Some(h) = &hooks {
                    if let Some(v) = h.hook_max_retries() {
                        max_retries = v;
                    }
                    if let Some(v) = h.hook_retry_delay_ms() {
                        retry_delay_ms = v;
                    }
                }

                if max_retries == 0 {
                    log::warn!("Error polling on {}: {}", config.port_name, err);
                    if let Some(h) = &hooks {
                        h.on_error(&config.port_name, &err);
                    }
                } else {
                    log::warn!(
                        "Error polling on {}: {} - retrying up to {} times",
                        config.port_name,
                        err,
                        max_retries
                    );
                    let mut last_err = err;
                    let mut success = false;
                    for attempt in 1..=max_retries {
                        // wait before retrying
                        tokio::time::sleep(std::time::Duration::from_millis(retry_delay_ms)).await;

                        // call before_request hooks again
                        if let Some(h) = &hooks {
                            if let Err(e) = h.on_before_request(&config.port_name) {
                                log::warn!("Hook on_before_request failed during retry: {}", e);
                            }
                        }

                        match core::execute_single_poll_internal(
                            &port_arc,
                            config.station_id,
                            config.register_address,
                            config.register_length,
                            config.register_mode,
                        ) {
                            Ok(response) => {
                                if let Some(h) = &hooks {
                                    if let Err(e) =
                                        h.on_after_response(&config.port_name, &response)
                                    {
                                        log::warn!("Hook on_after_response failed: {}", e);
                                    }
                                }
                                if let Err(e) = handler.handle_response(&response) {
                                    log::error!(
                                        "Handler failed to process response after retry: {}",
                                        e
                                    );
                                    if let Some(h) = &hooks {
                                        h.on_error(&config.port_name, &e);
                                    }
                                }
                                success = true;
                                break;
                            }
                            Err(e2) => {
                                last_err = e2;
                                if let Some(h) = &hooks {
                                    h.on_error(&config.port_name, &last_err);
                                }
                                log::warn!(
                                    "Retry {}/{} failed: {}",
                                    attempt,
                                    max_retries,
                                    last_err
                                );
                            }
                        }
                    }

                    if !success {
                        log::warn!(
                            "Retries exhausted for {}: last error: {}",
                            config.port_name,
                            last_err
                        );
                    }
                }
            }
        }

        // Poll interval
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
// New function implementation to be appended to the end of master.rs
// Middleware loop for the Builder API

/// Master loop - uses middleware chains for hooks and data sources (Builder API)
///
/// Process hooks and data sources using a middleware chain
async fn run_master_loop(
    config: ModbusPortConfig,
    hooks: Vec<Arc<dyn ModbusHook>>,
    mut data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
    sender: flume::Sender<ModbusResponse>,
) -> Result<()> {
    let ModbusPortConfig {
        port_name,
        baud_rate,
        station_id,
        register_address,
        register_length,
        register_mode,
        timeout_ms,
        error_recovery_delay_ms: _, // Master does not use this field
        poll_interval_ms: _, // Poll interval is hard-coded to 1 second for single-register mode
    } = config;

    log::info!("Starting master loop (middleware) for {}", port_name);
    // debug info removed

    let port_handle = open_serial_port(&port_name, baud_rate, Duration::from_millis(timeout_ms))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        // Execute hook chain: on_before_request
        for hook in &hooks {
            if let Err(e) = hook.on_before_request(&port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        // Try to get data from data source chain and perform write if data available
        if !data_sources.is_empty() {
            match super::traits::execute_data_source_chain(&mut data_sources) {
                Ok(Some(values)) => {
                    // debug info removed

                    // Execute write operation based on register mode
                    match register_mode {
                        RegisterMode::Coils => {
                            // Convert u16 values to bool for coils (0 = false, non-zero = true)
                            let coil_values: Vec<bool> = values.iter().map(|&v| v != 0).collect();

                            // Generate write request frame
                            let mut request_frame = Vec::new();
                            match crate::protocol::modbus::generate_pull_set_coils_request(
                                station_id,
                                coil_values.clone(),
                            ) {
                                Ok(mut request) => {
                                    if let Err(e) = request.generate_set_coils_bulk(
                                        register_address,
                                        &coil_values,
                                        &mut request_frame,
                                    ) {
                                        log::error!("Failed to generate coils write frame: {}", e);
                                    } else {
                                        // Call on_before_write hooks to transform data (e.g., byte-swap)
                                        for hook in &hooks {
                                            if let Err(e) = hook.on_before_write(
                                                &port_name,
                                                &mut request_frame,
                                                register_mode,
                                            ) {
                                                log::warn!("Hook on_before_write failed: {}", e);
                                            }
                                        }

                                        // debug info removed

                                        // Send write request and receive confirmation
                                        {
                                            let mut port = port_arc.lock().unwrap();
                                            if let Err(e) = port.write_all(&request_frame) {
                                                log::error!("Failed to send write request: {}", e);
                                                let err = anyhow::anyhow!(
                                                    "Failed to send write request: {}",
                                                    e
                                                );
                                                for hook in &hooks {
                                                    hook.on_error(&port_name, &err);
                                                }
                                            } else if let Err(e) = port.flush() {
                                                log::error!("Failed to flush write request: {}", e);
                                            } else {
                                                // Wait for confirmation
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(50),
                                                );
                                                let mut buffer = vec![0u8; 256];
                                                match port.read(&mut buffer) {
                                                    Ok(bytes_read) if bytes_read >= 8 => {
                                                        let response = &buffer[..bytes_read];
                                                        // debug info removed
                                                        if response[1] == 0x0F {
                                                            log::info!("Successfully wrote {} coils to slave at address 0x{:04X}", coil_values.len(), register_address);
                                                        } else if response[1] & 0x80 != 0 {
                                                            log::error!("Modbus exception: error code 0x{:02X}", response[2]);
                                                        }
                                                    }
                                                    Ok(bytes_read) => {
                                                        log::warn!(
                                                            "Incomplete write response: {} bytes",
                                                            bytes_read
                                                        );
                                                    }
                                                    Err(e) => {
                                                        log::error!(
                                                            "Failed to read write confirmation: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to generate coils write request: {}", e);
                                }
                            }
                        }
                        RegisterMode::Holding => {
                            log::warn!("Holding register write not yet implemented");
                        }
                        _ => {
                            log::warn!("Write operation not supported for {:?}", register_mode);
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    log::error!("Data source chain error: {}", e);
                }
            }
        }

        // Poll the slave
        match core::execute_single_poll_internal(
            &port_arc,
            station_id,
            register_address,
            register_length,
            register_mode,
        ) {
            Ok(response) => {
                // Execute hook chain: on_after_response
                for hook in &hooks {
                    if let Err(e) = hook.on_after_response(&port_name, &response) {
                        log::warn!("Hook on_after_response failed: {}", e);
                    }
                }

                // Send response to channel
                if sender.send(response).is_err() {
                    log::warn!("Receiver dropped, stopping master loop");
                    break;
                }
            }
            Err(err) => {
                // Determine retry policy from hooks (first provider wins)
                let mut max_retries = 0u32;
                let mut retry_delay_ms = 500u64;
                for hook in &hooks {
                    if max_retries == 0 {
                        if let Some(v) = hook.hook_max_retries() {
                            max_retries = v;
                        }
                    }
                    if retry_delay_ms == 500 {
                        if let Some(v) = hook.hook_retry_delay_ms() {
                            retry_delay_ms = v;
                        }
                    }
                }

                if max_retries == 0 {
                    log::warn!("Error polling on {}: {}", port_name, err);
                    for hook in &hooks {
                        hook.on_error(&port_name, &err);
                    }
                } else {
                    log::warn!(
                        "Error polling on {}: {} - retrying up to {} times",
                        port_name,
                        err,
                        max_retries
                    );
                    let mut last_err = err;
                    let mut success = false;
                    for attempt in 1..=max_retries {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_delay_ms)).await;

                        // call before_request hooks again
                        for hook in &hooks {
                            if let Err(e) = hook.on_before_request(&port_name) {
                                log::warn!("Hook on_before_request failed during retry: {}", e);
                            }
                        }

                        match core::execute_single_poll_internal(
                            &port_arc,
                            station_id,
                            register_address,
                            register_length,
                            register_mode,
                        ) {
                            Ok(response) => {
                                for hook in &hooks {
                                    if let Err(e) = hook.on_after_response(&port_name, &response) {
                                        log::warn!("Hook on_after_response failed: {}", e);
                                    }
                                }
                                if sender.send(response).is_err() {
                                    log::warn!("Receiver dropped, stopping master loop");
                                    return Ok(());
                                }
                                success = true;
                                break;
                            }
                            Err(e2) => {
                                last_err = e2;
                                for hook in &hooks {
                                    hook.on_error(&port_name, &last_err);
                                }
                                log::warn!(
                                    "Retry {}/{} failed: {}",
                                    attempt,
                                    max_retries,
                                    last_err
                                );
                            }
                        }
                    }

                    if !success {
                        log::warn!(
                            "Retries exhausted for {}: last error: {}",
                            port_name,
                            last_err
                        );
                    }
                }
            }
        }

        // Poll interval
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

/// Multi-register master loop - middleware chains (Builder API)
///
/// Polls multiple register types with middleware support
async fn run_multi_register_master_loop(
    config: ModbusPortConfig,
    register_polls: Vec<super::RegisterPollConfig>,
    hooks: Vec<Arc<dyn ModbusHook>>,
    mut data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
    sender: flume::Sender<ModbusResponse>,
) -> Result<()> {
    let ModbusPortConfig {
        port_name,
        baud_rate,
        station_id,
        register_address: _,
        register_length: _,
        register_mode: _,
        timeout_ms,
        error_recovery_delay_ms: _, // Master does not use this field
        poll_interval_ms,           // Use the configured poll interval
    } = config;

    log::info!(
        "Starting multi-register master loop (middleware) for {} ({} register types)",
        port_name,
        register_polls.len()
    );

    // Open port once (shared across all register types)
    let port_handle = open_serial_port(&port_name, baud_rate, Duration::from_millis(timeout_ms))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        // Poll each register type
        for poll_config in &register_polls {
            // Execute hook chain: on_before_request
            for hook in &hooks {
                if let Err(e) = hook.on_before_request(&port_name) {
                    log::warn!("Hook on_before_request failed: {}", e);
                }
            }

            // Try to get data from data source chain (before each poll)
            if !data_sources.is_empty() {
                match super::traits::execute_data_source_chain(&mut data_sources) {
                    Ok(Some(values)) => {
                        log::info!(
                            "Data source provided {} values for write operation",
                            values.len()
                        );

                        // Perform write operation for Coils register type
                        // Write to the first Coils register found in poll configs
                        if let Some(coils_config) = register_polls
                            .iter()
                            .find(|p| matches!(p.register_mode, RegisterMode::Coils))
                        {
                            // Convert u16 values to bool for coils (0 = false, non-zero = true)
                            let coil_values: Vec<bool> = values.iter().map(|&v| v != 0).collect();

                            log::info!(
                                "Writing {} coils to address 0x{:04X}",
                                coil_values.len(),
                                coils_config.register_address
                            );

                            // Generate write request frame
                            let mut request_frame = Vec::new();
                            match crate::protocol::modbus::generate_pull_set_coils_request(
                                station_id,
                                coil_values.clone(),
                            ) {
                                Ok(mut request) => {
                                    if let Err(e) = request.generate_set_coils_bulk(
                                        coils_config.register_address,
                                        &coil_values,
                                        &mut request_frame,
                                    ) {
                                        log::error!("Failed to generate coils write frame: {}", e);
                                    } else {
                                        // Call on_before_write hooks to transform data (e.g., byte-swap)
                                        for hook in &hooks {
                                            if let Err(e) = hook.on_before_write(
                                                &port_name,
                                                &mut request_frame,
                                                RegisterMode::Coils,
                                            ) {
                                                log::warn!("Hook on_before_write failed: {}", e);
                                            }
                                        }

                                        // Send write request
                                        let write_result = {
                                            let mut port = port_arc.lock().unwrap();
                                            let write_res = port.write_all(&request_frame);
                                            if write_res.is_ok() {
                                                port.flush()
                                            } else {
                                                write_res
                                            }
                                        }; // Lock is released here

                                        if let Err(e) = write_result {
                                            log::error!(
                                                "Failed to send/flush write request: {}",
                                                e
                                            );
                                            let err = anyhow::anyhow!(
                                                "Failed to send write request: {}",
                                                e
                                            );
                                            for hook in &hooks {
                                                hook.on_error(&port_name, &err);
                                            }
                                        } else {
                                            // Wait for confirmation (after releasing lock)
                                            tokio::time::sleep(Duration::from_millis(50)).await;

                                            // Read confirmation (acquire lock again)
                                            let mut buffer = vec![0u8; 256];
                                            let read_result = {
                                                let mut port = port_arc.lock().unwrap();
                                                port.read(&mut buffer)
                                            };

                                            match read_result {
                                                Ok(bytes_read) if bytes_read >= 8 => {
                                                    let response = &buffer[..bytes_read];
                                                    if response[1] == 0x0F {
                                                        log::info!("Successfully wrote {} coils to slave at address 0x{:04X}", coil_values.len(), coils_config.register_address);
                                                    } else if response[1] & 0x80 != 0 {
                                                        log::error!(
                                                            "Modbus exception: error code 0x{:02X}",
                                                            response[2]
                                                        );
                                                    }
                                                }
                                                Ok(bytes_read) => {
                                                    log::warn!(
                                                        "Incomplete write response: {} bytes",
                                                        bytes_read
                                                    );
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "Failed to read write confirmation: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to generate coils write request: {}", e);
                                }
                            }
                        } else {
                            log::warn!("Data source provided data but no Coils register type is configured for writing");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Data source chain error: {}", e);
                    }
                }
            }

            // Poll the register
            match core::execute_single_poll_internal(
                &port_arc,
                station_id,
                poll_config.register_address,
                poll_config.register_length,
                poll_config.register_mode,
            ) {
                Ok(response) => {
                    // debug info removed

                    // Execute hook chain: on_after_response
                    for hook in &hooks {
                        if let Err(e) = hook.on_after_response(&port_name, &response) {
                            log::warn!("Hook on_after_response failed: {}", e);
                        }
                    }

                    // Send response to channel
                    if sender.send(response).is_err() {
                        log::warn!("Receiver dropped, stopping master loop");
                        return Ok(());
                    }
                }
                Err(err) => {
                    log::warn!(
                        "Error polling {} register on {}: {}",
                        poll_config.register_mode,
                        port_name,
                        err
                    );
                    let err_msg = err.to_string();

                    // Extract retry policy from hooks (first provider wins)
                    let mut max_retries = 0u32;
                    let mut retry_delay_ms = 500u64;
                    for hook in &hooks {
                        if max_retries == 0 {
                            if let Some(v) = hook.hook_max_retries() {
                                max_retries = v;
                            }
                        }
                        if retry_delay_ms == 500 {
                            if let Some(v) = hook.hook_retry_delay_ms() {
                                retry_delay_ms = v;
                            }
                        }
                    }

                    if max_retries == 0 {
                        for hook in &hooks {
                            hook.on_error(&port_name, &err);
                        }
                    } else {
                        log::warn!(
                            "Retrying {} register on {} up to {} times",
                            poll_config.register_mode,
                            port_name,
                            max_retries
                        );
                        let mut last_err = err;
                        let mut success = false;
                        for attempt in 1..=max_retries {
                            tokio::time::sleep(std::time::Duration::from_millis(retry_delay_ms))
                                .await;

                            // call before_request hooks again
                            for hook in &hooks {
                                if let Err(e) = hook.on_before_request(&port_name) {
                                    log::warn!("Hook on_before_request failed during retry: {}", e);
                                }
                            }

                            match core::execute_single_poll_internal(
                                &port_arc,
                                station_id,
                                poll_config.register_address,
                                poll_config.register_length,
                                poll_config.register_mode,
                            ) {
                                Ok(response) => {
                                    for hook in &hooks {
                                        if let Err(e) =
                                            hook.on_after_response(&port_name, &response)
                                        {
                                            log::warn!("Hook on_after_response failed: {}", e);
                                        }
                                    }
                                    if sender.send(response).is_err() {
                                        log::warn!("Receiver dropped, stopping master loop");
                                        return Ok(());
                                    }
                                    success = true;
                                    break;
                                }
                                Err(e2) => {
                                    last_err = e2;
                                    for hook in &hooks {
                                        hook.on_error(&port_name, &last_err);
                                    }
                                    log::warn!(
                                        "Retry {}/{} failed: {}",
                                        attempt,
                                        max_retries,
                                        last_err
                                    );
                                }
                            }
                        }

                        if !success {
                            log::warn!(
                                "Retries exhausted for {} register on {}: last error: {}",
                                poll_config.register_mode,
                                port_name,
                                last_err
                            );
                        }
                    }

                    // Additional delay after an error to give the serial buffer time to recover
                    if err_msg.contains("Function code mismatch") {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }

            // Interval is configurable via `with_poll_interval`; default is 1000ms
            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
}
