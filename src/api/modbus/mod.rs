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
pub use core::{master_poll_once, slave_process_one_request};

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
}

/// Configuration for a single register polling task
#[derive(Debug, Clone)]
pub struct RegisterPollConfig {
    pub register_mode: RegisterMode,
    pub register_address: u16,
    pub register_length: u16,
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
    station_id: u8,
    register_address: u16,
    register_length: u16,
    register_mode: RegisterMode,
    timeout_ms: u64,
    role: StationMode,
    // 支持多寄存器类型轮询
    register_polls: Vec<RegisterPollConfig>,
    // 中间件链：支持多个钩子
    hooks: Vec<Arc<dyn ModbusHook>>,
    // 中间件链：支持多个数据源（仅 Master）
    data_sources: Vec<Arc<Mutex<dyn traits::ModbusDataSource>>>,
}

impl ModbusBuilder {
    /// Create a new builder for a Modbus Master.
    pub fn new_master(station_id: u8) -> Self {
        Self {
            port_name: None,
            baud_rate: 9600,
            station_id,
            register_address: 0,
            register_length: 10,
            register_mode: RegisterMode::Holding,
            timeout_ms: 1000,
            role: StationMode::Master,
            register_polls: Vec::new(),
            hooks: Vec::new(),
            data_sources: Vec::new(),
        }
    }

    /// Create a new builder for a Modbus Slave.
    pub fn new_slave(station_id: u8) -> Self {
        Self {
            port_name: None,
            baud_rate: 9600,
            station_id,
            register_address: 0,
            register_length: 10,
            register_mode: RegisterMode::Holding,
            timeout_ms: 1000,
            role: StationMode::Slave,
            register_polls: Vec::new(),
            hooks: Vec::new(),
            data_sources: Vec::new(),
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

        slave::ModbusSlave::new(
            port_name,
            self.baud_rate,
            self.station_id,
            self.register_mode,
            self.register_address,
            self.register_length,
            self.timeout_ms,
            self.hooks,
        )
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
    pub fn build_master(self) -> Result<master::ModbusMaster> {
        if self.role != StationMode::Master {
            return Err(anyhow!("Builder is configured for Slave, not Master"));
        }

        let port_name = self.port_name.ok_or_else(|| {
            anyhow!("Port name is required. Use with_port() or with_virtual_port()")
        })?;

        // 如果有多个轮询配置，使用多寄存器模式
        if !self.register_polls.is_empty() {
            master::ModbusMaster::new_multi_register(
                port_name,
                self.baud_rate,
                self.station_id,
                self.register_polls,
                self.timeout_ms,
                self.hooks,
                self.data_sources,
            )
        } else {
            // 单寄存器模式
            master::ModbusMaster::new(
                port_name,
                self.baud_rate,
                self.station_id,
                self.register_mode,
                self.register_address,
                self.register_length,
                self.timeout_ms,
                self.hooks,
                self.data_sources,
            )
        }
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
}
