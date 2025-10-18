use anyhow::{anyhow, Result};
use regex::Regex;

use expectrl::Expect;

use crate::{sleep_a_while, ArrowKey, ExpectKeyExt, TerminalCapture};

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
    /// Press Ctrl+S to save configuration
    PressCtrlS,
    /// Type a character
    TypeChar(char),
    /// Type a string
    TypeString(String),
    /// Wait for a fixed duration
    Sleep { ms: u64 },
    /// Match a pattern within specified line and column range
    /// If match fails after retries, optionally execute retry_action and retry again
    /// Implements nested retry: 3 attempts -> execute retry_action -> repeat 3 times (total 9 attempts)
    MatchPattern {
        pattern: Regex,
        description: String,
        line_range: Option<(usize, usize)>, // (start_line, end_line) inclusive, 0-indexed
        col_range: Option<(usize, usize)>,  // (start_col, end_col) inclusive, 0-indexed
        retry_action: Option<Vec<CursorAction>>, // Actions to execute before retrying if match fails
    },
    /// Debug breakpoint: capture screen, print it, reset ports, and exit
    /// Only active when debug mode is enabled
    DebugBreakpoint { description: String },
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
        log::info!("📍 Action {} of {}: starting", idx + 1, actions.len());
        log::debug!("Action {} / {}: {:?}", idx + 1, actions.len(), action);

        match action {
            CursorAction::MatchPattern {
                pattern,
                description,
                line_range,
                col_range,
                retry_action,
            } => {
                log::info!("🔍 Matching pattern '{description}' with nested retry logic");

                const INNER_RETRIES: usize = 3; // Number of screen captures before executing retry_action
                const OUTER_RETRIES: usize = 3; // Number of times to execute retry_action
                const RETRY_INTERVAL_MS: u64 = 1000;

                let mut matched = false;
                let mut last_screen = String::new();
                let mut total_attempts = 0;

                // Outer loop: execute retry_action up to OUTER_RETRIES times
                for outer_attempt in 1..=OUTER_RETRIES {
                    // Inner loop: try to match pattern INNER_RETRIES times
                    for inner_attempt in 1..=INNER_RETRIES {
                        total_attempts += 1;

                        // Capture current screen
                        let screen = cap
                            .capture(
                                session,
                                &format!("{session_name} - match {description} (outer {outer_attempt}/{OUTER_RETRIES}, inner {inner_attempt}/{INNER_RETRIES})"),
                            )
                            .await?;
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
                            log::info!(
                                "✓ Pattern '{description}' matched successfully on attempt {total_attempts} (outer {outer_attempt}, inner {inner_attempt})"
                            );
                            matched = true;
                            break;
                        } else {
                            log::debug!("Pattern '{description}' not matched on attempt {total_attempts}, retrying in {RETRY_INTERVAL_MS}ms...");
                            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS))
                                .await;
                        }
                    }

                    // If matched in inner loop, break outer loop
                    if matched {
                        break;
                    }

                    // If we have retry_action and haven't exhausted outer retries, execute it
                    if let Some(ref retry_actions) = retry_action {
                        if outer_attempt < OUTER_RETRIES {
                            log::info!(
                                "🔄 Pattern '{description}' not matched after {INNER_RETRIES} attempts, executing retry_action (outer attempt {outer_attempt}/{OUTER_RETRIES})..."
                            );

                            // Recursively execute retry_action using Box::pin for async recursion
                            Box::pin(execute_cursor_actions(
                                session,
                                cap,
                                retry_actions,
                                &format!("{session_name}_retry_{outer_attempt}"),
                            ))
                            .await?;

                            log::info!("✓ Retry action completed, resuming pattern matching...");

                            // Add a small delay after retry_action before next attempt
                            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS))
                                .await;
                        }
                    } else {
                        // No retry_action, so we're done if not matched
                        break;
                    }
                }

                if !matched {
                    // All retries failed - dump screen and return error
                    log::error!(
                        "❌ Pattern '{description}' NOT FOUND after {total_attempts} total attempts ({OUTER_RETRIES} outer × {INNER_RETRIES} inner)"
                    );
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
                        "Pattern '{description}' not found in {session_name} after {total_attempts} attempts (lines {start_line}..={end_line}, cols {col_range:?})",
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
            CursorAction::PressCtrlS => {
                log::info!("💾 Pressing Ctrl+S to save");
                session.send_ctrl_s()?;
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
            CursorAction::DebugBreakpoint { description } => {
                // Only active in debug mode
                let debug_mode = std::env::var("DEBUG_MODE").is_ok();
                if debug_mode {
                    log::info!("🔴 DEBUG BREAKPOINT: {description}");

                    // Capture and print current screen
                    let screen = cap
                        .capture(session, &format!("debug_breakpoint_{description}"))
                        .await?;
                    log::info!("📺 Current screen state:\n{screen}\n");
                    log::info!("⏸️ Debug breakpoint reached (execution continues)");
                } else {
                    log::debug!("Debug breakpoint '{description}' skipped (DEBUG_MODE not set)");
                }
            }
        }

        log::info!(
            "📍 Action {} of {}: completed, sleeping",
            idx + 1,
            actions.len()
        );
        sleep_a_while().await;
        log::info!("📍 Action {} of {}: sleep done", idx + 1, actions.len());
    }

    log::info!("✓ All cursor actions executed successfully for {session_name}");
    Ok(())
}
