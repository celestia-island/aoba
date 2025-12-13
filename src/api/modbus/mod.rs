pub mod core;
pub mod master;
pub mod slave;
pub mod traits;

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

// Re-export types from protocol
#[doc(hidden)]
pub use crate::protocol::status::types::cli::OutputSink;
#[doc(hidden)]
pub use crate::protocol::status::types::modbus::ModbusMasterDataSource;
pub use crate::protocol::status::types::modbus::{
    ModbusResponse, RegisterMode, ResponseRegisterMode, StationMode,
};

// Re-export core traits (API layer - abstract interfaces)
pub use traits::{
    execute_data_source_chain, execute_master_handler_chain, execute_slave_handler_chain,
    HandlerError, LoggingHandler, ModbusDataSource, ModbusMasterHandler, ModbusSlaveHandler,
    NoOpHandler,
};

// Re-export concrete implementations (kept for backward compatibility)
pub use master::ModbusMaster;
pub use slave::{ModbusSlave, ModbusSlaveIterator, TryIterator};

// Re-export core functions for custom implementations
pub use core::{master_poll_loop, master_poll_once_and_stop, slave_process_one_request};

// Re-export CLI default handlers (flume-based implementations)
pub use crate::cli::modbus::handlers::{
    FlumeHandlerControl, FlumeMasterHandler, FlumeSlaveHandler,
};

// Define the configuration struct
#[derive(Debug, Clone)]
pub struct ModbusPortConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub station_id: u8,
    pub register_address: u16,
    pub register_length: u16,
    pub register_mode: RegisterMode,
    pub timeout_ms: u64,
    /// Error recovery delay (milliseconds): pause polling when parsing repeatedly fails to
    /// let both sides' buffers stabilize. Recommended: 100-500ms; adjust according to baud rate
    /// and communication quality.
    pub error_recovery_delay_ms: Option<u64>,
    /// Poll interval (milliseconds): delay between each poll request. For multi-register polling
    /// this is the interval between each register type poll, ensuring the slave has time to process.
    /// Default: 1000ms (1 second); adjust based on slave response time and communication stability.
    pub poll_interval_ms: u64,
}

/// Configuration for a single register polling task
#[derive(Debug, Clone)]
pub struct RegisterPollConfig {
    pub register_mode: RegisterMode,
    pub register_address: u16,
    pub register_length: u16,
}

/// Robust mode configuration for unreliable or slow devices.
///
/// Enables aggressive retry and recovery strategies for devices with:
/// - Very slow response times (3-4+ seconds)
/// - High packet loss rates
/// - Poor buffer management
/// - Intermittent data corruption
#[derive(Debug, Clone)]
pub struct RobustModeConfig {
    /// Increase read timeout significantly (milliseconds).
    /// Recommended: 5000ms for very slow devices. Default: 5000ms.
    pub read_timeout_ms: u64,

    /// Number of retries for failed requests.
    /// Recommended: 10 for extremely unreliable devices. Default: 5.
    pub max_retries: u32,

    /// Delay after timeout/failure before retry (milliseconds).
    /// Recommended: 500ms to let slave stabilize. Default: 500ms.
    pub retry_delay_ms: u64,

    /// Delay after successful reception before next poll (milliseconds).
    /// Recommended: 1000ms to give slave breathing room. Default: 1000ms.
    pub success_delay_ms: u64,

    /// Force flush input buffer before each write (clears residual data).
    /// Recommended: true. Default: true.
    pub flush_input_buffer: bool,
}

impl Default for RobustModeConfig {
    fn default() -> Self {
        Self {
            read_timeout_ms: 5000,
            max_retries: 5,
            retry_delay_ms: 500,
            success_delay_ms: 1000,
            flush_input_buffer: true,
        }
    }
}

