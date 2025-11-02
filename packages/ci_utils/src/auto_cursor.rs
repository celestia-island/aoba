use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    sleep_1s, sleep_3s, ArrowKey, ExpectKeyExt, ExpectSession, TerminalCapture, TuiStatus,
};

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
    /// Check status tree path matches expected value
    /// Debug breakpoint: capture screen, print it, reset ports, and exit
    /// Only active when debug mode is enabled
    DebugBreakpoint { description: String },
}

/// Extract a sub-region from terminal screen text based on optional line/column ranges
fn extract_screen_region(
    screen: &str,
    line_range: Option<(usize, usize)>,
    col_range: Option<(usize, usize)>,
) -> String {
    let lines: Vec<&str> = screen.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return String::new();
    }

    let (raw_start, raw_end) = line_range.unwrap_or((0, total_lines.saturating_sub(1)));
    let start_line = raw_start.min(total_lines.saturating_sub(1));
    let end_line = raw_end.min(total_lines.saturating_sub(1));

    let mut region = String::new();
    for idx in start_line..=end_line {
        if idx >= lines.len() {
            break;
        }
        let line = lines[idx];
        let segment = if let Some((start_col, end_col)) = col_range {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                String::new()
            } else {
                let sc = start_col.min(chars.len().saturating_sub(1));
                let ec = end_col.min(chars.len().saturating_sub(1));
                chars[sc..=ec].iter().collect()
            }
        } else {
            line.to_string()
        };
        region.push_str(&segment);
        region.push('\n');
    }

    region
}

/// Update function signature for mock status adjustments in screenshot-generation mode
pub type StateUpdater = Box<dyn Fn(&mut TuiStatus) + Send + Sync>;

/// Specification for a regex-based screen pattern assertion
#[derive(Clone)]
pub struct ScreenPatternSpec {
    pattern: Regex,
    description: String,
    line_range: Option<(usize, usize)>,
    col_range: Option<(usize, usize)>,
    retry_action: Option<Vec<CursorAction>>,
    inner_retries: usize,
    outer_retries: usize,
    retry_interval_ms: u64,
}

impl ScreenPatternSpec {
    /// Create a pattern spec with default retry parameters
    pub fn new(pattern: Regex, description: impl Into<String>) -> Self {
        Self {
            pattern,
            description: description.into(),
            line_range: None,
            col_range: None,
            retry_action: None,
            inner_retries: 3,
            outer_retries: 3,
            retry_interval_ms: 1000,
        }
    }

    /// Restrict match to a subset of lines
    pub fn with_line_range(mut self, range: Option<(usize, usize)>) -> Self {
        self.line_range = range;
        self
    }

    /// Restrict match to a subset of columns
    pub fn with_col_range(mut self, range: Option<(usize, usize)>) -> Self {
        self.col_range = range;
        self
    }

    /// Provide additional cursor actions to perform before a retry cycle
    pub fn with_retry_action(mut self, actions: Option<Vec<CursorAction>>) -> Self {
        self.retry_action = actions;
        self
    }

    /// Configure inner retry attempts (captures without replaying actions)
    pub fn with_inner_retries(mut self, retries: usize) -> Self {
        self.inner_retries = retries.max(1);
        self
    }

    /// Configure outer retry attempts (after executing retry actions)
    pub fn with_outer_retries(mut self, retries: usize) -> Self {
        self.outer_retries = retries.max(1);
        self
    }

    /// Configure retry interval in milliseconds between attempts
    pub fn with_retry_interval_ms(mut self, interval_ms: u64) -> Self {
        self.retry_interval_ms = interval_ms.max(1);
        self
    }

