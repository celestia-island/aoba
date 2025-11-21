pub mod master;
pub mod slave;

use anyhow::{anyhow, Result};
use std::sync::Arc;

// Re-export types from protocol
#[doc(hidden)]
pub use crate::protocol::status::types::cli::OutputSink;
#[doc(hidden)]
pub use crate::protocol::status::types::modbus::ModbusMasterDataSource;
pub use crate::protocol::status::types::modbus::{ModbusResponse, RegisterMode, StationMode};

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

    /// Run the Modbus loop with the configured settings.
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

/// Run the Modbus loop with the given configurations.
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
                StationMode::Slave => slave::run_slave_loop(config, hooks).await,
                StationMode::Master => master::run_master_loop(config, hooks).await,
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}

pub(crate) fn extract_values_from_storage(
    storage: &std::sync::Arc<std::sync::Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
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
