#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod gui;
mod i18n;
mod tui;

use anyhow::Result;

#[cfg(windows)]
fn ensure_console() -> Option<ConsoleGuard> {
    // returns Some(ConsoleGuard) if attached or allocated (guard handles cleanup if needed)
    use windows_sys::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            // no parent, allocate
            if AllocConsole() != 0 {
                // redirect stdio to the new console, but save originals
                if let Ok(guard) = ConsoleGuard::new(true) {
                    return Some(guard);
                }
            }
        } else {
            // attached to parent console; return guard that does not free
            if let Ok(guard) = ConsoleGuard::new(false) {
                return Some(guard);
            }
        }
    }
    None
}

#[cfg(windows)]
fn redirect_stdio_to_console() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    extern "C" {
        fn _open_osfhandle(osfhandle: isize, flags: i32) -> i32;
    }

    unsafe {
        let out_h = GetStdHandle(STD_OUTPUT_HANDLE);
        if out_h != std::ptr::null_mut() && out_h != INVALID_HANDLE_VALUE {
            let fd = _open_osfhandle(out_h as isize, 0);
            if fd >= 0 {
                libc::dup2(fd, 1);
            }
        }

        let err_h = GetStdHandle(STD_ERROR_HANDLE);
        if err_h != std::ptr::null_mut() && err_h != INVALID_HANDLE_VALUE {
            let fd = _open_osfhandle(err_h as isize, 0);
            if fd >= 0 {
                libc::dup2(fd, 2);
            }
        }

        let in_h = GetStdHandle(STD_INPUT_HANDLE);
        if in_h != std::ptr::null_mut() && in_h != INVALID_HANDLE_VALUE {
            let fd = _open_osfhandle(in_h as isize, 0);
            if fd >= 0 {
                libc::dup2(fd, 0);
            }
        }
    }
}

#[cfg(not(windows))]
fn redirect_stdio() {}

#[cfg(not(windows))]
fn ensure_console() {}

/// Detect if desktop environment is available (simple check for Windows/macOS/Linux)
fn has_desktop_env() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Windows usually has desktop environment
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        // Check DISPLAY or WAYLAND_DISPLAY env variable
        return std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return false;
    }
}

#[cfg(windows)]
struct ConsoleGuard {
    allocated: bool,
    orig_stdin: i32,
    orig_stdout: i32,
    orig_stderr: i32,
}

#[cfg(windows)]
impl ConsoleGuard {
    fn new(allocated: bool) -> Result<ConsoleGuard, ()> {
        // duplicate current std fds so we can restore later
        unsafe {
            let orig_stdin = libc::dup(0);
            let orig_stdout = libc::dup(1);
            let orig_stderr = libc::dup(2);
            // redirect to console handles
            redirect_stdio_to_console();
            Ok(ConsoleGuard {
                allocated,
                orig_stdin,
                orig_stdout,
                orig_stderr,
            })
        }
    }
}

#[cfg(windows)]
impl Drop for ConsoleGuard {
    fn drop(&mut self) {
        unsafe {
            // restore original fds
            if self.orig_stdin >= 0 {
                libc::dup2(self.orig_stdin, 0);
                libc::close(self.orig_stdin);
            }
            if self.orig_stdout >= 0 {
                libc::dup2(self.orig_stdout, 1);
                libc::close(self.orig_stdout);
            }
            if self.orig_stderr >= 0 {
                libc::dup2(self.orig_stderr, 2);
                libc::close(self.orig_stderr);
            }

            if self.allocated {
                use windows_sys::Win32::System::Console::FreeConsole;
                FreeConsole();
            }
        }
    }
}

#[cfg(not(windows))]
struct ConsoleGuard;

#[cfg(not(windows))]
impl ConsoleGuard {
    fn new(_allocated: bool) -> Result<ConsoleGuard, ()> {
        Ok(ConsoleGuard)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // init translations
    crate::i18n::init_i18n();
    let matches = cli::parse_args();

    if matches.get_flag("gui") {
        log::info!("Forced GUI mode by argument");
        gui::start()?;
    } else if matches.get_flag("tui") {
        log::info!("Forced TUI mode by argument");
        let _guard = ensure_console();
        tui::start()?;
    } else if has_desktop_env() {
        log::info!("Desktop environment detected, launching GUI mode");
        gui::start()?;
    } else {
        log::info!("No desktop environment detected, launching TUI mode");
        let _guard = ensure_console();
        tui::start()?;
    }

    Ok(())
}
