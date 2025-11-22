use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    core::slave_process_one_request, traits::ModbusSlaveHandler, ModbusHook, ModbusPortConfig,
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
    /// Create and start a new Modbus slave listener
    pub fn start(config: ModbusPortConfig, hooks: Option<Arc<dyn ModbusHook>>) -> Result<Self> {
        let (sender, receiver) = flume::unbounded();
        // Create a one-shot control channel so callers can request stop.
        let (stop_tx, stop_rx) = flume::bounded::<()>(1);

        // Spawn the slave loop using the project's task manager helper so tasks
        // are scheduled and tracked consistently across the application.
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

pub(crate) async fn run_slave_loop(
    config: ModbusPortConfig,
    hooks: Option<Arc<dyn ModbusHook>>,
    sender: flume::Sender<ModbusResponse>,
    control: Option<flume::Receiver<()>>,
) -> Result<()> {
    log::info!("Starting slave loop for {}", config.port_name);

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
        // Check for external stop request (non-blocking)
        if let Some(ctrl) = &control {
            if ctrl.try_recv().is_ok() {
                log::info!("Stop requested for {}", config.port_name);
                break;
            }
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

                // Send response to channel
                if sender.send(response).is_err() {
                    log::warn!("Receiver dropped, stopping slave loop");
                    break;
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
                if let Err(e) = handler.handle_response(response) {
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
