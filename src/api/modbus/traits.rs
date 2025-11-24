/// Core Modbus API Traits - Abstract interfaces without communication channel dependencies
///
/// This module defines the core traits for Modbus slave and master handlers.
/// These traits are independent of any specific communication mechanism (like flume channels).
///
/// The CLI module provides default implementations using flume channels.
///
/// # Middleware Pattern (Interceptor Chain)
///
/// Handlers work like middleware in a web server:
/// - Return `Ok(())` to **intercept** ("I handled this, stop here")
/// - Return `Err(HandlerError::NotHandled)` to **pass through** ("I can't handle this, try next")
/// - Return `Err(other)` for actual errors (stops processing with error)
///
/// The handler chain stops at the first `Ok`, or returns the last non-NotHandled error.
use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

use crate::protocol::status::types::modbus::ModbusResponse;

/// Error types for middleware-style handlers
#[derive(Debug, Clone)]
pub enum HandlerError {
    /// This handler cannot process the request - pass to next handler in chain
    NotHandled(String),
    /// Actual processing error - stop the chain
    ProcessingError(String),
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::NotHandled(msg) => write!(f, "Not handled: {}", msg),
            HandlerError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
        }
    }
}

impl std::error::Error for HandlerError {}

/// Trait for handling Modbus slave responses (middleware pattern)
///
/// Implement this trait to define how slave responses should be processed.
/// The API core calls this after receiving and parsing a Modbus request.
///
/// # Middleware Behavior
///
/// - Return `Ok(())` to intercept and stop the chain
/// - Return `Err(HandlerError::NotHandled)` to pass to next handler
/// - Return `Err(HandlerError::ProcessingError)` for actual errors
pub trait ModbusSlaveHandler: Send + Sync {
    /// Handle a Modbus slave response
    ///
    /// Called after successfully processing a Modbus request.
    /// Implementations can send the response through channels, write to files, etc.
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Successfully handled, stop processing chain
    /// - `Err(HandlerError::NotHandled)` - Cannot handle, try next handler
    /// - `Err(other)` - Processing error, stop chain with error
    fn handle_response(&self, response: &ModbusResponse) -> Result<()>;

    /// Check if the handler should continue processing
    ///
    /// Return false to stop the slave loop gracefully.
    fn should_continue(&self) -> bool {
        true
    }
}

/// Trait for providing data to write to Modbus master (middleware pattern)
///
/// Implement this trait to provide dynamic data sources for Modbus master writes.
/// The master will call `next_data()` before each write operation.
///
/// # Middleware Behavior
///
/// Data sources are tried in order until one returns `Ok(Some(data))`.
pub trait ModbusDataSource: Send + Sync {
    /// Get the next set of register values to write
    ///
    /// # Returns
    ///
    /// - `Ok(Some(values))` - Data to write, intercepts the chain
    /// - `Ok(None)` - No data from this source, try next
    /// - `Err(HandlerError::NotHandled)` - Cannot provide data, try next
    /// - `Err(other)` - Actual error, stop chain
    fn next_data(&mut self) -> Result<Option<Vec<u16>>>;
}

/// Trait for handling Modbus master responses (middleware pattern)
///
/// Implement this trait to define how master poll responses should be processed.
///
/// # Middleware Behavior
///
/// - Return `Ok(())` to intercept and stop the chain
/// - Return `Err(HandlerError::NotHandled)` to pass to next handler
/// - Return `Err(HandlerError::ProcessingError)` for actual errors
pub trait ModbusMasterHandler: Send + Sync {
    /// Handle a Modbus master poll response
    ///
    /// Called after successfully polling and receiving data from a slave.
    /// Implementations can send the response through channels, write to files, etc.
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Successfully handled, stop processing chain
    /// - `Err(HandlerError::NotHandled)` - Cannot handle, try next handler
    /// - `Err(other)` - Processing error, stop chain with error
    fn handle_response(&self, response: &ModbusResponse) -> Result<()>;

    /// Check if the handler should continue polling
    ///
    /// Return false to stop the master loop gracefully.
    fn should_continue(&self) -> bool {
        true
    }
}

/// No-op handler that passes through (never intercepts)
pub struct NoOpHandler;

impl ModbusSlaveHandler for NoOpHandler {
    fn handle_response(&self, _response: &ModbusResponse) -> Result<()> {
        Err(anyhow!(HandlerError::NotHandled(
            "NoOpHandler: pass through".to_string()
        )))
    }
}

