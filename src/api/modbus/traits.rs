/// Core Modbus API Traits - Abstract interfaces without communication channel dependencies
///
/// This module defines the core traits for Modbus slave and master handlers.
/// These traits are independent of any specific communication mechanism (like flume channels).
///
/// The CLI module provides default implementations using flume channels.
use anyhow::Result;

use crate::protocol::status::types::modbus::ModbusResponse;

/// Trait for handling Modbus slave responses
///
/// Implement this trait to define how slave responses should be processed.
/// The API core calls this after receiving and parsing a Modbus request.
pub trait ModbusSlaveHandler: Send + Sync {
    /// Handle a Modbus slave response
    ///
    /// Called after successfully processing a Modbus request.
    /// Implementations can send the response through channels, write to files, etc.
    fn handle_response(&self, response: ModbusResponse) -> Result<()>;

    /// Check if the handler should continue processing
    ///
    /// Return false to stop the slave loop gracefully.
    fn should_continue(&self) -> bool {
        true
    }
}

/// Trait for providing data to write to Modbus master
///
/// Implement this trait to provide dynamic data sources for Modbus master writes.
/// The master will call `next_data()` before each write operation.
pub trait ModbusDataSource: Send + Sync {
    /// Get the next set of register values to write
    ///
    /// Returns None if there's no data to write (master will skip write and only read).
    /// Returns Some(Vec<u16>) with the values to write to the configured registers.
    fn next_data(&mut self) -> Option<Vec<u16>>;
}

/// Trait for handling Modbus master responses
///
/// Implement this trait to define how master poll responses should be processed.
pub trait ModbusMasterHandler: Send + Sync {
    /// Handle a Modbus master poll response
    ///
    /// Called after successfully polling and receiving data from a slave.
    /// Implementations can send the response through channels, write to files, etc.
    fn handle_response(&self, response: ModbusResponse) -> Result<()>;

    /// Check if the handler should continue polling
    ///
    /// Return false to stop the master loop gracefully.
    fn should_continue(&self) -> bool {
        true
    }
}

/// No-op handler that does nothing with responses (useful for testing)
pub struct NoOpHandler;

impl ModbusSlaveHandler for NoOpHandler {
    fn handle_response(&self, _response: ModbusResponse) -> Result<()> {
        Ok(())
    }
}

impl ModbusMasterHandler for NoOpHandler {
    fn handle_response(&self, _response: ModbusResponse) -> Result<()> {
        Ok(())
    }
}

/// Logging handler that logs responses to the console
pub struct LoggingHandler;

impl ModbusSlaveHandler for LoggingHandler {
    fn handle_response(&self, response: ModbusResponse) -> Result<()> {
        log::info!(
            "Slave response: station={}, address={}, values={:?}",
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }
}

impl ModbusMasterHandler for LoggingHandler {
    fn handle_response(&self, response: ModbusResponse) -> Result<()> {
        log::info!(
            "Master poll response: station={}, address={}, values={:?}",
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }
}
