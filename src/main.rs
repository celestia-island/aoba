use anyhow::Result;
use std::process::Command;

// Try to spawn the GUI next to the current exe. Return true if GUI started successfully,
// False if spawning or early startup failed (caller may fallback to TUI).
fn spawn_gui_next_to_current_exe() -> bool {
    println!("Launching AOBA GUI...");

    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.set_file_name("aoba_gui.exe");
        match Command::new(&exe_path).spawn() {
            Ok(mut child) => {
                // Give the GUI process a short time to fail fast. If it exits quickly,
                // Assume startup failed and return false so caller can fallback to TUI.
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
                        Ok(_) => continue,
                        Err(err) => {
                            eprintln!("Failed to query GUI process status: {}", err);
                            failed_early = true;
                            break;
                        }
                    }
                }

                if failed_early {
                    eprintln!("GUI failed to start; falling back to TUI.");
                    return false;
                }

                // GUI appears to be running.
                return true;
            }
            Err(err) => {
                eprintln!("Failed to spawn GUI '{}': {}", exe_path.display(), err);
                eprintln!("Falling back to TUI.");
                return false;
            }
        }
    }

    // Couldn't determine current exe or other unexpected issue: fallback.
    eprintln!("Unable to locate GUI executable; falling back to TUI.");
    false
}

#[tokio::main]
async fn main() -> Result<()> {
    // Console launcher: keep it simple and let the OS / terminal manage stdio.
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

    // If GUI requested, try to spawn the GUI binary next to this exe. If it fails, fallback to TUI.
    if matches.get_flag("gui") {
        if spawn_gui_next_to_current_exe() {
            return Ok(());
        } else {
            // Fall through to start TUI
            aoba::start_tui()?;
            return Ok(());
        }
    }

    // Default: prefer GUI when desktop available, otherwise TUI.
    if aoba::is_desktop_available() {
        if !spawn_gui_next_to_current_exe() {
            // GUI failed to start; fallback to TUI in the same process.
            aoba::start_tui()?;
        }
    } else {
        aoba::start_tui()?;
    }

    Ok(())
}
