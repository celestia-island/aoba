pub mod cli;
pub mod tui;

pub mod i18n;
pub mod protocol;

use anyhow::Result;
use clap::ArgMatches;

#[cfg(debug_assertions)]
use chrono::Local;
#[cfg(debug_assertions)]
use std::io::Write;

#[cfg(debug_assertions)]
use env_logger::Builder;
#[cfg(debug_assertions)]
use log::LevelFilter;

/// Common initialization used by TUI entrypoint.
pub fn init_common() {
    #[cfg(not(debug_assertions))]
    env_logger::init();

    #[cfg(debug_assertions)]
    {
        // Check for AOBA_LOG_FILE environment variable
        let log_path = std::env::var("AOBA_LOG_FILE").unwrap_or_else(|_| "./log.log".to_string());
        let target = Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .expect("Can't open log file"),
        );
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

pub fn start_tui(matches: &ArgMatches) -> Result<()> {
    crate::tui::start(matches)?;
    Ok(())
}
