pub mod cli;
pub mod gui;
pub mod tui;

pub mod i18n;
pub mod protocol;

use anyhow::Result;

/// Common initialization used by both GUI and TUI entrypoints.
pub fn init_common() {
    let _ = env_logger::try_init();
    crate::i18n::init_i18n();
}

pub fn start_gui() -> Result<()> {
    crate::gui::start()?;
    Ok(())
}

pub fn start_tui() -> Result<()> {
    crate::tui::start()?;
    Ok(())
}

/// Simple heuristic to detect if a desktop environment is available.
pub fn is_desktop_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}