    async fn verify<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        session_name: &str,
        mode: ActionExecutionMode,
    ) -> Result<()> {
        if mode != ActionExecutionMode::Normal {
            return Ok(());
        }

        let mut last_screen = String::new();
        let mut total_attempts = 0usize;

        for outer in 1..=self.outer_retries {
            for inner in 1..=self.inner_retries {
                total_attempts += 1;
                let capture_name = format!("{session_name}_pattern_outer{}_inner{}", outer, inner);
                let screen = cap
                    .capture_with_logging(session, &capture_name, false)
                    .await?;
                last_screen = screen.clone();

                let region = extract_screen_region(&screen, self.line_range, self.col_range);
                if self.pattern.is_match(&region) {
                    log::debug!(
                        "✅ Pattern '{}' matched after {} attempt(s)",
                        self.description,
                        total_attempts
                    );
                    return Ok(());
                }

                tokio::time::sleep(std::time::Duration::from_millis(self.retry_interval_ms)).await;
            }

            if let Some(retry_actions) = &self.retry_action {
                if outer < self.outer_retries {
                    log::debug!(
                        "Retrying pattern '{}' with additional actions (outer {}/{})",
                        self.description,
                        outer,
                        self.outer_retries
                    );
                    Box::pin(execute_cursor_actions(
                        session,
                        cap,
                        retry_actions,
                        &format!("{session_name}_pattern_retry_outer{outer}"),
                    ))
                    .await?;
                    tokio::time::sleep(std::time::Duration::from_millis(self.retry_interval_ms))
                        .await;
                }
            }
        }

        log::error!(
            "❌ Pattern '{}' did not match after {} attempts",
            self.description,
            total_attempts
        );
        log::error!("Last captured screen:");
        log::error!("\n{}\n", last_screen);

        Err(anyhow!(
            "Pattern '{}' not found after {} attempts",
            self.description,
            total_attempts
        ))
    }
}

/// Specification for a screenshot comparison assertion
#[derive(Clone)]
pub struct ScreenCaptureSpec {
    test_name: String,
    step_name: String,
    description: String,
    line_range: Option<(usize, usize)>,
    col_range: Option<(usize, usize)>,
    placeholders: Vec<crate::placeholder::PlaceholderValue>,
}

impl ScreenCaptureSpec {
    /// Create a screenshot comparison spec
    pub fn new(
        test_name: impl Into<String>,
        step_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            test_name: test_name.into(),
            step_name: step_name.into(),
            description: description.into(),
            line_range: None,
            col_range: None,
            placeholders: Vec::new(),
        }
    }

    /// Restrict comparison to a line range
    pub fn with_line_range(mut self, range: Option<(usize, usize)>) -> Self {
        self.line_range = range;
        self
    }

    /// Restrict comparison to a column range
    pub fn with_col_range(mut self, range: Option<(usize, usize)>) -> Self {
        self.col_range = range;
        self
    }

    /// Provide placeholder values for dynamic content
    pub fn with_placeholders(
        mut self,
        placeholders: Vec<crate::placeholder::PlaceholderValue>,
    ) -> Self {
        self.placeholders = placeholders;
        self
    }

    async fn verify<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        session_name: &str,
        mode: ActionExecutionMode,
    ) -> Result<()> {
        if mode == ActionExecutionMode::GenerateScreenshots {
            if !self.placeholders.is_empty() {
                crate::placeholder::register_placeholder_values(&self.placeholders);
            }
            return Ok(());
        }

        let expected_screen = read_screen_capture(&self.test_name, &self.step_name)?;
        let capture_step = format!(
            "{}_capture_{}_{}",
            session_name, self.test_name, self.step_name
        );
        let current_screen = cap.capture(session, &capture_step).await?;

        let mut expected_region =
            extract_screen_region(&expected_screen, self.line_range, self.col_range);
        let current_region =
            extract_screen_region(&current_screen, self.line_range, self.col_range);

        if !self.placeholders.is_empty() {
            crate::placeholder::register_placeholder_values(&self.placeholders);
            expected_region =
                crate::placeholder::restore_placeholders_for_verification(&expected_region);
        }

        if expected_region == current_region {
            log::debug!(
                "✅ Screen capture '{}' matched reference {}::{}",
                self.description,
                self.test_name,
                self.step_name
            );
            return Ok(());
        }

        log::error!(
            "❌ Screen capture mismatch for '{}' ({}::{})",
            self.description,
            self.test_name,
            self.step_name
        );
        log::error!(
            "Expected region (lines {:?}, cols {:?}):\n{}",
            self.line_range,
            self.col_range,
            expected_region
        );
        log::error!(
            "Actual region (lines {:?}, cols {:?}):\n{}",
            self.line_range,
            self.col_range,
            current_region
        );
        log::error!("Full actual screen:\n{}", current_screen);

        Err(anyhow!(
            "Screen capture mismatch for '{}' ({}::{})",
            self.description,
            self.test_name,
            self.step_name
        ))
    }
}

