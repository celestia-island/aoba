use anyhow::{anyhow, Result};
use std::time::Duration;

/// Open a serial port with the requested timeout, enabling exclusive access on Unix systems.
pub fn open_serial_port(
    port: &str,
    baud_rate: u32,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>> {
    use crate::protocol::status::types::port::PortType;

    let port_type = PortType::detect(port);
    if port_type.is_virtual() {
        return Err(anyhow!(
            "Port {} is a virtual port (type: {}). Virtual ports cannot be opened as physical serial ports. \
            Use IPC or HTTP communication methods instead.",
            port,
            port_type
        ));
    }

    let builder = serialport::new(port, baud_rate).timeout(timeout);

    #[cfg(unix)]
    {
        let mut handle = builder
            .open_native()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;
        handle
            .set_exclusive(true)
            .map_err(|err| anyhow!("Failed to acquire exclusive access to {port}: {err}"))?;
        Ok(Box::new(handle))
    }

    #[cfg(not(unix))]
    {
        builder
            .open()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))
    }
}

/// Check whether a serial port is currently held by another process.
///
/// Returns `true` if the port is open by some other process, `false` if it is
/// free or cannot be determined. Virtual ports (IPC/HTTP) always return `false`.
///
/// # Platform behaviour
///
/// - **Windows**: attempts an exclusive `CreateFileW`; checks for
///   `ERROR_SHARING_VIOLATION` / `ERROR_ACCESS_DENIED`.
/// - **Unix**: resolves the canonical device path, then walks `/proc/*/fd`
///   comparing device IDs.
pub fn is_port_occupied(port_name: &str) -> bool {
    use crate::protocol::status::types::port::PortType;

    let port_type = PortType::detect(port_name);
    if port_type.is_virtual() {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{CloseHandle, GetLastError, WIN32_ERROR},
                Storage::FileSystem::{
                    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
                    FILE_SHARE_NONE, OPEN_EXISTING,
                },
            },
        };

        let device_path = if port_name.starts_with("\\\\.\\") {
            port_name.to_string()
        } else {
            format!("\\\\.\\{port_name}")
        };

        let wide_path: Vec<u16> = device_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            );

            if handle.is_err() {
                let error = GetLastError();
                return matches!(error, WIN32_ERROR(32) | WIN32_ERROR(5));
            }

            let handle = handle.unwrap();
            let _ = CloseHandle(handle);
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::fs;
        use std::os::unix::fs::MetadataExt;
        use std::path::{Path, PathBuf};

        fn canonical_device_path(port_path: &str) -> Option<PathBuf> {
            match fs::canonicalize(port_path) {
                Ok(path) => Some(path),
                Err(_) => {
                    let candidate = Path::new(port_path);
                    if candidate.is_absolute() {
                        Some(candidate.to_path_buf())
                    } else {
                        std::env::current_dir().ok().map(|cwd| cwd.join(candidate))
                    }
                }
            }
        }

        let target_path = match canonical_device_path(port_name) {
            Some(p) => p,
            None => return false,
        };

        let target_rdev = match fs::metadata(&target_path) {
            Ok(meta) => meta.rdev(),
            Err(_) => return false,
        };

        if target_rdev == 0 {
            return false;
        }

        let self_pid = std::process::id();
        if let Ok(proc_entries) = fs::read_dir("/proc") {
            for entry in proc_entries.flatten() {
                let file_name = entry.file_name();
                let pid: u32 = match file_name.to_string_lossy().parse() {
                    Ok(pid) => pid,
                    Err(_) => continue,
                };

                if pid == self_pid {
                    continue;
                }

                let fd_dir = entry.path().join("fd");
                let fd_iter = match fs::read_dir(&fd_dir) {
                    Ok(iter) => iter,
                    Err(_) => continue,
                };

                for fd_entry in fd_iter.flatten() {
                    let fd_path = fd_entry.path();

                    if let Ok(meta) = fs::metadata(&fd_path) {
                        if meta.rdev() == target_rdev {
                            return true;
                        }
                    }

                    if let Ok(link) = fs::read_link(&fd_path) {
                        if let Ok(canon) = fs::canonicalize(&link) {
                            if canon == target_path {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}
