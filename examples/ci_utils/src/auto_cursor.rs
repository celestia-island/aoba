use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;

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
    /// Press Ctrl+A to select all text
    PressCtrlA,
    /// Press Backspace to delete
    PressBackspace,
    /// Press PageUp key
    PressPageUp,
    /// Press PageDown key
    PressPageDown,
    /// Press Ctrl+PageUp key
    PressCtrlPageUp,
    /// Press Ctrl+PageDown key
    PressCtrlPageDown,
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
    /// Check status from TUI/CLI status dump files
    /// Verifies that a JSON path in the status equals the expected value
    /// Uses status monitoring to read current state and compare
    CheckStatus {
        /// Description of what is being checked
        description: String,
        /// JSON path to check (e.g., "page", "ports[0].enabled", "ports[0].modbus_masters[0].station_id")
        path: String,
        /// Expected value as serde_json::Value (use json! macro to construct)
        expected: Value,
        /// Timeout in seconds (default: 10)
        timeout_secs: Option<u64>,
        /// Retry interval in milliseconds (default: 500)
        retry_interval_ms: Option<u64>,
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
    for (idx, action) in actions.iter().enumerate() {
        match action {
            CursorAction::MatchPattern {
                pattern,
                description,
                line_range,
                col_range,
                retry_action,
            } => {
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

                        // Capture current screen (without logging content to reduce verbosity)
                        let screen = cap
                            .capture_with_logging(
                                session,
                                &format!("{session_name} - match {description} (outer {outer_attempt}/{OUTER_RETRIES}, inner {inner_attempt}/{INNER_RETRIES})"),
                                false, // Don't log content on every attempt
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
                            matched = true;
                            break;
                        } else {
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
                            // Recursively execute retry_action using Box::pin for async recursion
                            Box::pin(execute_cursor_actions(
                                session,
                                cap,
                                retry_actions,
                                &format!("{session_name}_retry_{outer_attempt}"),
                            ))
                            .await?;

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
                    // All retries failed - dump screen and return error with step position
                    log::error!(
                        "❌ Action Step {} FAILED: Pattern '{description}' NOT FOUND after {total_attempts} total attempts ({OUTER_RETRIES} outer × {INNER_RETRIES} inner)",
                        idx + 1
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
                        "Action Step {}: Pattern '{description}' not found in {session_name} after {total_attempts} attempts (lines {start_line}..={end_line}, cols {col_range:?})",
                        idx + 1
                    ));
                }
            }
            CursorAction::PressArrow { direction, count } => {
                for _ in 0..*count {
                    session.send_arrow(*direction)?;
                }
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressEnter => {
                session.send_enter()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressEscape => {
                session.send_escape()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressTab => {
                session.send_tab()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::CtrlC => {
                session.send_ctrl_c()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressCtrlS => {
                session.send_ctrl_s()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressCtrlA => {
                session.send_ctrl_a()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressBackspace => {
                session.send_backspace()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressPageUp => {
                session.send_page_up()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressPageDown => {
                session.send_page_down()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressCtrlPageUp => {
                session.send_ctrl_page_up()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::PressCtrlPageDown => {
                session.send_ctrl_page_down()?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::TypeChar(ch) => {
                session.send_char(*ch)?;
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::TypeString(s) => {
                for ch in s.chars() {
                    session.send_char(ch)?;
                }
                // Auto sleep after keypress
                sleep_a_while().await;
            }
            CursorAction::Sleep { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            }
            CursorAction::DebugBreakpoint { description } => {
                // Check if debug mode is enabled (set by main program based on --debug flag)
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
            CursorAction::CheckStatus {
                description,
                path,
                expected,
                timeout_secs,
                retry_interval_ms,
            } => {
                let timeout = timeout_secs.unwrap_or(10);
                let interval = retry_interval_ms.unwrap_or(500);

                if let Err(e) = check_status_path(path, expected, timeout, interval).await {
                    log::error!(
                        "❌ Action Step {} FAILED: Status check FAILED for '{description}': {e}",
                        idx + 1
                    );

                    // Capture terminal screen for debugging
                    log::error!("📺 Capturing terminal screen for debugging...");
                    match cap
                        .capture(
                            session,
                            &format!("status_check_failed_{}", description.replace(' ', "_")),
                        )
                        .await
                    {
                        Ok(screen) => {
                            log::error!("Current terminal content:");
                            log::error!("\n{}\n", screen);
                        }
                        Err(cap_err) => {
                            log::error!("Failed to capture terminal: {}", cap_err);
                        }
                    }

                    // Dump all available status files
                    log::error!("📋 Dumping all available status files:");
                    dump_all_status_files();

                    return Err(anyhow!(
                        "Action Step {}: Status check failed for '{description}': {e}",
                        idx + 1
                    ));
                }
            }
        }

        sleep_a_while().await;
    }

    Ok(())
}

/// Dump all available status files for debugging
fn dump_all_status_files() {
    // TUI status
    log::error!("📄 /tmp/ci_tui_status.json:");
    match std::fs::read_to_string("/tmp/ci_tui_status.json") {
        Ok(content) => {
            log::error!("{}", content);
        }
        Err(e) => {
            log::error!("  (not available: {})", e);
        }
    }

    // CLI status files - check for common port names (only vcom1/vcom2 in CI)
    let common_ports = vec!["vcom1", "vcom2"];
    for port in common_ports {
        let cli_path = format!("/tmp/ci_cli_{}_status.json", port);
        log::error!("📄 {}:", cli_path);
        match std::fs::read_to_string(&cli_path) {
            Ok(content) => {
                log::error!("{}", content);
            }
            Err(_) => {
                // Silently skip if file doesn't exist (expected for unused ports)
            }
        }
    }

    // Also try to list all ci_cli_*_status.json files in /tmp
    match std::fs::read_dir("/tmp") {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("ci_cli_") && name.ends_with("_status.json") {
                        let path = entry.path();
                        log::error!("📄 {}:", path.display());
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            log::error!("{}", content);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read /tmp directory: {}", e);
        }
    }
}

/// Check a JSON path in the TUI status and verify it matches the expected value
/// Retries with timeout and interval until the path matches or timeout is reached
async fn check_status_path(
    path: &str,
    expected: &Value,
    timeout_secs: u64,
    retry_interval_ms: u64,
) -> Result<()> {
    use serde_json_path::JsonPath;
    use tokio::time::{sleep, Duration};

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(retry_interval_ms);

    // Compile the JSONPath once outside the loop
    let json_path_str = if path.starts_with('$') {
        path.to_string()
    } else {
        format!("$.{}", path)
    };

    let json_path = JsonPath::parse(&json_path_str)
        .map_err(|e| anyhow!("Invalid JSONPath '{}': {}", json_path_str, e))?;

    loop {
        if start.elapsed() > timeout.into() {
            return Err(anyhow!(
                "Timeout waiting for status path '{}' to equal {:?} (waited {}s)",
                path,
                expected,
                timeout_secs
            ));
        }

        // Read current TUI status
        match crate::read_tui_status() {
            Ok(status) => {
                // Serialize status to JSON for path lookup
                let status_json = serde_json::to_value(&status)
                    .map_err(|e| anyhow!("Failed to serialize status: {}", e))?;

                // Query the JSON path using the library
                let nodes = json_path.query(&status_json);

                // Check if we got exactly one result
                match nodes.exactly_one() {
                    Ok(actual) => {
                        if actual == expected {
                            log::debug!(
                                "✓ Status path '{}' matches expected value: {:?}",
                                path,
                                expected
                            );
                            return Ok(());
                        } else {
                            log::debug!(
                                "Status path '{}' is {:?}, waiting for {:?}",
                                path,
                                actual,
                                expected
                            );
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to find unique value at path '{}': {}", path, e);
                    }
                }
            }
            Err(e) => {
                log::debug!("Failed to read TUI status: {}", e);
            }
        }

        sleep(interval).await;
    }
}