/// Screen-level assertion variants supported by [`TuiStep`]
#[derive(Clone)]
pub enum ScreenAssertion {
    Pattern(ScreenPatternSpec),
    Capture(ScreenCaptureSpec),
}

impl ScreenAssertion {
    pub fn pattern(spec: ScreenPatternSpec) -> Self {
        Self::Pattern(spec)
    }

    pub fn capture(spec: ScreenCaptureSpec) -> Self {
        Self::Capture(spec)
    }
}

/// Composite step definition combining cursor actions, state patches, and screen assertions
pub struct TuiStep {
    pub name: String,
    pub actions: Vec<CursorAction>,
    pub status_checks: Vec<CursorAction>,
    pub assertions: Vec<ScreenAssertion>,
    pub state_patch: Option<StateUpdater>,
    pub retry_setup: Option<Vec<CursorAction>>,
    pub max_attempts: usize,
}

impl TuiStep {
    /// Create a new step with sensible defaults
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: Vec::new(),
            status_checks: Vec::new(),
            assertions: Vec::new(),
            state_patch: None,
            retry_setup: None,
            max_attempts: 3,
        }
    }

    /// Override the actions for this step
    pub fn with_actions(mut self, actions: Vec<CursorAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Override status checks for this step
    pub fn with_status_checks(mut self, checks: Vec<CursorAction>) -> Self {
        self.status_checks = checks;
        self
    }

    /// Override assertions for this step
    pub fn with_assertions(mut self, assertions: Vec<ScreenAssertion>) -> Self {
        self.assertions = assertions;
        self
    }

    /// Attach a state patch closure (screenshot mode only)
    pub fn with_state_patch<F>(mut self, patch: F) -> Self
    where
        F: Fn(&mut TuiStatus) + Send + Sync + 'static,
    {
        self.state_patch = Some(Box::new(patch));
        self
    }

    /// Provide retry setup actions executed between attempts when a failure occurs
    pub fn with_retry_setup(mut self, setup: Vec<CursorAction>) -> Self {
        self.retry_setup = Some(setup);
        self
    }

    /// Override maximum retry attempts (default: 3)
    pub fn with_max_attempts(mut self, attempts: usize) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }

    /// Execute the step in normal mode
    pub async fn run<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
    ) -> Result<()> {
        self.run_with_mode(session, cap, ActionExecutionMode::Normal, None)
            .await
    }

    /// Execute the step with explicit execution mode and optional mock status
    pub async fn run_with_mode<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        mode: ActionExecutionMode,
        mut mock_status: Option<&mut TuiStatus>,
    ) -> Result<()> {
        let attempts = self.max_attempts.max(1);

        for attempt in 1..=attempts {
            if attempt == 1 && mode == ActionExecutionMode::GenerateScreenshots {
                if let Some(updater) = &self.state_patch {
                    if let Some(status) = mock_status.as_mut() {
                        log::info!(
                            "🔄 Applying state patch for step '{}' in screenshot mode",
                            self.name
                        );
                        updater(*status);
                    } else {
                        log::warn!(
                            "⚠️ Step '{}' configured a state patch but mock status is unavailable",
                            self.name
                        );
                    }
                }
            }

            let action_result = execute_cursor_actions_with_mode(
                session,
                cap,
                &self.actions,
                &format!("{}_actions_attempt_{}", self.name, attempt),
                mode,
            )
            .await;

            if let Err(err) = action_result {
                if attempt < attempts {
                    log::warn!(
                        "⚠️ Step '{}' actions failed on attempt {}/{}: {}",
                        self.name,
                        attempt,
                        attempts,
                        err
                    );
                    sleep_1s().await;
                    self.run_retry_setup(session, cap, mode, attempt).await?;
                    continue;
                } else {
                    return Err(anyhow!(
                        "Step '{}' actions failed after {} attempts: {}",
                        self.name,
                        attempts,
                        err
                    ));
                }
            }

            let check_result = execute_cursor_actions_with_mode(
                session,
                cap,
                &self.status_checks,
                &format!("{}_checks_attempt_{}", self.name, attempt),
                mode,
            )
            .await;

            if let Err(err) = check_result {
                if attempt < attempts {
                    log::warn!(
                        "⚠️ Step '{}' status checks failed on attempt {}/{}: {}",
                        self.name,
                        attempt,
                        attempts,
                        err
                    );
                    sleep_1s().await;
                    self.run_retry_setup(session, cap, mode, attempt).await?;
                    continue;
                } else {
                    return Err(anyhow!(
                        "Step '{}' status checks failed after {} attempts: {}",
                        self.name,
                        attempts,
                        err
                    ));
                }
            }

            if let Err(err) = self.run_assertions(session, cap, mode, attempt).await {
                if attempt < attempts {
                    log::warn!(
                        "⚠️ Step '{}' assertions failed on attempt {}/{}: {}",
                        self.name,
                        attempt,
                        attempts,
                        err
                    );
                    sleep_1s().await;
                    self.run_retry_setup(session, cap, mode, attempt).await?;
                    continue;
                } else {
                    return Err(anyhow!(
                        "Step '{}' assertions failed after {} attempts: {}",
                        self.name,
                        attempts,
                        err
                    ));
                }
            }

            if attempt > 1 {
                log::info!(
                    "✅ Step '{}' succeeded on attempt {}/{}",
                    self.name,
                    attempt,
                    attempts
                );
            }
            return Ok(());
        }

        Err(anyhow!(
            "Step '{}' exhausted {} attempts without success",
            self.name,
            attempts
        ))
    }

    async fn run_retry_setup<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        mode: ActionExecutionMode,
        attempt: usize,
    ) -> Result<()> {
        if mode != ActionExecutionMode::Normal {
            return Ok(());
        }

        if let Some(setup) = &self.retry_setup {
            log::debug!(
                "Executing retry setup for step '{}' after attempt {}",
                self.name,
                attempt
            );
            execute_cursor_actions_with_mode(
                session,
                cap,
                setup,
                &format!("{}_retry_setup_{}", self.name, attempt),
                mode,
            )
            .await?;
        }

        Ok(())
    }

    async fn run_assertions<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        mode: ActionExecutionMode,
        attempt: usize,
    ) -> Result<()> {
        for assertion in &self.assertions {
            match assertion {
                ScreenAssertion::Pattern(spec) => {
                    spec.verify(
                        session,
                        cap,
                        &format!("{}_assertion_attempt_{}", self.name, attempt),
                        mode,
                    )
                    .await?;
                }
                ScreenAssertion::Capture(spec) => {
                    spec.verify(
                        session,
                        cap,
                        &format!("{}_assertion_attempt_{}", self.name, attempt),
                        mode,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionExecutionMode {
    /// Normal mode: execute all actions including keyboard input
    Normal,
    /// Screenshot generation mode: skip keyboard actions, only process screenshots and status updates
    GenerateScreenshots,
}

/// Execute a sequence of cursor actions on an expect session
/// In Normal mode: executes all actions including keyboard input
/// In GenerateScreenshots mode: skips keyboard actions and only processes status patches/assertions
pub async fn execute_cursor_actions<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    session_name: &str,
) -> Result<()> {
    execute_cursor_actions_with_mode(
        session,
        cap,
        actions,
        session_name,
        ActionExecutionMode::Normal,
    )
    .await
}

/// Execute cursor actions with specified execution mode and optional mock status
/// This is the internal implementation that supports both normal and screenshot generation modes
pub async fn execute_cursor_actions_with_mode<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    actions: &[CursorAction],
    session_name: &str,
    mode: ActionExecutionMode,
) -> Result<()> {
    log::debug!(
        "Executing cursor action batch '{}' with {} steps (mode: {:?})",
        session_name,
        actions.len(),
        mode
    );

    for (_idx, action) in actions.iter().enumerate() {
        match action {
            // In screenshot mode, keyboard actions are skipped
            CursorAction::PressArrow { direction, count }
                if mode == ActionExecutionMode::Normal =>
            {
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
                log::debug!(
                    "Skipping keyboard action in screenshot generation mode: {:?}",
                    action
                );
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
    assertions: &[ScreenAssertion],
    session_name: &str,
    max_retries: Option<usize>,
) -> Result<()> {
    let mut step = TuiStep::new(session_name.to_string())
        .with_actions(actions.to_vec())
        .with_status_checks(status_checks.to_vec())
        .with_assertions(assertions.to_vec());

    if let Some(retries) = max_retries {
        step = step.with_max_attempts(retries);
    }

    step.run(session, cap).await
}
