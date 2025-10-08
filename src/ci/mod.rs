//! Re-export test CI utilities from the workspace examples/ci_utils crate
//! so example tests can import `aoba::ci::...`.

pub use ci_utils::auto_cursor;
pub use ci_utils::key_input;
pub use ci_utils::log_utils;
pub use ci_utils::snapshot;
pub use ci_utils::terminal;
pub use ci_utils::utils;

pub use ci_utils::should_run_vcom_tests;
pub use ci_utils::sleep_a_while;
pub use ci_utils::{
    build_debug_bin, execute_cursor_actions, run_binary_sync, spawn_expect_process, ArrowKey,
    CursorAction, ExpectKeyExt, TerminalCapture,
};
