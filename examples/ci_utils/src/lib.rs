//! Test CI helpers (hidden from docs)
#![doc(hidden)]

pub mod auto_cursor;
pub mod key_input;
pub mod log_utils;
pub mod snapshot;
pub mod terminal;
// `utils` module removed; prefer specific helper modules (data, ports, tui, etc.).
pub mod cli;
pub mod data;
pub mod helpers;
pub mod ports;
pub mod tui;
pub mod verify;

pub use auto_cursor::{execute_cursor_actions, CursorAction};
pub use cli::{create_modbus_command, run_cli_slave_poll};
pub use data::{generate_random_coils, generate_random_registers};
pub use helpers::sleep_a_while;
pub use key_input::{ArrowKey, ExpectKeyExt};
pub use ports::{should_run_vcom_tests, vcom_matchers, VcomMatchers};
pub use snapshot::TerminalCapture;
pub use terminal::{build_debug_bin, run_binary_sync, spawn_expect_process};
pub use tui::{enable_port_carefully, navigate_to_vcom, update_tui_registers};
pub use verify::{verify_cli_output, verify_continuous_data};
