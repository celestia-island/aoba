/// Core business logic for Aoba
///
/// This package contains UI-independent business logic including:
/// - Subprocess management for CLI workers
/// - Message bus for UI-core communication
/// - Core runtime loop logic
/// - Logging infrastructure
///
/// This separation allows multiple UI frontends (TUI, GUI, WebUI) to share
/// the same core functionality.
pub mod bus;
pub mod logs;
pub mod runtime;
pub mod subprocess;

// Re-export commonly used types
pub use bus::{Bus, CoreToUi, UiToCore};
pub use runtime::{CoreContext, CoreRuntimeConfig, RuntimeStartConfig};
pub use subprocess::{CliSubprocessConfig, SubprocessManager};
