use anyhow::Result;
use chrono::Local;
use log::LevelFilter;
use std::io::{self, Write};

use clap::ArgMatches;
use env_logger::{Builder, Target};

/// Multi-writer for logging to both file and stdout
struct DualWriter {
    file: std::fs::File,
    stdout: io::Stdout,
}

impl Write for DualWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write_all(buf)?;
        self.stdout.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()?;
        self.stdout.flush()?;
        Ok(())
    }
}

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

    crate::utils::i18n::init_i18n();
}

pub async fn start_tui(matches: &ArgMatches) -> Result<()> {
    crate::tui::start(matches).await?;
    Ok(())
}

pub async fn start_daemon(matches: &ArgMatches) -> Result<()> {
    crate::tui::daemon::start_daemon(matches).await?;
    Ok(())
}

/// Initialize dual logger for daemon mode (outputs to both file and terminal)
pub fn init_daemon_logger(path: &str) -> io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    let dual_writer = DualWriter {
        file,
        stdout: io::stdout(),
    };

    let mut builder = Builder::new();
    builder
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .target(Target::Pipe(Box::new(dual_writer)))
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    log::info!("Daemon logger initialized - logging to file and terminal");

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
