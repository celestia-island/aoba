// Test file to verify recursive merging

// Before: Multiple crate::tui::* imports that should be merged
use crate::tui::{
    cli_data::initialize_cli_data_source,
    ipc::handle_cli_ipc_message,
    logs::{append_log, get_logs},
    status::{
        port::{PortConfig, PortData},
        Status, TuiStatus,
    },
};

fn main() {
    println!("Test");
}
