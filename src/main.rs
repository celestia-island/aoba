use anyhow::Result;
use std::process::Command;

#[cfg(target_os = "windows")]
fn launched_from_explorer() -> bool {
    // Detect if parent process is explorer.exe by scanning processes and checking parent PID.
    unsafe {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::*;

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) == 0 {
            CloseHandle(snapshot);
            return false;
        }

        let current_pid = std::process::id() as u32;
        let mut parent_pid: u32 = 0;
        loop {
            if entry.th32ProcessID == current_pid {
                parent_pid = entry.th32ParentProcessID;
                break;
            }
            if Process32NextW(snapshot, &mut entry) == 0 {
                break;
            }
        }

        if parent_pid == 0 {
            CloseHandle(snapshot);
            return false;
        }

        // Find parent process entry to read its exe name
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut parent_name = String::new();
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == parent_pid {
                    let wide: &[u16] = &entry.szExeFile;
                    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
                    parent_name = String::from_utf16_lossy(&wide[..len]).to_lowercase();
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        parent_name.ends_with("explorer.exe")
    }
}

#[cfg(not(target_os = "windows"))]
fn launched_from_explorer() -> bool {
    false
}

fn spawn_gui_next_to_current_exe() {
    println!("Launching AOBA GUI...");

    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.set_file_name("aoba_gui.exe");
        match Command::new(&exe_path).spawn() {
            Ok(mut child) => {
                // Give the GUI process a short time to fail fast. If it exits quickly,
                // assume startup failed and keep the console visible so the user can see errors.
                use std::{thread, time::Duration};

                let mut failed_early = false;
                for _ in 0..30 {
                    thread::sleep(Duration::from_millis(100));
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            eprintln!("GUI process exited early with status={}", status);
                            failed_early = true;
                            break;
                        }
                        Ok(None) => continue,
                        Err(err) => {
                            eprintln!("Failed to query GUI process status: {}", err);
                            failed_early = true;
                            break;
                        }
                    }
                }

                if failed_early {
                    eprintln!("GUI failed to start; check the logs.");

                    if launched_from_explorer() {
                        // When started from Explorer (double-click), show a dialog and keep console open so the
                        // user can see the error details.
                        eprintln!("Press Enter to exit...");
                        let mut _buf = String::new();
                        let _ = std::io::stdin().read_line(&mut _buf);
                        std::process::exit(1);
                    } else {
                        // Non-explorer (terminal/CI): just print and exit immediately.
                        std::process::exit(1);
                    }
                }
            }
            Err(err) => {
                // Spawn failure: stderr bilingual
                eprintln!("Failed to spawn GUI '{}': {}", exe_path.display(), err);

                if launched_from_explorer() {
                    eprintln!("Press Enter to exit...");
                    let mut _buf = String::new();
                    let _ = std::io::stdin().read_line(&mut _buf);
                    std::process::exit(1);
                } else {
                    std::process::exit(1);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Console launcher: keep it simple and let the OS/terminal manage stdio.
    aoba::init_common();

    let matches = aoba::cli::parse_args();

    // One-shot actions (e.g., --list-ports). If handled, exit.
    if aoba::cli::actions::run_one_shot_actions(&matches) {
        return Ok(());
    }

    // If TUI requested, run in this process so it inherits the terminal.
    if matches.get_flag("tui") {
        aoba::start_tui()?;
        return Ok(());
    }

    // If GUI requested, spawn the GUI binary (windows_subsystem) next to this exe.
    if matches.get_flag("gui") {
        spawn_gui_next_to_current_exe();
        return Ok(());
    }

    // Default: prefer GUI when desktop available, otherwise TUI.
    if aoba::is_desktop_available() {
        spawn_gui_next_to_current_exe();
    } else {
        aoba::start_tui()?;
    }

    Ok(())
}
