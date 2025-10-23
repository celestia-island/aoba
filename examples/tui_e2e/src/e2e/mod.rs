// TUI E2E test modules
pub mod basic_master;
pub mod basic_slave;

pub use basic_master::test_tui_master_with_cli_slave_continuous;
pub use basic_slave::test_tui_slave_with_cli_master_continuous;
