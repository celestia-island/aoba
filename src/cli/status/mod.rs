/// CLI status module
///
/// This module provides CLI-specific status structures for E2E testing.
/// Unlike TUI, CLI doesn't maintain a global status tree - it only has
/// serializable status for debug dumps.

pub mod serializable;

// Re-export main types
pub use serializable::CliStatus;
