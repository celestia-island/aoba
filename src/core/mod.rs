/// Core business logic for Aoba
///
/// This package contains UI-independent business logic including:
/// - Subprocess management for CLI workers
/// - Message bus for UI-core communication
/// - Core runtime loop logic
/// - Logging infrastructure
/// - CLI data source management
/// - Configuration persistence
/// - Helper utilities
///
/// This separation allows multiple UI frontends (TUI, GUI, WebUI) to share
/// the same core functionality.
pub mod bus;
pub mod cli_data;
pub mod helpers;
pub mod logs;
pub mod persistence;
pub mod runtime;
pub mod subprocess;
pub mod task_manager;

// Note: port_scan and ipc modules remain in TUI for now as they have tight
// coupling with TUI-specific status types. They will be refactored in a future iteration.

// Re-export commonly used types
pub use bus::{Bus, CoreToUi, UiToCore};
pub use runtime::{CoreContext, CoreRuntimeConfig, RuntimeStartConfig};
pub use subprocess::{CliSubprocessConfig, SubprocessManager};
