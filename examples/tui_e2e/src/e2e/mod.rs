// TUI E2E test modules
pub mod common;
pub mod multi_station;
pub mod single_station;

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
