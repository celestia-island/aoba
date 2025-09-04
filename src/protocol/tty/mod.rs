// Platform-dispatched TTY helpers

#[cfg(windows)]
mod tty_windows;
#[cfg(windows)]
pub use tty_windows::{available_ports_sorted, available_ports_enriched, try_extract_vid_pid_serial};

#[cfg(unix)]
mod tty_unix;
#[cfg(unix)]
pub use tty_unix::{available_ports_sorted, available_ports_enriched, try_extract_vid_pid_serial};

/// 统一的端口附加元数据结构
#[derive(Debug, Clone, Default)]
pub struct PortExtra {
    pub guid: Option<String>,          // Windows 设备接口 GUID
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

// Fallback for other platforms: expose raw available_ports()
#[cfg(not(any(unix, windows)))]
pub fn available_ports_sorted() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports().unwrap_or_default()
}

#[cfg(not(any(unix, windows)))]
pub fn available_ports_enriched() -> Vec<(serialport::SerialPortInfo, PortExtra)> {
    available_ports_sorted()
        .into_iter()
        .map(|p| (p, PortExtra::default()))
        .collect()
}

#[cfg(not(any(unix, windows)))]
pub fn try_extract_vid_pid_serial(
    _pt: &serialport::SerialPortType,
) -> Option<(u16, u16, Option<String>)> {
    None
}
