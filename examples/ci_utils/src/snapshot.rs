use anyhow::Result;

use expectrl::{Expect, Regex as ExpectRegex};
use vt100::Parser;

use crate::helpers::sleep_1s;

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
        session: &mut impl Expect,
        step_description: &str,
    ) -> Result<String> {
        self.capture_with_logging(session, step_description, true)
            .await
    }

    /// Capture screen content with optional logging of the content itself.
    /// Set `log_content` to false to reduce log verbosity during successful operations.
    pub async fn capture_with_logging(
        &mut self,
        session: &mut impl Expect,
        step_description: &str,
        log_content: bool,
    ) -> Result<String> {
        if log_content {
            log::info!("📺 Screen capture point: {step_description}");
        } else {
            log::debug!("📺 Screen capture point: {step_description}");
        }

        // Capture the most recent frame we have; this may be empty on the first call.
        let mut out = self.parser.screen().contents();

        if out.trim().is_empty() {
            // First-time capture: poll in a non-blocking loop until the TUI paints something
            // or we give up after a handful of retries. This avoids the long expect timeout.
            const MAX_ATTEMPTS: usize = 3;
            for attempt in 0..MAX_ATTEMPTS {
                match session.check(ExpectRegex("(?s).+")) {
                    Ok(captures) => {
                        let bytes = captures.as_bytes();
                        log::debug!(
                            "🔍 Captured {} bytes on attempt {}",
                            bytes.len(),
                            attempt + 1
                        );
                        if !bytes.is_empty() {
                            self.parser.process(bytes);
                            out = self.parser.screen().contents();
                            if !out.trim().is_empty() {
                                log::info!("✅ Screen content captured on attempt {}", attempt + 1);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("⚠️ Check failed on attempt {}: {}", attempt + 1, e);
                    }
                }

                // Give the TUI a moment to draw before polling again.
                if attempt + 1 < MAX_ATTEMPTS {
                    sleep_1s().await;
                }
            }

            if out.trim().is_empty() {
                log::warn!("⚠️ Screen still empty after {MAX_ATTEMPTS} attempts");
            }
        } else if let Ok(captures) = session.check(ExpectRegex("(?s).+")) {
            let bytes = captures.as_bytes();
            if !bytes.is_empty() {
                self.parser.process(bytes);
                out = self.parser.screen().contents();
            }
        }

        // Log as a single multi-line string to preserve CI log formatting (only if requested)
        if log_content {
            log::info!("\n{out}\n");
        }

        // Add a small delay after capture to let the terminal stabilize
        sleep_1s().await;

        Ok(out)
    }

    /// Return the last-rendered screen contents without consuming session
    /// output (useful when you already called `capture` and just want the
    /// latest string again).
    pub fn last(&self) -> String {
        self.parser.screen().contents()
    }
}
