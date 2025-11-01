use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{sleep_1s, sleep_3s, ArrowKey, ExpectKeyExt, ExpectSession, TerminalCapture, TuiStatus};

/// Read a screen capture from file
fn read_screen_capture(test_name: &str, step_name: &str) -> Result<String> {
    // Support hierarchical test names (e.g., "single_station/master_modes")
    let test_path = PathBuf::from(test_name);
    let filepath = Path::new("examples/tui_e2e/screenshots")
        .join(test_path)
        .join(format!("{}.txt", step_name));

    let content = fs::read_to_string(&filepath)?;
    Ok(content)
}

/// Write a screen capture to file
fn write_screen_capture(test_name: &str, step_name: &str, content: &str) -> Result<()> {
    // Support hierarchical test names (e.g., "single_station/master_modes")
    let test_path = PathBuf::from(test_name);
    let dir_path = Path::new("examples/tui_e2e/screenshots").join(test_path);
    
    // Create directory if it doesn't exist
    fs::create_dir_all(&dir_path)?;
    
    let filepath = dir_path.join(format!("{}.txt", step_name));
    fs::write(&filepath, content)?;
    
    log::info!("💾 Wrote screenshot: {}", filepath.display());
    Ok(())
}

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
    /// Wait for 1 second (1000ms)
    Sleep1s,
    /// Wait for 3 seconds (3000ms)
    Sleep3s,
    /// Match a regex pattern against terminal output with retry logic
    MatchPattern {
        /// Regex pattern to match
        pattern: regex::Regex,
        /// Description for logging
        description: String,
        /// Optional line range to search (start, end) inclusive, default is entire screen
        line_range: Option<(usize, usize)>,
        /// Optional column range to search (start, end) inclusive, default is entire line
        col_range: Option<(usize, usize)>,
        /// Optional retry action to execute before retrying pattern match
        retry_action: Option<Vec<CursorAction>>,
    },
    /// Match screen capture against saved screenshot (non-screenshot mode) or write screenshot (screenshot mode)
    /// In non-screenshot mode: reads reference file and compares
    /// In screenshot mode: writes current terminal output to file
    MatchScreenCapture {
        /// Test name (can be hierarchical path like "single_station/master_modes")
        test_name: String,
        /// Step name (e.g., "001_initial_screen", "002_after_navigation")
        step_name: String,
        /// Description for logging
        description: String,
        /// Optional line range to compare (start, end) inclusive
        line_range: Option<(usize, usize)>,
        /// Optional column range to compare (start, end) inclusive
        col_range: Option<(usize, usize)>,
        /// Placeholder values for random data (e.g., register values)
        /// During generation: creates placeholders like {{#x}}, {{0x#x}}, {{0b#x}}
        /// During verification: replaces placeholders with actual values before comparison
        placeholders: Vec<crate::placeholder::PlaceholderValue>,
    },
    /// Check status tree path matches expected value
    CheckStatus {
        /// Description for logging
        description: String,
        /// JSONPath to check in status tree (e.g., "page" or "ports[0].enabled")
        path: String,
        /// Expected value at that path
        expected: serde_json::Value,
        /// Timeout in seconds (default: 10)
        timeout_secs: Option<u64>,
        /// Retry interval in milliseconds (default: 500)
        retry_interval_ms: Option<u64>,
    },
    /// Update mock global status (screenshot mode only)
    /// In screenshot mode: updates the mock TuiStatus using the provided closure
    /// In non-screenshot mode: ignored
    /// The closure receives a mutable reference to TuiStatus and can modify it
    AssertUpdateStatus {
        /// Description for logging
        description: String,
        /// Closure to update the status (similar to write_status pattern)
        updater: fn(&mut TuiStatus),
    },
    /// Debug breakpoint: capture screen, print it, reset ports, and exit
    /// Only active when debug mode is enabled
    DebugBreakpoint { description: String },
}

/// Execution mode for cursor actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionExecutionMode {
    /// Normal mode: execute all actions including keyboard input
    Normal,
    /// Screenshot generation mode: skip keyboard actions, only process screenshots and status updates
    GenerateScreenshots,
}

