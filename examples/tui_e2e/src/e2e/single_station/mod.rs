// TUI E2E single-station tests
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
