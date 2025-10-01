use anyhow::Result;
use expectrl::Expect;
use regex::Regex;

use crate::ci::{ArrowKey, ExpectKeyExt, TerminalCapture};

/// Action instruction for automated cursor navigation
#[derive(Debug, Clone)]
pub enum CursorAction {
    /// Wait for a pattern to appear on screen (with optional timeout)
    WaitForPattern {
        pattern: Regex,
        description: String,
        timeout_ms: Option<u64>,
    },
    /// Press an arrow key N times
    PressArrow { direction: ArrowKey, count: usize },
    /// Press Enter key
    PressEnter,
    /// Press Escape key
    PressEscape,
    /// Press Tab key
    PressTab,
    /// Type a character
    TypeChar(char),
    /// Type a string
    TypeString(String),
    /// Wait for a fixed duration
    Sleep { ms: u64 },
    /// Capture screen for debugging
    CaptureScreen { description: String },
}

/// Execute a sequence of cursor actions on an expect session
pub async fn execute_cursor_actions<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    session_name: &str,
) -> Result<()> {
    log::info!(
        "🤖 Executing {} cursor actions for {}",
        actions.len(),
        session_name
    );

    for (idx, action) in actions.iter().enumerate() {
        log::debug!("Action {} / {}: {:?}", idx + 1, actions.len(), action);

        match action {
            CursorAction::WaitForPattern {
                pattern,
                description,
                timeout_ms,
            } => {
                let timeout = timeout_ms.unwrap_or(5000);
                log::info!("⏳ Waiting for pattern '{description}' ({timeout} ms)");

                // Try to find the pattern on screen
                let start = std::time::Instant::now();
                loop {
                    let screen =
                        cap.capture(session, &format!("{session_name} - wait for {description}"))?;
                    if pattern.is_match(&screen) {
                        log::info!("✓ Pattern '{description}' found");
                        break;
                    }

                    if start.elapsed().as_millis() > timeout as u128 {
                        return Err(anyhow::anyhow!(
                            "Timeout waiting for pattern '{}' in {}",
                            description,
                            session_name
                        ));
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
            CursorAction::PressArrow { direction, count } => {
                log::info!("⬆️⬇️ Pressing {direction:?} {count} times");
                for _ in 0..*count {
                    session.send_arrow(*direction)?;
                }
            }
            CursorAction::PressEnter => {
                log::info!("↩️ Pressing Enter");
                session.send_enter()?;
            }
            CursorAction::PressEscape => {
                log::info!("⎋ Pressing Escape");
                session.send_escape()?;
            }
            CursorAction::PressTab => {
                log::info!("⇥ Pressing Tab");
                session.send_tab()?;
            }
            CursorAction::TypeChar(ch) => {
                log::info!("⌨️ Typing character '{ch}'");
                session.send_char(*ch)?;
            }
            CursorAction::TypeString(s) => {
                log::info!("⌨️ Typing string '{s}'");
                for ch in s.chars() {
                    session.send_char(ch)?;
                }
            }
            CursorAction::Sleep { ms } => {
                log::info!("💤 Sleeping for {ms} ms");
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            }
            CursorAction::CaptureScreen { description } => {
                log::info!("📸 Capturing screen: {description}");
                let screen = cap.capture(session, &format!("{session_name} - {description}"))?;
                log::info!("Screen content:\n{screen}");
            }
        }
    }

    log::info!("✓ All cursor actions executed successfully for {session_name}");
    Ok(())
}
