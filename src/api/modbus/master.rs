use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    core::master_poll_once,
    traits::{ModbusDataSource, ModbusMasterHandler},
    ModbusHook, ModbusPortConfig,
};
use crate::{api::utils::open_serial_port, protocol::status::types::modbus::ModbusResponse};

/// Handle to a running Modbus master that polls a slave station
pub struct ModbusMaster {
    receiver: flume::Receiver<ModbusResponse>,
    _handle: tokio::task::JoinHandle<Result<()>>,
}

impl ModbusMaster {
    /// Create and start a new Modbus master with an optional data source
    pub fn start(
        config: ModbusPortConfig,
        hooks: Option<Arc<dyn ModbusHook>>,
        data_source: Option<Arc<Mutex<dyn ModbusDataSource>>>,
    ) -> Result<Self> {
        let (sender, receiver) = flume::unbounded();

        let handle =
            tokio::spawn(async move { run_master_loop(config, hooks, data_source, sender).await });

        Ok(Self {
            receiver,
            _handle: handle,
        })
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

pub(crate) async fn run_master_loop(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
    data_source: Option<Arc<Mutex<dyn ModbusDataSource>>>,
    sender: flume::Sender<ModbusResponse>,
) -> Result<()> {
    log::info!("Starting master loop for {}", config.port_name);

    let port_handle = open_serial_port(
        &config.port_name,
        config.baud_rate,
        Duration::from_millis(config.timeout_ms),
    )?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        if let Some(h) = &hooks {
            if let Err(e) = h.on_before_request(&config.port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        // Check if data source has new data to write
        if let Some(ds) = &data_source {
            if let Some(values) = ds.lock().unwrap().next_data() {
                log::debug!("Writing {} values from data source", values.len());
                // TODO: Implement write operation based on register mode
                // For now, we'll just log it
            }
        }

        match master_poll_once(
            port_arc.clone(),
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

                // Send response to channel
                if sender.send(response).is_err() {
                    log::warn!("Receiver dropped, stopping master loop");
                    break;
                }
            }
            Err(err) => {
                log::warn!("Error polling on {}: {}", config.port_name, err);
                if let Some(h) = &hooks {
                    h.on_error(&config.port_name, &err);
                }
            }
        }

        // Poll interval
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
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
            if let Some(values) = ds.lock().unwrap().next_data() {
                log::debug!("Writing {} values from data source", values.len());
                // TODO: Implement write operation based on register mode
                // For now, we'll just log it
            }
        }

        match master_poll_once(
            port_arc.clone(),
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
                if let Err(e) = handler.handle_response(response) {
                    log::error!("Handler failed to process response: {}", e);
                    if let Some(h) = &hooks {
                        h.on_error(&config.port_name, &e);
                    }
                }
            }
            Err(err) => {
                log::warn!("Error polling on {}: {}", config.port_name, err);
                if let Some(h) = &hooks {
                    h.on_error(&config.port_name, &err);
                }
            }
        }

        // Poll interval
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
