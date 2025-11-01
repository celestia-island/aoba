pub mod detailed_workflow;
pub mod isomorphic_workflow;
pub mod modbus;
pub mod setup;
pub mod workflow;

pub use detailed_workflow::configure_stations_with_screenshots as configure_stations_detailed;
pub use isomorphic_workflow::configure_stations_with_screenshots;
pub use modbus::navigate_to_modbus_panel;
pub use setup::setup_tui_test;
pub use workflow::configure_tui_station;
