// CLI E2E multi-station tests (2 stations)
pub mod two_stations;

pub use two_stations::{
    test_multi_station_mixed_register_types, test_multi_station_mixed_station_ids,
    test_multi_station_spaced_addresses,
};
