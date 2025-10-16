mod config_mode;
mod help;
mod list_ports;
mod list_ports_json;
mod list_ports_status;
mod modbus_cli;
mod modbus_e2e;

pub use config_mode::test_config_mode;
pub use help::test_cli_help;
pub use list_ports::test_cli_list_ports;
pub use list_ports_json::test_cli_list_ports_json;
pub use list_ports_status::test_cli_list_ports_json_with_status;
pub use modbus_cli::{
    test_master_provide_persist, test_master_provide_temp, test_slave_listen_persist,
    test_slave_listen_temp,
};
pub use modbus_e2e::{
    test_master_provide_with_vcom, test_master_slave_communication, test_slave_listen_with_vcom,
};

// E2E tests
pub mod e2e;
pub use e2e::basic::test_basic_master_slave_communication;
pub use e2e::multi_masters::{test_multi_masters, test_multi_masters_same_station};
pub use e2e::multi_slaves::{
    test_multi_slaves, test_multi_slaves_adjacent_registers, test_multi_slaves_same_station,
};
