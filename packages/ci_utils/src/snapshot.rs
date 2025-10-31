use anyhow::Result;

use expectrl::{process::NonBlocking, Expect};
use vt100::Parser;

use crate::helpers::sleep_1s;
use std::{
    io,
    sync::{Mutex, OnceLock},
};

type SnapshotRecord = (String, String);

fn snapshot_store() -> &'static Mutex<Option<SnapshotRecord>> {
    static STORE: OnceLock<Mutex<Option<SnapshotRecord>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn update_last_snapshot(step_description: &str, screen: &str) {
    let record = (step_description.to_string(), screen.to_string());
    if let Ok(mut guard) = snapshot_store().lock() {
        *guard = Some(record);
    } else {
        log::warn!("Failed to update last terminal snapshot due to poisoned mutex");
    }
}

/// Log the most recent captured terminal screen to assist debugging failures.
pub fn log_last_terminal_snapshot(context: &str) {
    match snapshot_store().lock() {
        Ok(guard) => {
            if let Some((step, screen)) = guard.as_ref() {
                log::error!("❌ {context}: last captured screen at '{step}'\n{screen}");
            } else {
                log::error!("❌ {context}: no terminal snapshot captured yet");
            }
        }
        Err(err) => {
            log::error!(
                "❌ {context}: unable to retrieve terminal snapshot (mutex poisoned: {err})"
            );
        }
    }
}

/// Standard terminal sizes for E2E tests
#[derive(Debug, Clone, Copy)]
pub enum TerminalSize {
    /// Small terminal: 24 rows x 80 columns (for basic tests with few stations)
    Small,
    /// Large terminal: 60 rows x 80 columns (for multi-station tests)
    Large,
    /// Extra large terminal: 80 rows x 80 columns (for extensive multi-station tests)
    ExtraLarge,
}

impl TerminalSize {
    /// Get the (rows, cols) dimensions for this terminal size
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            TerminalSize::Small => (24, 80),
            TerminalSize::Large => (40, 80), // Reduced from 60 to 40 for debugging
            TerminalSize::ExtraLarge => (80, 80),
        }
    }

    /// Get the number of rows for this terminal size
    pub fn rows(self) -> u16 {
        self.dimensions().0
    }

    /// Get the number of columns for this terminal size
    pub fn cols(self) -> u16 {
        self.dimensions().1
    }
}

/// TerminalCapture maintains a vt100 Parser to apply incremental updates from
/// a pty session and expose the current rendered screen as a String. This
/// centralizes consumption of the session output so callers can repeatedly
/// query the current screen without re-consuming or splitting the underlying
/// pty stream elsewhere.
pub struct TerminalCapture {
    parser: Parser,
}

/// Extension trait for expectrl sessions that support non-blocking reads.
pub trait ExpectSession: Expect {
    /// Attempt to read bytes from the underlying PTY without blocking.
    fn try_read_nonblocking(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

impl<P, S> ExpectSession for expectrl::session::Session<P, S>
where
    S: io::Read + io::Write + NonBlocking,
{
    fn try_read_nonblocking(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        expectrl::session::Session::try_read(self, buf)
    }
}

impl TerminalCapture {
    /// Create a new TerminalCapture with standard terminal size
    pub fn with_size(size: TerminalSize) -> Self {
        let (rows, cols) = size.dimensions();
        Self {
            parser: Parser::new(rows, cols, 0),
        }
    }

    /// Create a new TerminalCapture with given rows/cols (legacy method)
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
        }
    }

    /// Read available bytes from the expectrl session, feed them to the
    /// internal vt100 parser (so cursor moves / clears are applied), log a
    /// snapshot, and return the current rendered screen contents.
    ///
    /// If `log_content` is false, only logs the capture point but not the screen content.
    /// This reduces log verbosity during successful test runs.
    pub async fn capture(
        &mut self,
        session: &mut impl ExpectSession,
        step_description: &str,
    ) -> Result<String> {
        self.capture_with_logging(session, step_description, true)
            .await
    }

    /// Capture screen content with optional logging of the content itself.
    /// Set `log_content` to false to reduce log verbosity during successful operations.
    pub async fn capture_with_logging(
        &mut self,
        session: &mut impl ExpectSession,
        step_description: &str,
        log_content: bool,
    ) -> Result<String> {
        if log_content {
            log::info!("📺 Screen capture point: {step_description}");
        } else {
            log::debug!("📺 Screen capture point: {step_description}");
        }

        const MAX_ATTEMPTS: usize = 3;
        let mut out = String::new();
        let mut last_bytes = 0usize;

        for attempt in 0..MAX_ATTEMPTS {
            let bytes_read = self.drain_session(session)?;
            last_bytes = bytes_read;

            if bytes_read > 0 {
                log::debug!(
                    "🔍 Drained {bytes_read} bytes from session on attempt {}",
                    attempt + 1
                );
            }

            out = self.parser.screen().contents();

            if !out.trim().is_empty() {
                if bytes_read == 0 {
                    log::debug!(
                        "ℹ️ Screen already populated before attempt {}, no new bytes drained",
                        attempt + 1
                    );
                } else {
                    log::info!(
                        "✅ Screen content captured on attempt {} ({} bytes)",
                        attempt + 1,
                        bytes_read
                    );
                }
                break;
            }

            if attempt + 1 < MAX_ATTEMPTS {
                sleep_1s().await;
            }
        }

        if out.trim().is_empty() {
            log::warn!(
                "⚠️ Screen still empty after {MAX_ATTEMPTS} attempts (last drain {} bytes)",
                last_bytes
            );
        }

        update_last_snapshot(step_description, &out);

        // Log as a single multi-line string to preserve CI log formatting (only if requested)
        if log_content {
            log::info!("\n{out}\n");
        }

        // Add a small delay after capture to let the terminal stabilize
        sleep_1s().await;

        Ok(out)
    }

    /// Drain any bytes currently available from the session and feed them into the vt100 parser.
    /// Returns the number of bytes that were processed.
    fn drain_session(&mut self, session: &mut impl ExpectSession) -> Result<usize> {
        use io::ErrorKind;

        let mut total = 0usize;
        let mut buf = [0u8; 4096];

        loop {
            match session.try_read_nonblocking(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                    self.parser.process(&buf[..n]);
                    total += n;
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err.into()),
            }
        }

        Ok(total)
    }

    /// Return the last-rendered screen contents without consuming session
    /// output (useful when you already called `capture` and just want the
    /// latest string again).
    pub fn last(&self) -> String {
        self.parser.screen().contents()
    }
}
