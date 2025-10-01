mod deadlock_test;
mod modbus_master_slave;

pub use deadlock_test::test_navigation_to_refresh_no_deadlock;
pub use modbus_master_slave::test_modbus_master_slave_communication;
