/// Common protocol status types
///
/// This module only exports utility types used across the application.
/// The actual global status structures are defined in their respective modules:
/// - TUI global status: `src/tui/global_status.rs`
/// - CLI status: `src/cli/status.rs`
pub mod cursor;
pub mod modbus;
pub mod port;
pub mod ui;

// Re-export Page, Status, and ErrorInfo from their new locations for backward compatibility
pub use crate::tui::global_status::{ErrorInfo, Page, Status};
