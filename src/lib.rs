pub mod cli;
pub mod core;
pub mod protocol;
pub mod tui;
pub mod utils;

use anyhow::Result;
use chrono::Local;
use log::LevelFilter;
use std::io::{self, Write};

use clap::ArgMatches;
use env_logger::{Builder, Target};

/// Common initialization used by TUI entrypoint.
pub fn init_common() {
    let log_file = std::env::var("AOBA_LOG_FILE").ok().or_else(|| {
        #[cfg(debug_assertions)]
        {
            Some(format!("./log_{}.log", Local::now().format("%Y%m%d%H%M%S")))
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    });

    if let Some(path) = log_file {
        if let Err(err) = init_file_logger(&path) {
            eprintln!("Failed to initialize file logger at '{path}': {err}");
            env_logger::init();
        }
    } else {
        env_logger::init();
    }

    utils::i18n::init_i18n();
}

pub async fn start_tui(matches: &ArgMatches) -> Result<()> {
    tui::start(matches).await?;
    Ok(())
}

fn init_file_logger(path: &str) -> io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    let mut builder = Builder::new();
    builder
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
        .target(Target::Pipe(Box::new(file)))
        .filter_level(LevelFilter::Debug)
        .parse_default_env()
        .init();

    log::info!("File logger initialized at {path}");

    Ok(())
}
