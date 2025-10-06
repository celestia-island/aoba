use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;

use crate::ci::{sleep_a_while, ArrowKey, ExpectKeyExt, TerminalCapture};

/// Action instruction for automated cursor navigation
#[derive(Debug, Clone)]
pub enum CursorAction {
    /// Press an arrow key N times
    PressArrow { direction: ArrowKey, count: usize },
    /// Press Enter key
    PressEnter,
    /// Press Escape key
    PressEscape,
    /// Press Tab key
    PressTab,
    /// Press Ctrl+C to exit program quickly
    CtrlC,
    /// Type a character
    TypeChar(char),
    /// Type a string
    TypeString(String),
    /// Wait for a fixed duration
    Sleep { ms: u64 },
    /// Match a pattern within specified line and column range
    /// If match fails, dumps screen and returns error immediately
    MatchPattern {
        pattern: Regex,
        description: String,
        line_range: Option<(usize, usize)>, // (start_line, end_line) inclusive, 0-indexed
        col_range: Option<(usize, usize)>,  // (start_col, end_col) inclusive, 0-indexed
    },
}

/// Execute a sequence of cursor actions on an expect session
/// All actions execute in order. If MatchPattern fails, the function
/// dumps the current screen and returns an error immediately.
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
            CursorAction::MatchPattern {
                pattern,
                description,
                line_range,
                col_range,
            } => {
                log::info!("🔍 Matching pattern '{description}' with retry logic");

                const MAX_RETRIES: usize = 10;
                const RETRY_INTERVAL_MS: u64 = 1000;
                
                let mut matched = false;
                let mut last_screen = String::new();
                
                for attempt in 1..=MAX_RETRIES {
                    // Capture current screen
                    let screen =
                        cap.capture(session, &format!("{session_name} - match {description} (attempt {attempt})"))?;
                    last_screen = screen.clone();

                    // Extract region to search based on line and column ranges
                    let lines: Vec<&str> = screen.lines().collect();
                    let total_lines = lines.len();

                    let (start_line, end_line) =
                        line_range.unwrap_or((0, total_lines.saturating_sub(1)));
                    let start_line = start_line.min(total_lines.saturating_sub(1));
                    let end_line = end_line.min(total_lines.saturating_sub(1));

                    let mut search_text = String::new();
                    for line_idx in start_line..=end_line {
                        if line_idx >= lines.len() {
                            break;
                        }
                        let line = lines[line_idx];
                        let line_text = if let Some((start_col, end_col)) = col_range {
                            let chars: Vec<char> = line.chars().collect();
                            let char_count = chars.len();
                            if char_count == 0 {
                                String::new()
                            } else {
                                let sc = (*start_col).min(char_count.saturating_sub(1));
                                let ec = (*end_col).min(char_count.saturating_sub(1));
                                chars[sc..=ec].iter().collect()
                            }
                        } else {
                            line.to_string()
                        };
                        search_text.push_str(&line_text);
                        search_text.push('\n');
                    }

                    // Try to match pattern
                    if pattern.is_match(&search_text) {
                        log::info!("✓ Pattern '{description}' matched successfully on attempt {attempt}");
                        matched = true;
                        break;
                    } else {
                        if attempt < MAX_RETRIES {
                            log::debug!("Pattern '{description}' not matched on attempt {attempt}, retrying in {RETRY_INTERVAL_MS}ms...");
                            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS)).await;
                        }
                    }
                }
                
                if !matched {
                    // All retries failed - dump screen and return error
                    log::error!("❌ Pattern '{description}' NOT FOUND after {MAX_RETRIES} attempts");
                    log::error!("Expected pattern: {:?}", pattern.as_str());
                    
                    let lines: Vec<&str> = last_screen.lines().collect();
                    let total_lines = lines.len();
                    let (start_line, end_line) =
                        line_range.unwrap_or((0, total_lines.saturating_sub(1)));
                    
                    log::error!(
                        "Search range: lines {start_line}..={end_line}, cols {col_range:?}"
                    );
                    log::error!("Last screen content for {session_name}:");
                    log::error!("\n{last_screen}\n");

                    return Err(anyhow!(
                        "Pattern '{description}' not found in {session_name} after {MAX_RETRIES} attempts (lines {start_line}..={end_line}, cols {col_range:?})",
                    ));
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
            CursorAction::CtrlC => {
                log::info!("🛑 Pressing Ctrl+C to exit");
                session.send_ctrl_c()?;
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
        }

        sleep_a_while().await;
    }

    log::info!("✓ All cursor actions executed successfully for {session_name}");
    Ok(())
}
