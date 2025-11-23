//! Aoba — Multi-protocol debugging and simulation tool for Modbus RTU
//!
//! This crate provides the core library for Aoba. It exposes a programmatic
//! API used by the CLI and TUI frontends as well as the daemon runner. For
//! end-user usage see the CLI/TUI examples under `examples/` and the top-level
//! README which documents common usage patterns, daemon configuration, and
//! testing tools.
//!
//! The public modules re-export the main APIs for each domain (protocols,
//! TUI, core helpers, etc.). The internal runtime/boot helpers are placed in
//! a separate, hidden module to keep implementation details out of the generated
//! documentation.
//!
//! ## Programmatic API
//!
//! Aoba provides a trait-based API for embedding Modbus functionality in your
//! Rust applications. The API supports both master (client) and slave (server)
//! roles with customizable hooks and data sources.
//!
//! ### Master Example (Polling a Slave)
//!
//! ```rust,no_run
//! use aoba::api::modbus::{ModbusBuilder, RegisterMode};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create and start a master that polls a slave
//!     let master = ModbusBuilder::new_master(1)
//!         .with_port("/dev/ttyUSB0")
//!         .with_register(RegisterMode::Holding, 0, 10)
//!         .start_master(None, None)?;
//!
//!     // Receive responses via iterator interface
//!     while let Some(response) = master.recv_timeout(std::time::Duration::from_secs(2)) {
//!         println!("Received: {:?}", response.values);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Slave Example (Responding to Requests)
//!
//! ```rust,no_run
//! use aoba::api::modbus::{ModbusBuilder, RegisterMode};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create and start a slave that responds to master requests
//!     let slave = ModbusBuilder::new_slave(1)
//!         .with_port("/dev/ttyUSB0")
//!         .with_register(RegisterMode::Holding, 0, 10)
//!         .start_slave(None)?;
//!
//!     // Receive request notifications via iterator interface
//!     while let Some(notification) = slave.recv_timeout(std::time::Duration::from_secs(10)) {
//!         println!("Processed request: {:?}", notification.values);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Advanced Usage
//!
//! For more advanced usage including custom data sources, hooks, and multiple stations,
//! see the complete examples:
//!
//! - [`examples/api_master`](https://github.com/celestia-island/aoba/tree/master/examples/api_master) - Master with fixed test data patterns
//! - [`examples/api_slave`](https://github.com/celestia-island/aoba/tree/master/examples/api_slave) - Slave with automatic register management
//!
//! The API provides:
//! - **Builder Pattern**: Easy configuration via `ModbusBuilder`
//! - **Trait-Based Design**: Implement `ModbusDataSource`, `ModbusHook`, `ModbusMasterHandler`, and `ModbusSlaveHandler`
//! - **Iterator Interface**: Receive responses/notifications via `recv_timeout()`
//! - **Multiple Register Types**: Holding, Input, Coils, Discrete Inputs
//! - **Flexible Port Management**: Physical serial ports, virtual ports (socat), custom transports

pub mod api;
#[doc(hidden)]
pub mod boot;
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod core;
#[doc(hidden)]
pub mod protocol;
#[doc(hidden)]
pub mod tui;
#[doc(hidden)]
pub mod utils;

pub use api::*;
