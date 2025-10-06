// Hybrid tests combining TUI with CLI
// These tests use CLI commands to interact with TUI instances for easier automation

mod cli_master_tui_slave;
mod tui_master_cli_slave;

pub use cli_master_tui_slave::test_cli_master_with_tui_slave;
pub use tui_master_cli_slave::test_tui_master_with_cli_slave;
