mod detection;
mod startup_ctrlc;
mod virtual_ports;

pub use detection::test_tui_startup_detection;
pub use startup_ctrlc::test_tui_startup_ctrl_c_exit;
pub use virtual_ports::test_tui_with_virtual_ports;
