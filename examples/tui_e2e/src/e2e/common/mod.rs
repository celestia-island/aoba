//! Common utilities for TUI E2E testing
//!
//! This module provides shared functionality for end-to-end testing of the AOBA TUI
//! (Terminal User Interface). It includes utilities for:
//!
//! - **Retry mechanisms**: Transaction-style operations with safe rollback
//! - **Configuration structures**: Station and register configuration
//! - **Navigation helpers**: TUI panel navigation and setup
//! - **Test execution**: High-level test orchestrators for various scenarios
//!
//! # Module Organization
//!
//! The common module is split into functional areas:
//!
//! - [`retry`]: Transaction-style retry mechanisms with checkpoint-based rollback
//! - [`config`]: Configuration structures for Modbus stations and registers
//! - [`navigation`]: TUI environment setup and panel navigation
//! - [`execution`]: High-level test orchestrators and data verification
//! - [`station`]: Station-specific test utilities and configurations
//!
//! # Usage Examples
//!
//! ## Basic Single-Station Test
//!
//! ```rust,no_run
//! use common::{config::StationConfig, execution::run_single_station_master_test, navigation::setup_tui_test};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure a Master station
//! let config = StationConfig {
//!     station_id: 1,
//!     register_mode: common::config::RegisterMode::Holding,
//!     start_address: 100,
//!     register_count: 10,
//!     is_master: true,
//!     register_values: None,
//! };
//!
//! // Run the test
//! run_single_station_master_test("COM3", "COM4", config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Multi-Station Test
//!
//! ```rust,no_run
//! use common::{config::{StationConfig, RegisterMode}, execution::run_multi_station_master_test};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let masters = vec![
//!     StationConfig {
//!         station_id: 1,
//!         register_mode: RegisterMode::Holding,
//!         start_address: 100,
//!         register_count: 5,
//!         is_master: true,
//!         register_values: None,
//!     },
//! ];
//!
//! let slaves = vec![
//!     StationConfig {
//!         station_id: 1,
//!         register_mode: RegisterMode::Holding,
//!         start_address: 100,
//!         register_count: 5,
//!         is_master: false,
//!         register_values: Some(vec![1000, 2000, 3000, 4000, 5000]),
//!     },
//! ];
//!
//! run_multi_station_master_test("COM3", "COM4", &masters, &slaves).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## TUI Navigation
//!
//! ```rust,no_run
//! use common::navigation::{setup_tui_test, navigate_to_modbus_panel};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Setup TUI environment
//! let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
//!
//! // Navigate to Modbus panel
//! navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Dependencies
//!
//! This module depends on several external crates:
//!
//! - `anyhow`: Error handling
//! - `serde_json`: JSON parsing for CLI output and status files
//! - `ci_utils`: Custom utilities for terminal interaction and screen capture
//! - `expectrl`: Terminal automation for TUI control
//!
//! # Error Handling
//!
//! All functions return `Result<T, anyhow::Error>` to provide detailed error
//! information. Common error patterns include:
//!
//! - **TUI interaction failures**: Navigation timeouts, screen parsing errors
//! - **CLI execution failures**: Binary not found, port unavailable, command errors
//! - **Data verification failures**: Value mismatches, length discrepancies
//! - **Configuration errors**: Invalid parameters, missing required fields
//!
//! # Logging
//!
//! Extensive debug logging is provided throughout all modules. Enable debug logging
//! to see detailed operation traces:
//!
//! ```bash
//! RUST_LOG=debug cargo run --example tui_e2e
//! ```
//!
//! # Testing Strategy
//!
//! The test execution functions implement several testing patterns:
//!
//! - **Single-station tests**: Validate basic Master/Slave communication
//! - **Multi-station tests**: Test complex scenarios with multiple stations
//! - **Data verification**: Ensure data integrity across all operations
//! - **Transaction safety**: Use retry mechanisms for reliable operations
//!
//! # Performance Considerations
//!
//! - **TUI operations**: 5-45 seconds per station configuration
//! - **CLI operations**: 1-5 seconds per polling/verification cycle
//! - **Total test time**: 25-180 seconds depending on complexity
//! - **Memory usage**: Minimal, primarily for configuration storage

// Re-export all public items from submodules for convenient access
pub mod config;
pub mod execution;
pub mod navigation;
pub mod retry;
mod status_paths;
pub mod validation;

// Re-export commonly used types and functions
#[allow(unused_imports)]
pub use config::{RegisterMode, StationConfig};
#[allow(unused_imports)]
pub use execution::{
    run_multi_station_master_test, run_multi_station_slave_test, run_single_station_master_test,
    run_single_station_slave_test,
};
#[allow(unused_imports)]
pub use validation::*;

// Re-export station configuration helpers
#[allow(unused_imports)]
pub use station::{
    configure_register_count, configure_register_type, configure_start_address,
    configure_station_id, create_station, ensure_connection_mode, initialize_slave_registers,
    save_configuration_and_verify,
};

mod station;
