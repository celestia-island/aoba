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
pub mod status_monitor;
pub mod terminal;
pub mod tui;
pub mod verify;

pub use auto_cursor::{execute_cursor_actions, execute_with_status_checks, CursorAction};
pub use cli::{create_modbus_command, run_cli_slave_poll};
pub use data::{generate_random_coils, generate_random_registers};
pub use helpers::{sleep_1s, sleep_3s, terminate_session};
pub use key_input::{ArrowKey, ExpectKeyExt};
pub use log_parser::{
    get_latest_state, get_port_state, parse_state_dumps, verify_port_exists, wait_for_page,
    wait_for_port_state, ConfigEditState, PortState, StateDump,
};
pub use ports::{
    port_exists, should_run_vcom_tests_with_ports, vcom_matchers_with_ports, VcomMatchers,
    DEFAULT_PORT1, DEFAULT_PORT2,
};
pub use snapshot::{log_last_terminal_snapshot, TerminalCapture, TerminalSize};
pub use status_monitor::{
    get_port_log_count, port_exists_in_tui, read_cli_status, read_tui_status, wait_for_cli_status,
    wait_for_modbus_config, wait_for_port_enabled, wait_for_tui_page, CliMode, CliStatus,
    PortState as E2EPortState, TuiModbusMaster, TuiModbusSlave, TuiPage, TuiPort, TuiStatus,
};
pub use terminal::{
    build_debug_bin, run_binary_sync, spawn_expect_process, spawn_expect_process_with_size,
};
pub use tui::{
    check_status_indicator, enable_port_carefully, enter_modbus_panel, navigate_to_vcom,
    update_tui_registers, verify_port_enabled,
};
pub use verify::{verify_cli_output, verify_continuous_data};