impl ModbusMasterHandler for NoOpHandler {
    fn handle_response(&self, _response: &ModbusResponse) -> Result<()> {
        Err(anyhow!(HandlerError::NotHandled(
            "NoOpHandler: pass through".to_string()
        )))
    }
}

/// Logging handler that logs and **intercepts** (stops the chain)
pub struct LoggingHandler;

impl ModbusSlaveHandler for LoggingHandler {
    fn handle_response(&self, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "Slave response: station={}, address={}, values={:?}",
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(()) // Intercept - handled
    }
}

impl ModbusMasterHandler for LoggingHandler {
    fn handle_response(&self, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "Master poll response: station={}, address={}, values={:?}",
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(()) // Intercept - handled
    }
}

/// Helper function to execute a middleware chain for slave handlers
///
/// # Returns
///
/// - `Ok(())` if any handler intercepted (returned Ok)
/// - `Err` if all handlers passed through (NotHandled) or an error occurred
pub fn execute_slave_handler_chain(
    handlers: &[Arc<dyn ModbusSlaveHandler>],
    response: &ModbusResponse,
) -> Result<()> {
    if handlers.is_empty() {
        return Err(anyhow!("No handlers configured"));
    }

    let mut last_error: Option<anyhow::Error> = None;

    for (i, handler) in handlers.iter().enumerate() {
        match handler.handle_response(response) {
            Ok(()) => {
                return Ok(()); // First success intercepts
            }
            Err(e) => {
                if let Some(handler_err) = e.downcast_ref::<HandlerError>() {
                    match handler_err {
                        HandlerError::NotHandled(_msg) => {

                            // Continue to next handler
                        }
                        HandlerError::ProcessingError(msg) => {
                            log::error!("Handler {} processing error: {}", i, msg);
                            return Err(anyhow!(msg.clone()));
                        }
                    }
                } else {
                    // Other error types are treated as processing errors
                    log::error!("Handler {} error: {}", i, e);
                    last_error = Some(e);
                }
            }
        }
    }

    // All handlers passed through
    Err(last_error.unwrap_or_else(|| anyhow!("All handlers passed through (NotHandled)")))
}

/// Helper function to execute a middleware chain for master handlers
pub fn execute_master_handler_chain(
    handlers: &[Arc<dyn ModbusMasterHandler>],
    response: &ModbusResponse,
) -> Result<()> {
    if handlers.is_empty() {
        return Err(anyhow!("No handlers configured"));
    }

    let mut last_error: Option<anyhow::Error> = None;

    for (i, handler) in handlers.iter().enumerate() {
        match handler.handle_response(response) {
            Ok(()) => {
                return Ok(()); // First success intercepts
            }
            Err(e) => {
                if let Some(handler_err) = e.downcast_ref::<HandlerError>() {
                    match handler_err {
                        HandlerError::NotHandled(_msg) => {

                            // Continue to next handler
                        }
                        HandlerError::ProcessingError(msg) => {
                            log::error!("Handler {} processing error: {}", i, msg);
                            return Err(anyhow!(msg.clone()));
                        }
                    }
                } else {
                    log::error!("Handler {} error: {}", i, e);
                    last_error = Some(e);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("All handlers passed through (NotHandled)")))
}

/// Helper function to execute a middleware chain for data sources
///
/// # Returns
///
/// - `Ok(Some(data))` if any source provided data
/// - `Ok(None)` if all sources returned None or NotHandled
/// - `Err` if a processing error occurred
pub fn execute_data_source_chain(
    sources: &mut [Arc<Mutex<dyn ModbusDataSource>>],
) -> Result<Option<Vec<u16>>> {
    if sources.is_empty() {
        return Ok(None);
    }

    for (i, source) in sources.iter_mut().enumerate() {
        let mut src = source.lock().unwrap();
        match src.next_data() {
            Ok(Some(data)) => {
                return Ok(Some(data)); // First data source intercepts
            }
            Ok(None) => {

                // Continue to next source
            }
            Err(e) => {
                if let Some(handler_err) = e.downcast_ref::<HandlerError>() {
                    match handler_err {
                        HandlerError::NotHandled(_msg) => {

                            // Continue to next source
                        }
                        HandlerError::ProcessingError(msg) => {
                            log::error!("Data source {} processing error: {}", i, msg);
                            return Err(anyhow!(msg.clone()));
                        }
                    }
                } else {
                    log::error!("Data source {} error: {}", i, e);
                    return Err(e);
                }
            }
        }
    }

    Ok(None) // All sources passed through
}
