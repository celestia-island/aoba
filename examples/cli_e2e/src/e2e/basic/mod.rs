// Basic CLI E2E tests
pub mod basic_master_slave;
pub mod data_source_manual;

pub use basic_master_slave::test_basic_master_slave_communication;
pub use data_source_manual::{test_ipc_pipe_data_source, test_manual_data_source};
