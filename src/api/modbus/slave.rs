use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    core::slave_process_one_request, traits::ModbusSlaveHandler, ModbusHook, ModbusPortConfig,
    RegisterMode,
};
use crate::{api::utils::open_serial_port, protocol::status::types::modbus::ModbusResponse};

/// Handle to a running Modbus slave that provides an iterator-like interface
pub struct ModbusSlave {
    receiver: flume::Receiver<ModbusResponse>,
    _handle: tokio::task::JoinHandle<Result<()>>,
    // Optional one-shot sender used to request the slave loop to stop.
    // Keeping the sender alive allows callers to trigger a graceful shutdown
    // by sending a unit value. It's optional because other call sites may
    // not require manual stop control.
    stop_sender: Option<flume::Sender<()>>,
}

impl ModbusSlave {
    /// Create and start a new Modbus slave listener (new Builder API)
    ///
    /// Accepts hooks as a vector for middleware pattern.
    pub fn new(
        config: crate::api::modbus::ModbusPortConfig,
        hooks: Vec<Arc<dyn ModbusHook>>,
    ) -> Result<Self> {
        let (sender, receiver) = flume::unbounded();
        let (stop_tx, stop_rx) = flume::bounded::<()>(1);

        let handle = crate::core::task_manager::spawn_task(async move {
            run_slave_loop(config, hooks, sender, Some(stop_rx)).await
        });

        Ok(Self {
            receiver,
            _handle: handle,
            stop_sender: Some(stop_tx),
        })
    }

    /// Request the running slave loop to stop. This sends a one-time message
    /// to the control channel; subsequent calls are no-ops.
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_sender.take() {
            let _ = tx.send(());
        }
    }

    /// Try to receive a response without blocking (Iterator-like interface)
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
}

/// Synchronous Modbus Slave Iterator - blocking on each request
///
/// This provides a pure Iterator interface for slave operations.
/// Each call to `next()` will block until a request is received and processed.
///
/// # Example
///
/// ```no_run
/// use aoba::api::modbus::ModbusSlaveIterator;
///
/// let slave_iter = ModbusSlaveIterator::new("COM2", 57600, 19, 0x10, 33)?;
///
/// for response in slave_iter {
///     println!("Processed request: {:?}", response);
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct ModbusSlaveIterator {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    register_mode: RegisterMode,
}

impl ModbusSlaveIterator {
    /// Create a new synchronous slave iterator
    ///
    /// # Arguments
    ///
    /// * `port_name` - Serial port name (e.g., "COM2" on Windows, "/dev/ttyUSB1" on Linux)
    /// * `baud_rate` - Baud rate (e.g., 57600)
    /// * `station_id` - Modbus station ID
    /// * `register_address` - Starting register address
    /// * `register_length` - Number of registers
    pub fn new(
        port_name: &str,
        baud_rate: u32,
        station_id: u8,
        register_address: u16,
        register_length: u16,
    ) -> Result<Self> {
        Self::with_mode(
            port_name,
            baud_rate,
            station_id,
            register_address,
            register_length,
            RegisterMode::Holding,
        )
    }

