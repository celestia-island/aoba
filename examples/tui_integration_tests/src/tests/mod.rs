mod deadlock_test;
mod navigation;
mod port_list_scrolling;
mod serial_interaction;
mod startup_shutdown;

pub use deadlock_test::{test_navigation_to_refresh_no_deadlock, test_navigation_with_no_ports};
pub use navigation::test_tui_navigation;
pub use port_list_scrolling::test_port_list_scrolling;
pub use serial_interaction::test_tui_serial_port_interaction;
pub use startup_shutdown::test_tui_startup_shutdown;
