/// Common protocol status types
///
/// This module only exports utility types used across the application.
/// The actual global status structures are defined in their respective modules:
/// - TUI global status: `src/tui/status.rs`
/// - CLI serializable status: `src/protocol/status/types/cli.rs`
pub mod cli;
pub mod cursor;
pub mod modbus;
pub mod port;
pub mod ui;

pub use cli::{CliMode, CliStatus};
pub use modbus::RegisterMode;
