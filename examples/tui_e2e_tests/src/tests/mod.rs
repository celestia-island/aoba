mod modbus_master_slave;
mod hybrid;

pub use modbus_master_slave::test_modbus_master_slave_communication;
pub use hybrid::{test_tui_master_with_cli_slave, test_cli_master_with_tui_slave};
