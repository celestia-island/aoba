mod basic_master;
mod basic_slave;
mod cli_port_cleanup;
pub mod e2e;

pub use basic_master::test_tui_master_with_cli_slave_continuous;
pub use basic_slave::test_tui_slave_with_cli_master_continuous;
pub use cli_port_cleanup::test_cli_port_release;