/// Builder for creating Modbus configurations and starting loops.
///
/// # Middleware Pattern
///
/// The builder supports middleware-style handler chains:
/// - Add multiple hooks with `.add_hook()` (executed in order)
/// - Add multiple data sources with `.add_data_source()` (for Master)
/// - Handlers return `Ok` to intercept, `Err(NotHandled)` to pass through
pub struct ModbusBuilder {
    port_name: Option<String>,
    baud_rate: u32,
    poll_interval_ms: u64,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    register_mode: RegisterMode,
    timeout_ms: u64,
    error_recovery_delay_ms: Option<u64>,
    role: StationMode,

    // Supports multi-register type polling
    register_polls: Vec<RegisterPollConfig>,
    // Middleware chain: supports multiple hooks
    hooks: Vec<Arc<dyn ModbusHook>>,
    // Middleware chain: supports multiple data sources (Master only)
    data_sources: Vec<Arc<Mutex<dyn traits::ModbusDataSource>>>,

    // Robust mode configuration (for unreliable devices)
    robust_mode: Option<RobustModeConfig>,
}

impl ModbusBuilder {
    /// Create a new builder for a Modbus Master.
    pub fn new_master(station_id: u8) -> Self {
        Self {
            port_name: None,
            baud_rate: 9600,
            poll_interval_ms: 1000, // Default 1-second poll interval
            station_id,
            register_address: 0,
            register_length: 1,
            register_mode: RegisterMode::Holding,
            timeout_ms: 1000,
            error_recovery_delay_ms: None,
            role: StationMode::Master,
            register_polls: Vec::new(),
            hooks: Vec::new(),
            data_sources: Vec::new(),
            robust_mode: None,
        }
    }

    /// Create a new builder for a Modbus Slave.
    pub fn new_slave(station_id: u8) -> Self {
        Self {
            port_name: None,
            baud_rate: 9600,
            poll_interval_ms: 1000, // Default 1-second poll interval (unused by Slave)
            station_id,
            register_address: 0,
            register_length: 10,
            register_mode: RegisterMode::Holding,
            timeout_ms: 1000,
            error_recovery_delay_ms: Some(300), // Slave defaults to 300ms error recovery delay
            role: StationMode::Slave,
            register_polls: Vec::new(),
            hooks: Vec::new(),
            data_sources: Vec::new(),
            robust_mode: None,
        }
    }

    /// Set the serial port name.
    pub fn with_port(mut self, port_name: &str) -> Self {
        self.port_name = Some(port_name.to_string());
        self
    }

    /// Use a virtual serial port with a randomly generated name.
    /// On Unix, this will be `/tmp/vcom_{uuid}`.
    pub fn with_virtual_port(mut self) -> Self {
        let uuid = uuid::Uuid::new_v4();
        #[cfg(unix)]
        let port_name = format!("/tmp/vcom_{}", uuid);
        #[cfg(windows)]
        let port_name = format!("\\\\.\\CNCA{}", uuid.as_u128() % 100); // Fallback/Mock for Windows

        self.port_name = Some(port_name);
        self
    }

    /// Set the baud rate.
    pub fn with_baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Set the register configuration (single register, for backward compatibility).
    pub fn with_register(mut self, mode: RegisterMode, address: u16, length: u16) -> Self {
        self.register_mode = mode;
        self.register_address = address;
        self.register_length = length;
        self
    }

    /// Add a register polling configuration (for multi-register Master).
    ///
    /// This allows a Master to poll multiple register types on the same port.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, RegisterMode};
    ///
    /// let master = ModbusBuilder::new_master(19)
    ///     .with_port("COM1")
    ///     .with_baud_rate(57600)
    ///     .add_register_poll(RegisterMode::Coils, 0x01, 11)      // Poll coils
    ///     .add_register_poll(RegisterMode::Holding, 0x10, 33)    // Poll holdings
    ///     .with_timeout(2000)
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn add_register_poll(mut self, mode: RegisterMode, address: u16, length: u16) -> Self {
        self.register_polls.push(RegisterPollConfig {
            register_mode: mode,
            register_address: address,
            register_length: length,
        });
        self
    }

