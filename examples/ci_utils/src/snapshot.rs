use anyhow::Result;

use expectrl::{Expect, Regex as ExpectRegex};
use vt100::Parser;

use crate::helpers::sleep_a_while;

/// TerminalCapture maintains a vt100 Parser to apply incremental updates from
/// a pty session and expose the current rendered screen as a String. This
/// centralizes consumption of the session output so callers can repeatedly
/// query the current screen without re-consuming or splitting the underlying
/// pty stream elsewhere.
pub struct TerminalCapture {
    parser: Parser,
}

impl TerminalCapture {
    /// Create a new TerminalCapture with given rows/cols
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
        }
    }

    /// Read available bytes from the expectrl session, feed them to the
    /// internal vt100 parser (so cursor moves / clears are applied), log a
    /// snapshot, and return the current rendered screen contents.
    pub async fn capture(
        &mut self,
        session: &mut impl Expect,
        step_description: &str,
    ) -> Result<String> {
        log::info!("📺 Screen capture point: {step_description}");

        // Capture the most recent frame we have; this may be empty on the first call.
        let mut out = self.parser.screen().contents();

        if out.trim().is_empty() {
            // First-time capture: poll in a non-blocking loop until the TUI paints something
            // or we give up after a handful of retries. This avoids the long expect timeout.
            const MAX_ATTEMPTS: usize = 10;
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
                    sleep_a_while().await;
                }
            }

            if out.trim().is_empty() {
                log::warn!("⚠️ Screen still empty after {} attempts", MAX_ATTEMPTS);
            }
        } else if let Ok(captures) = session.check(ExpectRegex("(?s).+")) {
            let bytes = captures.as_bytes();
            if !bytes.is_empty() {
                self.parser.process(bytes);
                out = self.parser.screen().contents();
            }
        }

        // Log as a single multi-line string to preserve CI log formatting
        log::info!("\n{out}\n");

        // Add a small delay after capture to let the terminal stabilize
        sleep_a_while().await;

        Ok(out)
    }

    /// Return the last-rendered screen contents without consuming session
    /// output (useful when you already called `capture` and just want the
    /// latest string again).
    pub fn last(&self) -> String {
        self.parser.screen().contents()
    }
}
