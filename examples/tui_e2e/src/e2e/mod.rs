// TUI E2E test modules
pub mod basic_master;
pub mod basic_slave;
pub mod single_station;

pub use basic_master::test_tui_master_with_cli_slave_continuous;
pub use basic_slave::test_tui_slave_with_cli_master_continuous;
pub use single_station::{
    test_tui_master_coils, test_tui_master_discrete_inputs, test_tui_master_holding_registers,
    test_tui_master_input_registers, test_tui_slave_coils, test_tui_slave_discrete_inputs,
    test_tui_slave_holding_registers, test_tui_slave_input_registers,
};
