// CLI E2E single-station tests for different register modes
pub mod register_modes;

pub use register_modes::{
    test_single_station_coils, test_single_station_discrete_inputs,
    test_single_station_holding_registers, test_single_station_input_registers,
};
