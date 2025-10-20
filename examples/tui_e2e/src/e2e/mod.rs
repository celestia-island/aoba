// TUI E2E test modules
pub mod basic_master;
pub mod basic_slave;
pub mod multi_masters;
pub mod multi_slaves;

pub use basic_master::test_tui_master_with_cli_slave_continuous;
pub use basic_slave::test_tui_slave_with_cli_master_continuous;
pub use multi_masters::test_tui_multi_masters;
pub use multi_slaves::test_tui_multi_slaves;