    /// Create a new synchronous slave iterator with custom register mode
    pub fn with_mode(
        port_name: &str,
        baud_rate: u32,
        station_id: u8,
        register_address: u16,
        register_length: u16,
        register_mode: RegisterMode,
    ) -> Result<Self> {
        // Open serial port with short timeout (1 second)
        // The process_one_request() function will automatically retry on timeout
        // to provide true blocking behavior without timeout warnings
        let port = open_serial_port(port_name, baud_rate, Duration::from_secs(1))?;

        // Initialize Modbus storage
        let storage = Arc::new(Mutex::new(
            rmodbus::server::storage::ModbusStorageSmall::new(),
        ));

        // Pre-fill storage with mock data (optional, can be customized later)
        {
            use rmodbus::server::context::ModbusContext;
            let mut storage_lock = storage.lock().unwrap();

            for i in 0..register_length {
                let addr = register_address + i;
                let value = 0x1000 + i; // Mock data: 0x1000, 0x1001, 0x1002, ...

                // Set values based on register mode
                match register_mode {
                    RegisterMode::Holding => storage_lock.set_holding(addr, value)?,
                    RegisterMode::Input => storage_lock.set_input(addr, value)?,
                    RegisterMode::Coils => storage_lock.set_coil(addr, value != 0)?,
                    RegisterMode::DiscreteInputs => storage_lock.set_discrete(addr, value != 0)?,
                }
            }

            log::debug!(
                "Initialized {} {:?} registers starting at 0x{:04X}",
                register_length,
                register_mode,
                register_address
            );
        }

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            storage,
            station_id,
            register_address,
            register_length,
            register_mode,
        })
    }

    /// Try to process one request (blocking, with error recovery)
    ///
    /// This function blocks indefinitely until a request is received and processed.
    /// **Key behavior:**
    /// - Timeout errors: silently retried (not returned)
    /// - Parse errors: logged as warning, then **continue waiting for next request**
    /// - Fatal errors (e.g., port closed): returned as `Err`
    ///
    /// This ensures the iterator doesn't crash on malformed packets.
    pub fn try_process_one_request(&self) -> Result<ModbusResponse> {
        loop {
            match slave_process_one_request(
                self.port.clone(),
                self.station_id,
                self.register_address,
                self.register_length,
                self.register_mode,
                self.storage.clone(),
            ) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let err_msg = e.to_string().to_lowercase();

                    // Timeout or empty read - retry silently
                    if err_msg.contains("timeout")
                        || err_msg.contains("timed out")
                        || err_msg.contains("no data received")
                    {
                        log::trace!("Serial port read timeout, retrying...");
                        continue;
                    }

                    // Parse/protocol errors - log warning and continue
                    if err_msg.contains("parse")
                        || err_msg.contains("invalid")
                        || err_msg.contains("checksum")
                        || err_msg.contains("crc")
                        || err_msg.contains("frame")
                        || err_msg.contains("broken")
                        || err_msg.contains("malformed")
                        || err_msg.contains("incomplete")
                    {
                        log::warn!("⚠️  Modbus protocol error (ignoring): {}", e);
                        log::debug!("Waiting for next valid request...");
                        continue;
                    }

                    // Fatal errors (port closed, permission denied, etc.)
                    log::error!("❌ Fatal serial port error: {}", e);
                    return Err(e);
                }
            }
        }
    }

    /// Process one request without error recovery (for TryIterator)
    ///
    /// This function returns ALL errors (including parse errors) to the caller.
    /// Unlike `try_process_one_request()`, it does NOT auto-retry on parse errors.
    ///
    /// **Internal use only** - used by `TryIterator`.
    fn process_one_request_no_recovery(&self) -> Result<ModbusResponse> {
        loop {
            match slave_process_one_request(
                self.port.clone(),
                self.station_id,
                self.register_address,
                self.register_length,
                self.register_mode,
                self.storage.clone(),
            ) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let err_msg = e.to_string().to_lowercase();

                    // ONLY retry timeout/empty read - return ALL other errors
                    if err_msg.contains("timeout")
                        || err_msg.contains("timed out")
                        || err_msg.contains("no data received")
                    {
                        log::trace!("Serial port read timeout, retrying...");
                        continue;
                    }

                    // Return parse errors to caller (don't auto-skip)
                    return Err(e);
                }
            }
        }
    }

    /// Update register values in storage
    ///
    /// This allows updating the mock data that will be sent in responses
    pub fn update_registers(&mut self, values: &[u16]) -> Result<()> {
        use rmodbus::server::context::ModbusContext;
        let mut storage_lock = self.storage.lock().unwrap();

        for (i, &value) in values.iter().enumerate() {
            if i >= self.register_length as usize {
                break;
            }
            let addr = self.register_address + i as u16;

            match self.register_mode {
                RegisterMode::Holding => storage_lock.set_holding(addr, value)?,
                RegisterMode::Input => storage_lock.set_input(addr, value)?,
                RegisterMode::Coils => storage_lock.set_coil(addr, value != 0)?,
                RegisterMode::DiscreteInputs => storage_lock.set_discrete(addr, value != 0)?,
            }
        }

        Ok(())
    }

    /// Initialize coils in storage
    ///
    /// Coils are typically used for discrete on/off control (e.g., valve states).
    /// Each coil is a single bit (true = on/1, false = off/0).
    ///
    /// # Arguments
    ///
    /// * `start_address` - Starting coil address (e.g., 0x0000)
    /// * `coil_states` - Boolean array where true = coil on, false = coil off
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use aoba::api::modbus::ModbusSlaveIterator;
    /// let mut slave = ModbusSlaveIterator::new("COM2", 57600, 19, 0x10, 33)?;
    ///
    /// // Set coils 0x01: bit pattern for valves
    /// // Bit 0-2: bottle valves 1-3, Bit 3-9: pipe valves 1-7, Bit 10: main valve
    /// let valves = vec![
    ///     true, false, false,  // Bottle valves: 1 open, 2-3 closed
    ///     false, false, true, false, false, false, true,  // Pipe valves: 3 and 7 open
    ///     false,  // Main valve closed
    /// ];
    /// slave.init_coils(0x01, &valves)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn init_coils(&mut self, start_address: u16, coil_states: &[bool]) -> Result<()> {
        use rmodbus::server::context::ModbusContext;
        let mut storage_lock = self.storage.lock().unwrap();

        for (i, &state) in coil_states.iter().enumerate() {
            let addr = start_address + i as u16;
            storage_lock.set_coil(addr, state)?;
        }

        log::debug!(
            "Initialized {} coils starting at 0x{:04X}",
            coil_states.len(),
            start_address
        );

        Ok(())
    }

    /// Get direct access to the storage for advanced usage
    ///
    /// This allows manual manipulation of the Modbus storage including
    /// holdings, inputs, coils, and discrete inputs.
    pub fn storage(&self) -> Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>> {
        Arc::clone(&self.storage)
    }
}

