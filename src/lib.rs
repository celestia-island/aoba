pub mod cli;
pub mod gui;
pub mod tui;

pub mod i18n;
pub mod protocol;

#[doc(hidden)]
pub mod ci;

use anyhow::Result;

#[cfg(debug_assertions)]
use chrono::Local;
#[cfg(debug_assertions)]
use std::{fs::File, io::Write};

#[cfg(debug_assertions)]
use env_logger::Builder;
#[cfg(debug_assertions)]
use log::LevelFilter;

/// Common initialization used by both GUI and TUI entrypoints.
pub fn init_common() {
    #[cfg(not(debug_assertions))]
    env_logger::init();

    #[cfg(debug_assertions)]
    {
        let target = Box::new(File::create("./log.log").expect("Can't create file"));
        Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} {} [{}] - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                    record.level(),
                    record.args()
                )
            })
            .target(env_logger::Target::Pipe(target))
            .filter(None, LevelFilter::Debug)
            .init();
    }

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
