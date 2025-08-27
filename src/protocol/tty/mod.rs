// Platform-dispatched TTY helpers

#[cfg(windows)]
mod tty_windows;
#[cfg(windows)]
pub use tty_windows::available_ports_sorted;

#[cfg(unix)]
mod tty_unix;
#[cfg(unix)]
pub use tty_unix::available_ports_sorted;

// Fallback for other platforms: expose raw available_ports()
#[cfg(not(any(unix, windows)))]
pub fn available_ports_sorted() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports().unwrap_or_default()
}