/// Implement standard Iterator trait for blocking iteration
///
/// **Note:** This iterator only terminates on **fatal errors** (e.g., port closed).
/// Parse errors and malformed packets are automatically skipped with warnings.
impl Iterator for ModbusSlaveIterator {
    type Item = ModbusResponse;

    /// Block and wait for the next **valid** request
    ///
    /// This method will **permanently block** until a valid request is received.
    /// - Timeout: silently retried
    /// - Parse errors: logged and skipped
    /// - Fatal errors: returns `None` (terminates iteration)
    fn next(&mut self) -> Option<Self::Item> {
        match self.try_process_one_request() {
            Ok(response) => {
                log::debug!(
                    "✅ Processed request: station={}, address=0x{:04X}, values={}",
                    response.station_id,
                    response.register_address,
                    response.values.len()
                );
                Some(response)
            }
            Err(e) => {
                log::error!("❌ Fatal error, terminating iterator: {}", e);
                // Only fatal errors terminate the iterator
                None
            }
        }
    }
}

/// Alternative: Fallible iterator pattern (returns Result items)
///
/// This provides explicit error handling for each iteration.
/// Use this when you want to handle errors explicitly instead of automatic recovery.
impl ModbusSlaveIterator {
    /// Create a fallible iterator that yields `Result<ModbusResponse, Error>`
    ///
    /// Unlike the standard `Iterator` implementation which auto-recovers from parse errors,
    /// this returns each error to the caller for explicit handling.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::ModbusSlaveIterator;
    ///
    /// let slave = ModbusSlaveIterator::new("COM2", 57600, 19, 0x10, 33)?;
    ///
    /// for result in slave.try_iter() {
    ///     match result {
    ///         Ok(response) => println!("Success: {} registers", response.values.len()),
    ///         Err(e) => eprintln!("Error: {}, continuing...", e),
    ///     }
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn try_iter(self) -> TryIterator {
        TryIterator { inner: self }
    }
}

