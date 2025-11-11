pub mod input;
pub mod persistence;
pub mod status;
pub mod ui;
pub mod utils;

// Split implementation into smaller modules for maintainability
pub mod cli_data;
pub mod helpers;
pub mod ipc;
pub mod logs;
pub mod rendering;
pub mod runtime;
pub mod status_utils;

// Re-export subprocess from core for backwards compatibility
pub use aoba_core::subprocess;

// Re-export Page for convenience since it's used throughout TUI code
pub use status::Page;

// Re-export functions from submodules so existing callers continue to work
pub(crate) use helpers::*;
pub(crate) use logs::*;
pub(crate) use runtime::*;

// Export test-only rendering helper for examples and tests that render via TestBackend
pub use rendering::render_ui_for_testing;