    /// Set the timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the error recovery delay in milliseconds (Slave only).
    ///
    /// When frame parsing fails continuously, the slave will pause polling
    /// for this duration to let both sides' buffers stabilize.
    ///
    /// Recommended: 100-500ms depending on baud rate and communication quality.
    /// Default for Slave: 300ms. Master ignores this setting.
    pub fn with_error_recovery_delay(mut self, delay_ms: u64) -> Self {
        self.error_recovery_delay_ms = Some(delay_ms);
        self
    }

    /// Set the polling interval in milliseconds (Master only).
    ///
    /// For multi-register masters, this is the delay **between each register type poll**.
    /// Example: poll_interval=1000ms means:
    /// - Poll Coils → wait 1s → Poll Holding → wait 1s → Poll Coils...
    ///
    /// Shorter intervals = faster updates but higher bus load.
    /// Longer intervals = more stable but slower updates.
    ///
    /// Recommended: 500-2000ms depending on:
    /// - Slave response time
    /// - Number of register types
    /// - Communication stability requirements
    ///
    /// Default: 1000ms (1 second)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, RegisterMode};
    ///
    /// let master = ModbusBuilder::new_master(19)
    ///     .with_port("COM1")
    ///     .add_register_poll(RegisterMode::Coils, 0x01, 11)
    ///     .add_register_poll(RegisterMode::Holding, 0x10, 33)
    ///     .with_poll_interval(500)  // Poll every 500ms
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_poll_interval(mut self, interval_ms: u64) -> Self {
        self.poll_interval_ms = interval_ms;
        self
    }

