// TUI E2E test modules
pub mod basic_master;
pub mod basic_slave;
pub mod multi_station;
pub mod single_station;

pub use basic_master::test_tui_master_with_cli_slave_continuous;
pub use basic_slave::test_tui_slave_with_cli_master_continuous;
pub use multi_station::{
    test_tui_multi_master_mixed_register_types, test_tui_multi_master_mixed_station_ids,
    test_tui_multi_master_spaced_addresses, test_tui_multi_slave_mixed_register_types,
    test_tui_multi_slave_mixed_station_ids, test_tui_multi_slave_spaced_addresses,
};
pub use single_station::{
    test_tui_master_coils, test_tui_master_discrete_inputs, test_tui_master_holding_registers,
    test_tui_master_input_registers, test_tui_slave_coils, test_tui_slave_discrete_inputs,
    test_tui_slave_holding_registers, test_tui_slave_input_registers,
};