/// Fallible iterator wrapper that yields `Result<ModbusResponse, Error>`
///
/// **Key behavior:**
/// - Returns `Some(Ok(response))` on success
/// - Returns `Some(Err(e))` on parse/protocol errors (parse errors, frame broken, CRC errors, etc.)
/// - **Never returns `None`** - always blocks waiting for next request
/// - Timeout errors are silently retried (not returned to caller)
///
/// This iterator is designed to NEVER terminate on its own.
/// It will keep returning `Some(Result)` indefinitely.
pub struct TryIterator {
    inner: ModbusSlaveIterator,
}

impl Iterator for TryIterator {
    type Item = Result<ModbusResponse>;

    fn next(&mut self) -> Option<Self::Item> {
        // Always return Some(Result) - never terminate
        // Timeout errors are auto-retried internally
        // Parse errors are returned as Err
        Some(self.inner.process_one_request_no_recovery())
    }
}

/// Generic slave loop that uses a handler trait
///
/// This function is independent of communication channels.
/// It calls the handler's methods to process responses.
pub async fn run_slave_loop_with_handler(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
    handler: Arc<dyn ModbusSlaveHandler>,
) -> Result<()> {
    log::info!(
        "Starting slave loop with custom handler for {}",
        config.port_name
    );

    let port_handle = open_serial_port(
        &config.port_name,
        config.baud_rate,
        Duration::from_millis(config.timeout_ms),
    )?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    loop {
        // Check if handler wants to continue
        if !handler.should_continue() {
            log::info!("Handler requested stop, exiting slave loop");
            break;
        }

        if let Some(h) = &hooks {
            if let Err(e) = h.on_before_request(&config.port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        match slave_process_one_request(
            port_arc.clone(),
            config.station_id,
            config.register_address,
            config.register_length,
            config.register_mode,
            storage.clone(),
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
                log::warn!("Error processing request on {}: {}", config.port_name, err);
                if let Some(h) = &hooks {
                    h.on_error(&config.port_name, &err);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}

/// Slave loop - uses middleware chains for hooks (Builder API)
async fn run_slave_loop(
    config: crate::api::modbus::ModbusPortConfig,
    hooks: Vec<Arc<dyn ModbusHook>>,
    sender: flume::Sender<ModbusResponse>,
    control: Option<flume::Receiver<()>>,
) -> Result<()> {
    let crate::api::modbus::ModbusPortConfig {
        port_name,
        baud_rate,
        station_id,
        register_address,
        register_length,
        register_mode,
        timeout_ms,
    } = config;

    log::info!("Starting slave loop (middleware) for {}", port_name);
    log::debug!("  Hooks: {}", hooks.len());

    let port_handle = open_serial_port(&port_name, baud_rate, Duration::from_millis(timeout_ms))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize modbus storage
    let storage = Arc::new(Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::new(),
    ));

    loop {
        // Check for external stop request (non-blocking)
        if let Some(ctrl) = &control {
            if ctrl.try_recv().is_ok() {
                log::info!("Stop requested for {}", port_name);
                break;
            }
        }

        // Execute hook chain: on_before_request
        for hook in &hooks {
            if let Err(e) = hook.on_before_request(&port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        // Process one request (this function handles reading from port internally)
        match slave_process_one_request(
            port_arc.clone(),
            station_id,
            register_address,
            register_length,
            register_mode,
            storage.clone(),
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
                    log::warn!("Receiver dropped, stopping slave loop");
                    break;
                }
            }
            Err(err) => {
                log::warn!("Error processing request on {}: {}", port_name, err);
                // Execute hook chain: on_error
                for hook in &hooks {
                    hook.on_error(&port_name, &err);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}
