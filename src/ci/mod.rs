//! Test CI helpers (hidden from docs)
#![doc(hidden)]

pub mod snapshot;
pub mod terminal;
pub mod utils;

pub use snapshot::TerminalCapture;
pub use terminal::{build_debug_bin, run_binary_sync, spawn_expect_process};
pub use utils::*;
