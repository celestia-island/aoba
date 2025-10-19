//! Test CI helpers (hidden from docs)
#![doc(hidden)]

pub mod auto_cursor;
pub mod cli;
pub mod data;
pub mod helpers;
pub mod key_input;
pub mod log_parser;
pub mod log_utils;
pub mod ports;
pub mod snapshot;
pub mod terminal;
pub mod tui;
pub mod verify;

pub use auto_cursor::{execute_cursor_actions, CursorAction};
pub use cli::{create_modbus_command, run_cli_slave_poll};
pub use data::{generate_random_coils, generate_random_registers};
pub use helpers::{sleep_a_while, sleep_seconds};
pub use key_input::{ArrowKey, ExpectKeyExt};
pub use log_parser::{
    get_latest_state, get_port_state, parse_state_dumps, verify_port_exists, wait_for_page,
    wait_for_port_state, ConfigEditState, PortState, StateDump,
};
pub use ports::{
    port_exists, should_run_vcom_tests, should_run_vcom_tests_with_ports, vcom_matchers,
    vcom_matchers_with_ports, VcomMatchers, DEFAULT_PORT1, DEFAULT_PORT2,
};
pub use snapshot::TerminalCapture;
pub use terminal::{build_debug_bin, run_binary_sync, spawn_expect_process};
pub use tui::{
    check_status_indicator, enable_port_carefully, enter_modbus_panel, navigate_to_vcom,
    update_tui_registers, verify_port_enabled,
};
pub use verify::{verify_cli_output, verify_continuous_data};
