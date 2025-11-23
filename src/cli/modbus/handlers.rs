/// CLI-specific implementations of Modbus handlers using flume channels
///
/// This module provides the default implementations that use flume channels for communication.
/// These are used by the CLI commands and can also be used by external applications.
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{
    api::modbus::traits::{ModbusMasterHandler, ModbusSlaveHandler},
    protocol::status::types::modbus::ModbusResponse,
};

/// Flume-based slave handler that sends responses through a channel
pub struct FlumeSlaveHandler {
    sender: flume::Sender<ModbusResponse>,
    running: Arc<AtomicBool>,
}

impl FlumeSlaveHandler {
    /// Create a new flume slave handler with an unbounded channel
    pub fn new() -> (Self, flume::Receiver<ModbusResponse>) {
        let (sender, receiver) = flume::unbounded();
        let handler = Self {
            sender,
            running: Arc::new(AtomicBool::new(true)),
        };
        (handler, receiver)
    }

    /// Create a new flume slave handler with a bounded channel
    pub fn with_capacity(cap: usize) -> (Self, flume::Receiver<ModbusResponse>) {
        let (sender, receiver) = flume::bounded(cap);
        let handler = Self {
            sender,
            running: Arc::new(AtomicBool::new(true)),
        };
        (handler, receiver)
    }

    /// Stop the handler (causes should_continue to return false)
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Get a handle to control this handler
    pub fn get_control_handle(&self) -> FlumeHandlerControl {
        FlumeHandlerControl {
            running: Arc::clone(&self.running),
        }
    }
}

impl Default for FlumeSlaveHandler {
    fn default() -> Self {
        Self::new().0
    }
}

impl ModbusSlaveHandler for FlumeSlaveHandler {
    fn handle_response(&self, response: &ModbusResponse) -> Result<()> {
        self.sender
            .send(response.clone())
            .map_err(|_| anyhow::anyhow!("Failed to send response: receiver dropped"))
    }

    fn should_continue(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Flume-based master handler that sends poll responses through a channel
pub struct FlumeMasterHandler {
    sender: flume::Sender<ModbusResponse>,
    running: Arc<AtomicBool>,
}

impl FlumeMasterHandler {
    /// Create a new flume master handler with an unbounded channel
    pub fn new() -> (Self, flume::Receiver<ModbusResponse>) {
        let (sender, receiver) = flume::unbounded();
        let handler = Self {
            sender,
            running: Arc::new(AtomicBool::new(true)),
        };
        (handler, receiver)
    }

    /// Create a new flume master handler with a bounded channel
    pub fn with_capacity(cap: usize) -> (Self, flume::Receiver<ModbusResponse>) {
        let (sender, receiver) = flume::bounded(cap);
        let handler = Self {
            sender,
            running: Arc::new(AtomicBool::new(true)),
        };
        (handler, receiver)
    }

    /// Stop the handler (causes should_continue to return false)
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Get a handle to control this handler
    pub fn get_control_handle(&self) -> FlumeHandlerControl {
        FlumeHandlerControl {
            running: Arc::clone(&self.running),
        }
    }
}

impl Default for FlumeMasterHandler {
    fn default() -> Self {
        Self::new().0
    }
}

impl ModbusMasterHandler for FlumeMasterHandler {
    fn handle_response(&self, response: &ModbusResponse) -> Result<()> {
        self.sender
            .send(response.clone())
            .map_err(|_| anyhow::anyhow!("Failed to send response: receiver dropped"))
    }

    fn should_continue(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Control handle for flume handlers
///
/// Allows external code to stop the handler gracefully
pub struct FlumeHandlerControl {
    running: Arc<AtomicBool>,
}

impl FlumeHandlerControl {
    /// Request the handler to stop
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the handler is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::status::types::modbus::ResponseRegisterMode;

    #[test]
    fn test_flume_slave_handler() {
        let (handler, receiver) = FlumeSlaveHandler::new();

        let response = ModbusResponse {
            station_id: 1,
            register_address: 0,
            register_mode: ResponseRegisterMode::Holding,
            values: vec![1, 2, 3],
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        handler.handle_response(&response).unwrap();

        let received = receiver.recv().unwrap();
        assert_eq!(received.station_id, response.station_id);
        assert_eq!(received.values, response.values);
    }

    #[test]
    fn test_handler_control() {
        let (handler, _receiver) = FlumeSlaveHandler::new();
        let control = handler.get_control_handle();

        assert!(control.is_running());
        assert!(handler.should_continue());

        control.stop();

        assert!(!control.is_running());
        assert!(!handler.should_continue());
    }
}
