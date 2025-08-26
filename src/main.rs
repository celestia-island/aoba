#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod gui;
mod tui;

use anyhow::Result;

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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let matches = cli::parse_args();

    if matches.get_flag("gui") {
        log::info!("Forced GUI mode by argument");
        gui::start()?;
    } else if matches.get_flag("tui") {
        log::info!("Forced TUI mode by argument");
        tui::start()?;
    } else if has_desktop_env() {
        log::info!("Desktop environment detected, launching GUI mode");
        gui::start()?;
    } else {
        log::info!("No desktop environment detected, launching TUI mode");
        tui::start()?;
    }

    Ok(())
}