    /// Enable robust mode for unreliable or very slow devices.
    ///
    /// Applies aggressive retry and recovery strategies:
    /// - Increases read timeout to 5 seconds
    /// - Enables 5-10 retries per request
    /// - Adds recovery delays between failures
    /// - Flushes input buffer before each write
    /// - Adds breathing time after successful reads
    ///
    /// Recommended for devices with:
    /// - Very slow response times (3-4+ seconds)
    /// - High packet loss rates
    /// - Poor buffer management
    /// - Intermittent data corruption
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, RegisterMode};
    ///
    /// let master = ModbusBuilder::new_master(0x13)
    ///     .with_port("ttyUSB-CH340-A")
    ///     .with_baud_rate(57600)
    ///     .with_register(RegisterMode::Holding, 0x12, 2)
    ///     .with_robust_mode()  // Enable for unreliable device
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_robust_mode(mut self) -> Self {
        self.robust_mode = Some(RobustModeConfig::default());
        self
    }

    /// Enable robust mode with custom configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, RegisterMode, RobustModeConfig};
    ///
    /// let robust = RobustModeConfig {
    ///     read_timeout_ms: 8000,       // 8 seconds for extremely slow device
    ///     max_retries: 10,             // Try up to 10 times
    ///     retry_delay_ms: 1000,        // Wait 1s between retries
    ///     success_delay_ms: 2000,      // Wait 2s after successful read
    ///     flush_input_buffer: true,
    /// };
    ///
    /// let master = ModbusBuilder::new_master(0x13)
    ///     .with_port("ttyUSB-CH340-A")
    ///     .with_baud_rate(57600)
    ///     .with_robust_mode_config(robust)
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_robust_mode_config(mut self, config: RobustModeConfig) -> Self {
        self.robust_mode = Some(config);
        self
    }

    /// Add a hook to the middleware chain (can be called multiple times)
    ///
    /// Hooks are executed in the order they are added.
    /// The first hook to return `Ok` will intercept the response.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, LoggingHandler};
    /// use std::sync::Arc;
    ///
    /// let master = ModbusBuilder::new_master(1)
    ///     .with_port("COM1")
    ///     .add_hook(Arc::new(LoggingHandler))  // First hook
    ///     .add_hook(Arc::new(CustomHook))      // Second hook
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn add_hook(mut self, hook: Arc<dyn ModbusHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Add a data source to the middleware chain (can be called multiple times, Master only)
    ///
    /// Data sources are tried in the order they are added.
    /// The first source to return `Ok(Some(data))` will intercept.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, ModbusDataSource};
    /// use std::sync::{Arc, Mutex};
    ///
    /// let master = ModbusBuilder::new_master(1)
    ///     .with_port("COM1")
    ///     .add_data_source(Arc::new(Mutex::new(FileDataSource::new("data.csv")?)))
    ///     .add_data_source(Arc::new(Mutex::new(DefaultDataSource::new())))
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn add_data_source(mut self, source: Arc<Mutex<dyn traits::ModbusDataSource>>) -> Self {
        self.data_sources.push(source);
        self
    }

    /// Build the configuration (public legacy API).
    pub fn build(self) -> Result<ModbusPortConfig> {
        let port_name = self.port_name.ok_or_else(|| {
            anyhow!("Port name is required. Use with_port() or with_virtual_port()")
        })?;

        Ok(ModbusPortConfig {
            port_name,
            baud_rate: self.baud_rate,
            station_id: self.station_id,
            register_address: self.register_address,
            register_length: self.register_length,
            register_mode: self.register_mode,
            timeout_ms: self.timeout_ms,
            error_recovery_delay_ms: self.error_recovery_delay_ms,
            poll_interval_ms: self.poll_interval_ms,
        })
    }

    /// Build and start a Modbus slave
    ///
    /// Uses the hooks and data sources configured with `.add_hook()` and `.add_data_source()`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, LoggingHandler};
    /// use std::sync::Arc;
    ///
    /// let slave = ModbusBuilder::new_slave(19)
    ///     .with_port("COM2")
    ///     .with_baud_rate(57600)
    ///     .with_register(RegisterMode::Holding, 0x10, 33)
    ///     .add_hook(Arc::new(LoggingHandler))
    ///     .build_slave()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn build_slave(self) -> Result<slave::ModbusSlave> {
        if self.role != StationMode::Slave {
            return Err(anyhow!("Builder is configured for Master, not Slave"));
        }
        let port_name = self.port_name.ok_or_else(|| {
            anyhow!("Port name is required. Use with_port() or with_virtual_port()")
        })?;

        let config = ModbusPortConfig {
            port_name,
            baud_rate: self.baud_rate,
            station_id: self.station_id,
            register_address: self.register_address,
            register_length: self.register_length,
            register_mode: self.register_mode,
            timeout_ms: self.timeout_ms,
            error_recovery_delay_ms: self.error_recovery_delay_ms,
            poll_interval_ms: self.poll_interval_ms,
        };

        slave::ModbusSlave::new(config, self.hooks)
    }

    /// Build and start a Modbus master
    ///
    /// Uses the hooks and data sources configured with `.add_hook()` and `.add_data_source()`.
    ///
    /// Supports two modes:
    /// 1. Single register polling (using `with_register()`)
    /// 2. Multi-register polling (using `add_register_poll()` multiple times)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aoba::api::modbus::{ModbusBuilder, RegisterMode, LoggingHandler};
    /// use std::sync::Arc;
    ///
    /// let master = ModbusBuilder::new_master(19)
    ///     .with_port("COM1")
    ///     .with_baud_rate(57600)
    ///     .add_register_poll(RegisterMode::Coils, 0x01, 11)
    ///     .add_register_poll(RegisterMode::Holding, 0x10, 33)
    ///     .add_hook(Arc::new(LoggingHandler))
    ///     .with_timeout(2000)
    ///     .build_master()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn build_master(mut self) -> Result<master::ModbusMaster> {
        if self.role != StationMode::Master {
            return Err(anyhow!("Builder is configured for Slave, not Master"));
        }

        let port_name = self.port_name.ok_or_else(|| {
            anyhow!("Port name is required. Use with_port() or with_virtual_port()")
        })?;

        // Apply robust mode configuration if enabled
        if let Some(robust) = &self.robust_mode {
            self.timeout_ms = robust.read_timeout_ms;

            // Add robust mode hook if flush_input_buffer is enabled
            if robust.flush_input_buffer {
                self.hooks.push(Arc::new(RobustModeHook {
                    retry_delay_ms: robust.retry_delay_ms,
                    success_delay_ms: robust.success_delay_ms,
                    max_retries: robust.max_retries,
                }));
            }

            // Adjust poll interval to include breathing room after success
            self.poll_interval_ms = self.poll_interval_ms.max(robust.success_delay_ms);
        }

        // If there are multiple polling configurations, use multi-register mode
        let config = ModbusPortConfig {
            port_name,
            baud_rate: self.baud_rate,
            station_id: self.station_id,
            register_address: self.register_address,
            register_length: self.register_length,
            register_mode: self.register_mode,
            timeout_ms: self.timeout_ms,
            error_recovery_delay_ms: self.error_recovery_delay_ms,
            poll_interval_ms: self.poll_interval_ms,
        };

        if !self.register_polls.is_empty() {
            master::ModbusMaster::new_multi_register(
                config,
                self.register_polls,
                self.hooks,
                self.data_sources,
            )
        } else {
            // Single register mode
            master::ModbusMaster::new(config, self.hooks, self.data_sources)
        }
    }
}

