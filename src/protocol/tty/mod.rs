// Platform-dispatched TTY helpers

#[cfg(windows)]
mod tty_windows;
#[cfg(windows)]
pub use tty_windows::available_ports_sorted;
#[cfg(windows)]
pub use tty_windows::try_extract_vid_pid_serial;

#[cfg(unix)]
mod tty_unix;
#[cfg(unix)]
pub use tty_unix::available_ports_sorted;
#[cfg(unix)]
pub use tty_unix::try_extract_vid_pid_serial;

// Fallback for other platforms: expose raw available_ports()
#[cfg(not(any(unix, windows)))]
pub fn available_ports_sorted() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports().unwrap_or_default()
}

#[cfg(not(any(unix, windows)))]
pub fn try_extract_vid_pid_serial(
    _pt: &serialport::SerialPortType,
) -> Option<(u16, u16, Option<String>)> {
    None
}
