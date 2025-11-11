pub mod input;
pub mod status;
pub mod ui;
pub mod utils;

// Split implementation into smaller modules for maintainability
pub mod ipc;
pub mod logs;
pub mod rendering;
pub mod runtime;
pub mod status_utils;

// Re-export modules from core for backwards compatibility
pub use crate::core::{cli_data, helpers, persistence, subprocess};

// Re-export Page for convenience since it's used throughout TUI code
pub use status::Page;

// Re-export functions from submodules so existing callers continue to work
pub(crate) use logs::*;
pub(crate) use runtime::*;

// Export test-only rendering helper for examples and tests that render via TestBackend
pub use rendering::render_ui_for_testing;
