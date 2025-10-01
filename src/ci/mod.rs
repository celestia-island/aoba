//! Test CI helpers (hidden from docs)
#![doc(hidden)]

pub mod auto_cursor;
pub mod key_input;
pub mod snapshot;
pub mod terminal;
pub mod utils;

pub use auto_cursor::{execute_cursor_actions, CursorAction};
pub use key_input::{ArrowKey, ExpectKeyExt};
pub use snapshot::TerminalCapture;
pub use terminal::{build_debug_bin, run_binary_sync, spawn_expect_process};
pub use utils::*;
