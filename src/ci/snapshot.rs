use anyhow::Result;

use expectrl::{Expect, Regex as ExpectRegex};
use vt100::Parser;

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
    pub fn capture(&mut self, session: &mut impl Expect, step_description: &str) -> Result<String> {
        log::info!("📺 Screen capture point: {step_description}");

        // Pull any available bytes from the session. Using a permissive
        // regex ensures we take whatever is available and let the vt100
        // parser apply cursor moves and SGR sequences correctly.
        let matched = session.expect(ExpectRegex(".*"))?;
        let bytes = matched.as_bytes();

        // Feed bytes into the parser which updates its internal Screen
        self.parser.process(bytes);

        // Render the current screen contents (includes SGR sequences so
        // colors/attributes are preserved in the textual snapshot)
        let out = self.parser.screen().contents();

        log::info!(
            "--- Screen Capture Start ({step_description}) ---\n{out}\n--- Screen Capture End ---"
        );

        Ok(out)
    }

    /// Return the last-rendered screen contents without consuming session
    /// output (useful when you already called `capture` and just want the
    /// latest string again).
    pub fn last(&self) -> String {
        self.parser.screen().contents()
    }
}
