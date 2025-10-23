/// Common protocol status types
///
/// This module only exports utility types used across the application.
/// The actual global status structures are defined in their respective modules:
/// - TUI global status: `src/tui/status/global.rs`
/// - CLI status: `src/cli/status/serializable.rs`
pub mod cursor;
pub mod modbus;
pub mod port;
pub mod ui;
