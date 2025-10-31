//! TUI E2E single-station tests.
//!
//! ## Operation Plan
//!
//! 1. Launch the AOBA TUI with [`common::navigation::setup_tui_test`], which mirrors the
//!    entry flow in `src/tui/mod.rs`, by starting `--tui --debug-ci-e2e-test --no-config-cache`
//!    and moving from the Entry page into the ConfigPanel.
//! 2. Use [`common::navigation::navigate_to_modbus_panel`] to focus the target virtual port and
//!    confirm the page transition handled by `src/tui/ui/pages/modbus_panel`, ensuring we land on
//!    the ModbusDashboard view.
//! 3. Drive [`common::navigation::configure_tui_station`] (which chains helpers such as
//!    `station::ensure_connection_mode` and `station::create_station`) to edit each field with
//!    `execute_with_status_checks`, then save with `Ctrl+S` and assert `ports[].enabled == true`
//!    in the status snapshot so that `src/tui/subprocess.rs` spins up the CLI subprocess.
//! 4. For the Master scenario, keep the session alive long enough for the CLI `MasterProvide`
//!    subprocess to start, then call [`common::execution::verify_master_data`] from `port2` to run
//!    a one-shot `--slave-poll` health check against the provided data source.
//! 5. For the Slave scenario, populate deterministic register values via
//!    `initialize_slave_registers`, push data toward the TUI `SlavePoll` subprocess with
//!    [`common::execution::send_data_from_cli_master`], and confirm persistence through
//!    [`common::execution::verify_slave_data`].
//! 6. Follow the "one action, one verification" guidance from
//!    `examples/tui_e2e/MIGRATION_GUIDE.md`, binding a status check to every key sequence; if a
//!    serial handshake fails, immediately rerun `scripts/socat_init.sh` (no sudo required) to
//!    rebuild the virtual port pair before retrying.

pub mod master_modes;
pub mod slave_modes;

pub use master_modes::{
    test_tui_master_coils, test_tui_master_discrete_inputs, test_tui_master_holding_registers,
    test_tui_master_input_registers,
};
pub use slave_modes::{
    test_tui_slave_coils, test_tui_slave_discrete_inputs, test_tui_slave_holding_registers,
    test_tui_slave_input_registers,
};
