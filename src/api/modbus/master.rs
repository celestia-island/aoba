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
    /// Create and start a new Modbus master (new Builder API)
    ///
    /// Accepts hooks and data sources as vectors for middleware pattern.
    ///
    /// # Parameters
    ///
    /// - `hooks`: Vector of hooks (executed in order, first `Ok` intercepts)
    /// - `data_sources`: Vector of data sources (first to provide data wins)
    pub fn new(
        config: ModbusPortConfig,
        hooks: Vec<Arc<dyn ModbusHook>>,
        data_sources: Vec<Arc<Mutex<dyn ModbusDataSource>>>,
    ) -> Result<Self> {
        let (sender, receiver) = flume::unbounded();

        let handle =
            tokio::spawn(async move { run_master_loop(config, hooks, data_sources, sender).await });

        Ok(Self {
            receiver,
            _handle: handle,
        })
    }

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
        let handle = tokio::spawn(async move {
            run_multi_register_master_loop(config, register_polls, hooks, data_sources, sender)
                .await
        });

        Ok(Self {
            receiver,
            _handle: handle,
        })
    }

    /// Create and start a multi-register Modbus masterrator-like interface)
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
                Ok(Some(values)) => {
                    log::debug!("Writing {} values from data source", values.len());
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
                if let Err(e) = handler.handle_response(&response) {
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
// 这是即将添加到 master.rs 末尾的新函数实现
// 用于 Builder 模式的中间件循环

/// Master loop - uses middleware chains for hooks and data sources (Builder API)
///
/// 使用中间件链处理钩子和数据源
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
    } = config;

    log::info!("Starting master loop (middleware) for {}", port_name);
    log::debug!(
        "  Hooks: {}, Data sources: {}",
        hooks.len(),
        data_sources.len()
    );

    let port_handle = open_serial_port(&port_name, baud_rate, Duration::from_millis(timeout_ms))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    loop {
        // Execute hook chain: on_before_request
        for hook in &hooks {
            if let Err(e) = hook.on_before_request(&port_name) {
                log::warn!("Hook on_before_request failed: {}", e);
            }
        }

        // Try to get data from data source chain
        if !data_sources.is_empty() {
            match super::traits::execute_data_source_chain(&mut data_sources) {
                Ok(Some(values)) => {
                    log::debug!("Data source provided {} values", values.len());
                    // TODO: Implement write operation based on register mode
                    // For now, we'll just log it
                }
                Ok(None) => {
                    log::trace!("No data sources provided data this cycle");
                }
                Err(e) => {
                    log::error!("Data source chain error: {}", e);
                }
            }
        }

        // Poll the slave
        match master_poll_once(
            port_arc.clone(),
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
                log::warn!("Error polling on {}: {}", port_name, err);
                for hook in &hooks {
                    hook.on_error(&port_name, &err);
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
    } = config;

    log::info!(
        "Starting multi-register master loop (middleware) for {} ({} register types)",
        port_name,
        register_polls.len()
    );
    log::debug!(
        "  Hooks: {}, Data sources: {}",
        hooks.len(),
        data_sources.len()
    );

    // Open port once (shared across all register types)
    let port_handle = open_serial_port(&port_name, baud_rate, Duration::from_millis(timeout_ms))?;
    let port_arc = Arc::new(Mutex::new(port_handle));

    let mut poll_index = 0;

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
                        log::debug!("Data source provided {} values", values.len());
                        // TODO: Implement write operation
                    }
                    Ok(None) => {
                        log::trace!("No data sources provided data for this poll");
                    }
                    Err(e) => {
                        log::error!("Data source chain error: {}", e);
                    }
                }
            }

            // Poll the register
            match master_poll_once(
                port_arc.clone(),
                station_id,
                poll_config.register_address,
                poll_config.register_length,
                poll_config.register_mode,
            ) {
                Ok(response) => {
                    log::debug!(
                        "Polled {} register at 0x{:04X}: {} values",
                        poll_config.register_mode,
                        poll_config.register_address,
                        response.values.len()
                    );

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
                    for hook in &hooks {
                        hook.on_error(&port_name, &err);
                    }
                }
            }

            // Delay between register types (prevent bus conflicts)
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        poll_index += 1;
        log::trace!("Completed poll cycle #{}", poll_index);

        // Delay between complete cycles
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
