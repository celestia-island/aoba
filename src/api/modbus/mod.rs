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
pub use crate::protocol::status::types::modbus::{ModbusResponse, RegisterMode, StationMode};

// Re-export core traits (API layer - abstract interfaces)
pub use traits::{
    LoggingHandler, ModbusDataSource, ModbusMasterHandler, ModbusSlaveHandler, NoOpHandler,
};

// Re-export concrete implementations (kept for backward compatibility)
pub use master::ModbusMaster;
pub use slave::ModbusSlave;

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

/// Builder for creating Modbus configurations and starting loops.
pub struct ModbusBuilder {
    port_name: Option<String>,
    baud_rate: u32,
    station_id: u8,
    register_address: u16,
    register_length: u16,
    register_mode: RegisterMode,
    timeout_ms: u64,
    role: StationMode,
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

    /// Set the register configuration.
    pub fn with_register(mut self, mode: RegisterMode, address: u16, length: u16) -> Self {
        self.register_mode = mode;
        self.register_address = address;
        self.register_length = length;
        self
    }

    /// Set the timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Build the configuration.
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

    /// Start a Modbus slave and return a handle with iterator-like interface
    pub fn start_slave(self, hooks: Option<Arc<dyn ModbusHook>>) -> Result<slave::ModbusSlave> {
        if self.role != StationMode::Slave {
            return Err(anyhow!("Builder is configured for Master, not Slave"));
        }
        let config = self.build()?;
        slave::ModbusSlave::start(config, hooks)
    }

    /// Start a Modbus master and return a handle with iterator-like interface
    pub fn start_master(
        self,
        hooks: Option<Arc<dyn ModbusHook>>,
        data_source: Option<Arc<Mutex<dyn traits::ModbusDataSource>>>,
    ) -> Result<master::ModbusMaster> {
        if self.role != StationMode::Master {
            return Err(anyhow!("Builder is configured for Slave, not Master"));
        }
        let config = self.build()?;
        master::ModbusMaster::start(config, hooks, data_source)
    }

    /// Run the Modbus loop with the configured settings (legacy API, kept for compatibility).
    pub async fn run(self, hooks: Option<Arc<dyn ModbusHook>>) -> Result<()> {
        let role = self.role;
        let config = self.build()?;

        // For now, we wrap the single config in a vector as the underlying run_modbus_loop expects multiple configs
        // In the future, we might want to support multiple configs in the builder or change run_modbus_loop
        run_modbus_loop(vec![config], role, None, hooks).await
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

/// Run the Modbus loop with the given configurations (legacy API).
pub async fn run_modbus_loop(
    configs: Vec<ModbusPortConfig>,
    role: StationMode,
    _data_source: Option<ModbusMasterDataSource>,
    hooks: Option<Arc<dyn ModbusHook>>,
) -> Result<()> {
    if configs.is_empty() {
        return Err(anyhow!("No configurations provided"));
    }

    let mut handles = Vec::new();

    for config in configs {
        let hooks = hooks.clone();

        let handle = tokio::spawn(async move {
            match role {
                StationMode::Slave => {
                    let (sender, _receiver) = flume::unbounded();
                    slave::run_slave_loop(config, hooks, sender).await
                }
                StationMode::Master => {
                    let (sender, _receiver) = flume::unbounded();
                    master::run_master_loop(config, hooks, None, sender).await
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}