/// Robust mode hook for managing retries and buffer flushing
struct RobustModeHook {
    retry_delay_ms: u64,
    success_delay_ms: u64,
    max_retries: u32,
}

impl ModbusHook for RobustModeHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        // Log configured max_retries so the field is actively used.
        log::debug!("Robust mode: configured max_retries = {}", self.max_retries);
        Ok(())
    }

    fn on_after_response(&self, _port: &str, _response: &ModbusResponse) -> Result<()> {
        // Add breathing time after successful response
        log::debug!(
            "Robust mode: adding {}ms delay after successful response",
            self.success_delay_ms
        );
        std::thread::sleep(std::time::Duration::from_millis(self.success_delay_ms));
        Ok(())
    }

    fn on_error(&self, _port: &str, _error: &anyhow::Error) {
        // Add delay after error
        log::warn!(
            "Robust mode: adding {}ms delay after error",
            self.retry_delay_ms
        );
        std::thread::sleep(std::time::Duration::from_millis(self.retry_delay_ms));
    }

    fn hook_max_retries(&self) -> Option<u32> {
        Some(self.max_retries)
    }

    fn hook_retry_delay_ms(&self) -> Option<u64> {
        Some(self.retry_delay_ms)
    }
}

pub trait ModbusHook: Send + Sync {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }
    fn on_after_response(&self, _port: &str, _response: &ModbusResponse) -> Result<()> {
        Ok(())
    }
    fn on_error(&self, _port: &str, _error: &anyhow::Error) {}

    /// Called before writing data to Modbus (Master writing to Slave)
    ///
    /// This hook allows transforming the data before it's sent over the wire.
    /// Use cases include byte-order corrections, data validation, etc.
    ///
    /// # Parameters
    ///
    /// * `port` - The serial port name
    /// * `data` - Mutable reference to the data buffer (can be modified in-place)
    /// * `register_mode` - The type of register being written (Coils, Holding, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Continue with the write operation
    /// * `Err(_)` - Abort the write operation and report error
    fn on_before_write(
        &self,
        _port: &str,
        _data: &mut Vec<u8>,
        _register_mode: RegisterMode,
    ) -> Result<()> {
        Ok(())
    }

    /// Called after receiving a request but before processing it (Slave receiving from Master)
    ///
    /// This hook allows transforming the received request data before parsing.
    /// Use cases include byte-order corrections for hardware issues.
    ///
    /// # Parameters
    ///
    /// * `port` - The serial port name
    /// * `data` - Mutable reference to the request buffer (can be modified in-place)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Continue with request processing
    /// * `Err(_)` - Abort processing and report error
    fn on_after_receive_request(&self, _port: &str, _data: &mut [u8]) -> Result<()> {
        Ok(())
    }

    /// Optional: return the number of retries this hook requests for failed polls.
    /// Default: `None` (no special retry behavior).
    fn hook_max_retries(&self) -> Option<u32> {
        None
    }

    /// Optional: return the retry delay in milliseconds to use between attempts.
    /// Default: `None` (caller may use a reasonable default).
    fn hook_retry_delay_ms(&self) -> Option<u64> {
        None
    }
}