/// Execute a sequence of cursor actions on an expect session
/// In Normal mode: executes all actions including keyboard input
/// In GenerateScreenshots mode: skips keyboard actions, only processes MatchScreenCapture and AssertUpdateStatus
pub async fn execute_cursor_actions<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    session_name: &str,
) -> Result<()> {
    execute_cursor_actions_with_mode(session, cap, actions, session_name, ActionExecutionMode::Normal, None).await
}

/// Execute cursor actions with specified execution mode and optional mock status
/// This is the internal implementation that supports both normal and screenshot generation modes
pub async fn execute_cursor_actions_with_mode<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    session_name: &str,
    mode: ActionExecutionMode,
    mut mock_status: Option<&mut TuiStatus>,
) -> Result<()> {
    for (idx, action) in actions.iter().enumerate() {
        match action {
            // In screenshot mode, keyboard actions are skipped
            CursorAction::PressArrow { direction, count } if mode == ActionExecutionMode::Normal => {
                for _ in 0..*count {
                    session.send_arrow(*direction)?;
                }
                sleep_1s().await;
            }
            CursorAction::PressEnter if mode == ActionExecutionMode::Normal => {
                session.send_enter()?;
                sleep_1s().await;
            }
            CursorAction::PressEscape if mode == ActionExecutionMode::Normal => {
                session.send_escape()?;
                sleep_1s().await;
            }
            CursorAction::PressTab if mode == ActionExecutionMode::Normal => {
                session.send_tab()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::CtrlC => {
                session.send_ctrl_c()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressCtrlS => {
                session.send_ctrl_s()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressCtrlA => {
                session.send_ctrl_a()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressBackspace => {
                session.send_backspace()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressPageUp => {
                session.send_page_up()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressPageDown => {
                session.send_page_down()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressCtrlPageUp => {
                session.send_ctrl_page_up()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::PressCtrlPageDown => {
                session.send_ctrl_page_down()?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::TypeChar(ch) => {
                session.send_char(*ch)?;
                // Auto sleep after keypress
                sleep_1s().await;
            }
            CursorAction::TypeString(s) => {
                for ch in s.chars() {
                    session.send_char(ch)?;
                    // Sleep after each character to ensure TUI processes input properly
                    sleep_1s().await;
                }
            }
            CursorAction::Sleep1s => {
                sleep_1s().await;
            }
            CursorAction::Sleep3s => {
                sleep_3s().await;
            }
            CursorAction::MatchPattern {
                pattern,
                description,
                line_range,
                col_range,
                retry_action,
            } if mode == ActionExecutionMode::Normal => {
                const INNER_RETRIES: usize = 3;
                const OUTER_RETRIES: usize = 3;
                const RETRY_INTERVAL_MS: u64 = 1000;

                let mut matched = false;
                let mut last_screen = String::new();
                let mut total_attempts = 0;

                for outer_attempt in 1..=OUTER_RETRIES {
                    for inner_attempt in 1..=INNER_RETRIES {
                        total_attempts += 1;

                        let screen = cap
                            .capture_with_logging(
                                session,
                                &format!("{session_name} - match {description} (outer {outer_attempt}/{OUTER_RETRIES}, inner {inner_attempt}/{INNER_RETRIES})"),
                                false,
                            )
                            .await?;
                        last_screen = screen.clone();

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

                        if pattern.is_match(&search_text) {
                            matched = true;
                            break;
                        } else {
                            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS))
                                .await;
                        }
                    }

                    if matched {
                        break;
                    }

                    if let Some(ref retry_actions) = retry_action {
                        if outer_attempt < OUTER_RETRIES {
                            Box::pin(execute_cursor_actions(
                                session,
                                cap,
                                retry_actions,
                                &format!("{session_name}_retry_{outer_attempt}"),
                            ))
                            .await?;

                            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS))
                                .await;
                        }
                    } else {
                        break;
                    }
                }

                if !matched {
                    log::error!(
                        "❌ Action Step {} FAILED: Pattern '{description}' NOT FOUND after {total_attempts} total attempts",
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
                        "Action Step {}: Pattern '{description}' not found in {session_name} after {total_attempts} attempts",
                        idx + 1
                    ));
                }
            }
            CursorAction::MatchScreenCapture {
                test_name,
                step_name,
                description,
                line_range,
                col_range,
                placeholders,
            } => {
                // In screenshot generation mode, register placeholders before capturing
                if mode == ActionExecutionMode::GenerateScreenshots {
                    if !placeholders.is_empty() {
                        crate::placeholder::register_placeholder_values(placeholders);
                    }
                }

                // Read the saved screen capture
                let expected_screen = match read_screen_capture(test_name, step_name) {
                    Ok(content) => content,
                    Err(e) => {
                        log::error!(
                            "❌ Action Step {} FAILED: Failed to read screen capture for test '{}', step '{}': {}",
                            idx + 1,
                            test_name,
                            step_name,
                            e
                        );
                        return Err(anyhow!(
                            "Action Step {}: Failed to read screen capture for test '{}', step '{}': {}",
                            idx + 1,
                            test_name,
                            step_name,
                            e
                        ));
                    }
                };

                // Capture current screen
                let current_screen = cap
                    .capture(
                        session,
                        &format!("match_screen_{}_{}", test_name, step_name),
                    )
                    .await?;

                // Extract regions to compare based on line and column ranges
                let extract_region = |screen: &str| -> String {
                    let lines: Vec<&str> = screen.lines().collect();
                    let total_lines = lines.len();

                    let (start_line, end_line) =
                        line_range.unwrap_or((0, total_lines.saturating_sub(1)));
                    let start_line = start_line.min(total_lines.saturating_sub(1));
                    let end_line = end_line.min(total_lines.saturating_sub(1));

                    let mut region_text = String::new();
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
                        region_text.push_str(&line_text);
                        region_text.push('\n');
                    }
                    region_text
                };

                let expected_region = extract_region(&expected_screen);
                let current_region = extract_region(&current_screen);

                // In verification mode, restore placeholders in expected before comparison
                let expected_region = if mode == ActionExecutionMode::Normal && !placeholders.is_empty() {
                    // Register placeholders with actual values before restoration
                    crate::placeholder::register_placeholder_values(placeholders);
                    crate::placeholder::restore_placeholders_for_verification(&expected_region)
                } else {
                    expected_region
                };

                // Compare the regions
                if expected_region == current_region {
                    log::debug!("✅ Screen capture matched for '{}'", description);
                } else {
                    log::error!(
                        "❌ Action Step {} FAILED: Screen capture mismatch for '{}'",
                        idx + 1,
                        description
                    );
                    log::error!(
                        "Expected region (lines {:?}, cols {:?}):",
                        line_range,
                        col_range
                    );
                    log::error!("\n{}\n", expected_region);
                    log::error!(
                        "Current region (lines {:?}, cols {:?}):",
                        line_range,
                        col_range
                    );
                    log::error!("\n{}\n", current_region);
                    log::error!("Full current screen:");
                    log::error!("\n{}\n", current_screen);

                    return Err(anyhow!(
                        "Action Step {}: Screen capture mismatch for '{}' (test: '{}', step: '{}')",
                        idx + 1,
                        description,
                        test_name,
                        step_name
                    ));
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
                            log::error!("\n{screen}\n");
                        }
                        Err(cap_err) => {
                            log::error!("Failed to capture terminal: {cap_err}");
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
            CursorAction::AssertUpdateStatus { description, updater } => {
                if mode == ActionExecutionMode::GenerateScreenshots {
                    // In screenshot generation mode, update the mock status
                    if let Some(status) = mock_status.as_mut() {
                        log::info!("🔄 Updating mock status: {}", description);
                        updater(status);
                    } else {
                        log::warn!("⚠️  AssertUpdateStatus called without mock_status provided");
                    }
                } else {
                    // In normal mode, this action is a no-op (status is managed by real TUI)
                    log::debug!("Skipping AssertUpdateStatus in normal mode: {}", description);
                }
            }
            CursorAction::DebugBreakpoint { description } => {
                // Check if debug mode is enabled
                let debug_mode = std::env::var("DEBUG_MODE").is_ok();
                if debug_mode && mode == ActionExecutionMode::Normal {
                    log::info!("🔴 DEBUG BREAKPOINT: {description}");
                    let screen = cap
                        .capture(session, &format!("debug_breakpoint_{description}"))
                        .await?;
                    log::info!("📺 Current screen state:\n{screen}\n");
                    log::info!("⏸️ Debug breakpoint reached (execution continues)");
                } else {
                    log::debug!("Debug breakpoint '{description}' skipped");
                }
            }
            // Catch-all for keyboard actions in screenshot mode - skip them
            _ if mode == ActionExecutionMode::GenerateScreenshots => {
                log::debug!("Skipping keyboard action in screenshot generation mode: {:?}", action);
            }
            // Catch-all for any unhandled action patterns
            _ => {
                log::warn!("Unhandled action or action in wrong mode: {:?}", action);
            }
        }

        sleep_1s().await;
    }

    Ok(())
}

/// Dump all available status files for debugging
fn dump_all_status_files() {
    // TUI status
    log::error!("📄 /tmp/ci_tui_status.json:");
    match std::fs::read_to_string("/tmp/ci_tui_status.json") {
        Ok(content) => {
            log::error!("{content}");
        }
        Err(e) => {
            log::error!("  (not available: {e})");
        }
    }

    // CLI status files - check for common port names (only vcom1/vcom2 in CI)
    let common_ports = vec!["vcom1", "vcom2"];
    for port in common_ports {
        let cli_path = format!("/tmp/ci_cli_{port}_status.json");
        log::error!("📄 {cli_path}:");
        match std::fs::read_to_string(&cli_path) {
            Ok(content) => {
                log::error!("{content}");
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
                            log::error!("{content}");
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read /tmp directory: {e}");
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
        format!("$.{path}")
    };

    let json_path = JsonPath::parse(&json_path_str)
        .map_err(|e| anyhow!("Invalid JSONPath '{json_path_str}': {e}"))?;

    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for status path '{path}' to equal {expected:?} (waited {timeout_secs}s)"
            ));
        }

        // Read current TUI status
        match crate::read_tui_status() {
            Ok(status) => {
                // Serialize status to JSON for path lookup
                let status_json = serde_json::to_value(&status)
                    .map_err(|e| anyhow!("Failed to serialize status: {e}"))?;

                // Query the JSON path using the library
                let nodes = json_path.query(&status_json);

                // Check if we got exactly one result
                match nodes.exactly_one() {
                    Ok(actual) => {
                        if actual == expected {
                            log::debug!(
                                "✓ Status path '{path}' matches expected value: {expected:?}"
                            );
                            return Ok(());
                        } else {
                            log::debug!(
                                "Status path '{path}' is {actual:?}, waiting for {expected:?}"
                            );
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to find unique value at path '{path}': {e}");
                    }
                }
            }
            Err(e) => {
                log::debug!("Failed to read TUI status: {e}");
            }
        }

        sleep(interval).await;
    }
}

/// Factory function for executing actions with automatic status validation and retry
///
/// This function combines cursor actions with status checks in a single atomic operation
/// with automatic retry logic. It's designed to implement fine-grained validation at each
/// step of UI interaction.
///
/// # Purpose
///
/// In TUI E2E tests, we need to verify that the UI's internal state matches our expectations
/// after each interaction. This function automates the pattern of:
/// 1. Execute some cursor actions (e.g., press arrow keys, type text)
/// 2. Verify the result via status check (e.g., check field value, edit mode state)
/// 3. Retry if verification fails (e.g., action didn't take effect)
///
/// # Parameters
///
/// - `session`: Active TUI session
/// - `cap`: Terminal capture for debugging
/// - `actions`: Cursor actions to execute (e.g., navigation, typing)
/// - `status_checks`: Status validations to perform after actions
/// - `session_name`: Name for logging/debugging
/// - `max_retries`: Number of retry attempts (default: 3)
///
/// # Returns
///
/// - `Ok(())`: Actions executed and all status checks passed
/// - `Err`: Actions failed, status checks failed, or timeout reached
///
/// # Example 1: Navigate and verify cursor position
///
/// ```rust,no_run
/// # use ci_utils::*;
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use serde_json::json;
///
/// execute_with_status_checks(
///     &mut session,
///     &mut cap,
///     // Actions: Navigate down 2 times to "Station ID" field
///     &[
///         CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 },
///     ],
///     // Status checks: Verify cursor is on station_id field
///     &[
///         CursorAction::CheckStatus {
///             description: "Cursor on Station ID field".to_string(),
///             path: "cursor.field".to_string(),
///             expected: json!("station_id"),
///             timeout_secs: Some(5),
///             retry_interval_ms: Some(300),
///         },
///     ],
///     "navigate_to_station_id",
///     None,
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Enter edit mode and verify
///
/// ```rust,no_run
/// # use ci_utils::*;
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use serde_json::json;
///
/// execute_with_status_checks(
///     &mut session,
///     &mut cap,
///     // Actions: Press Enter to enter edit mode
///     &[CursorAction::PressEnter],
///     // Status checks: Verify in edit mode with empty buffer
///     &[
///         CursorAction::CheckStatus {
///             description: "Entered edit mode".to_string(),
///             path: "cursor.mode".to_string(),
///             expected: json!("Edit"),
///             timeout_secs: Some(3),
///             retry_interval_ms: Some(300),
///         },
///         CursorAction::CheckStatus {
///             description: "Edit buffer is empty".to_string(),
///             path: "cursor.edit_buffer".to_string(),
///             expected: json!(""),
///             timeout_secs: Some(2),
///             retry_interval_ms: Some(200),
///         },
///     ],
///     "enter_edit_mode",
///     None,
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example 3: Type and verify input buffer
///
/// ```rust,no_run
/// # use ci_utils::*;
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use serde_json::json;
///
/// execute_with_status_checks(
///     &mut session,
///     &mut cap,
///     // Actions: Type "123"
///     &[CursorAction::TypeString("123".to_string())],
///     // Status checks: Verify buffer contains "123"
///     &[
///         CursorAction::CheckStatus {
///             description: "Typed '123' into buffer".to_string(),
///             path: "cursor.edit_buffer".to_string(),
///             expected: json!("123"),
///             timeout_secs: Some(3),
///             retry_interval_ms: Some(300),
///         },
///     ],
///     "type_station_id",
///     None,
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example 4: Exit edit and verify value committed
///
/// ```rust,no_run
/// # use ci_utils::*;
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use serde_json::json;
///
/// execute_with_status_checks(
///     &mut session,
///     &mut cap,
///     // Actions: Press Enter to commit
///     &[CursorAction::PressEnter],
///     // Status checks: Verify exited edit mode AND value was written
///     &[
///         CursorAction::CheckStatus {
///             description: "Exited edit mode".to_string(),
///             path: "cursor.mode".to_string(),
///             expected: json!("Normal"),
///             timeout_secs: Some(3),
///             retry_interval_ms: Some(300),
///         },
///         CursorAction::CheckStatus {
///             description: "Station ID updated to 123".to_string(),
///             path: "ports[0].modbus_masters[0].station_id".to_string(),
///             expected: json!(123),
///             timeout_secs: Some(5),
///             retry_interval_ms: Some(500),
///         },
///     ],
///     "commit_station_id",
///     None,
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Retry Logic
///
/// If any status check fails:
/// 1. Log warning with attempt number
/// 2. Wait 1 second
/// 3. Re-execute ALL actions from the beginning
/// 4. Re-check ALL status validations
/// 5. Repeat up to `max_retries` times (default: 3)
///
/// This ensures that transient timing issues (e.g., UI not yet updated) don't cause
/// test failures, while still catching real bugs (actions not working at all).
///
/// # Granularity Best Practices
///
/// For fine-grained validation, break operations into small atomic steps:
///
/// ```rust,no_run
/// # use ci_utils::*;
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use serde_json::json;
///
/// // BAD: One big action without intermediate checks
/// // execute_with_status_checks(
/// //     &mut session, &mut cap,
/// //     &[
/// //         CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 },
/// //         CursorAction::PressEnter,
/// //         CursorAction::TypeString("123".to_string()),
/// //         CursorAction::PressEnter,
/// //     ],
/// //     &[/* only check final result */],
/// //     "big_action", None
/// // ).await?;
///
/// // GOOD: Multiple small actions with checks at each step
/// execute_with_status_checks(
///     &mut session, &mut cap,
///     &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 }],
///     &[CursorAction::CheckStatus {
///         description: "Cursor on target field".to_string(),
///         path: "cursor.field".to_string(),
///         expected: json!("station_id"),
///         timeout_secs: Some(3),
///         retry_interval_ms: Some(300),
///     }],
///     "navigate", None
/// ).await?;
///
/// execute_with_status_checks(
///     &mut session, &mut cap,
///     &[CursorAction::PressEnter],
///     &[CursorAction::CheckStatus {
///         description: "Entered edit mode".to_string(),
///         path: "cursor.mode".to_string(),
///         expected: json!("Edit"),
///         timeout_secs: Some(3),
///         retry_interval_ms: Some(300),
///     }],
///     "enter_edit", None
/// ).await?;
///
/// execute_with_status_checks(
///     &mut session, &mut cap,
///     &[CursorAction::TypeString("123".to_string())],
///     &[CursorAction::CheckStatus {
///         description: "Buffer contains typed value".to_string(),
///         path: "cursor.edit_buffer".to_string(),
///         expected: json!("123"),
///         timeout_secs: Some(3),
///         retry_interval_ms: Some(300),
///     }],
///     "type_value", None
/// ).await?;
///
/// execute_with_status_checks(
///     &mut session, &mut cap,
///     &[CursorAction::PressEnter],
///     &[
///         CursorAction::CheckStatus {
///             description: "Exited edit mode".to_string(),
///             path: "cursor.mode".to_string(),
///             expected: json!("Normal"),
///             timeout_secs: Some(3),
///             retry_interval_ms: Some(300),
///         },
///         CursorAction::CheckStatus {
///             description: "Value committed to config".to_string(),
///             path: "ports[0].modbus_masters[0].station_id".to_string(),
///             expected: json!(123),
///             timeout_secs: Some(5),
///             retry_interval_ms: Some(500),
///         },
///     ],
///     "commit_value", None
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`execute_cursor_actions`]: Lower-level action execution without retry
/// - [`CursorAction::CheckStatus`]: Individual status check action
/// - [`check_status_path`]: Underlying status verification function
pub async fn execute_with_status_checks<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    status_checks: &[CursorAction],
    session_name: &str,
    max_retries: Option<usize>,
) -> Result<()> {
    let max_retries = max_retries.unwrap_or(3);

    for attempt in 1..=max_retries {
        // Execute all actions
        match execute_cursor_actions(
            session,
            cap,
            actions,
            &format!("{}_actions_attempt_{}", session_name, attempt),
        )
        .await
        {
            Ok(()) => {
                // All actions succeeded, now check status
                match execute_cursor_actions(
                    session,
                    cap,
                    status_checks,
                    &format!("{}_checks_attempt_{}", session_name, attempt),
                )
                .await
                {
                    Ok(()) => {
                        // All checks passed
                        if attempt > 1 {
                            log::info!(
                                "✅ {} succeeded on attempt {}/{}",
                                session_name,
                                attempt,
                                max_retries
                            );
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        // Status checks failed
                        if attempt < max_retries {
                            log::warn!(
                                "⚠️  {} status checks failed on attempt {}/{}: {}",
                                session_name,
                                attempt,
                                max_retries,
                                e
                            );
                            log::warn!("   Retrying...");
                            sleep_1s().await;
                            continue;
                        } else {
                            log::error!(
                                "❌ {} status checks failed after {} attempts",
                                session_name,
                                max_retries
                            );
                            return Err(anyhow!(
                                "{} status checks failed after {} attempts: {}",
                                session_name,
                                max_retries,
                                e
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                // Actions failed
                if attempt < max_retries {
                    log::warn!(
                        "⚠️  {} actions failed on attempt {}/{}: {}",
                        session_name,
                        attempt,
                        max_retries,
                        e
                    );
                    log::warn!("   Retrying...");
                    sleep_1s().await;
                    continue;
                } else {
                    log::error!(
                        "❌ {} actions failed after {} attempts",
                        session_name,
                        max_retries
                    );
                    return Err(anyhow!(
                        "{} actions failed after {} attempts: {}",
                        session_name,
                        max_retries,
                        e
                    ));
                }
            }
        }
    }

    Err(anyhow!(
        "{} failed after {} attempts",
        session_name,
        max_retries
    ))
}
