/// Unified type for vid/pid/serial extraction: (vid, pid, serial, manufacturer, product)
pub type VidPidSerial = (u16, u16, Option<String>, Option<String>, Option<String>);

#[cfg(windows)]
mod tty_windows;
#[cfg(windows)]
pub use tty_windows::{
    available_ports_enriched, available_ports_sorted, try_extract_vid_pid_serial,
};

#[cfg(unix)]
mod tty_unix;
#[cfg(unix)]
pub use tty_unix::{available_ports_enriched, available_ports_sorted, try_extract_vid_pid_serial};

/// Unified per-port extra metadata structure
#[derive(Debug, Clone, Default)]
pub struct PortExtra {
    pub guid: Option<String>, // Windows device interface GUID
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
pub fn try_extract_vid_pid_serial(_pt: &serialport::SerialPortType) -> Option<VidPidSerial> {
    None
}
