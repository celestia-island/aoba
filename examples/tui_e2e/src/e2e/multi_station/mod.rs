// TUI E2E multi-station tests (2 stations)
pub mod master_modes;
pub mod slave_modes;

pub use master_modes::{
    test_tui_multi_master_mixed_register_types, test_tui_multi_master_mixed_station_ids,
    test_tui_multi_master_spaced_addresses,
};
pub use slave_modes::{
    test_tui_multi_slave_mixed_register_types, test_tui_multi_slave_mixed_station_ids,
    test_tui_multi_slave_spaced_addresses,
};
