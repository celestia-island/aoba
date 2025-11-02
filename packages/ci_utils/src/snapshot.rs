use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use expectrl::{process::NonBlocking, Expect};
use vt100::Parser;

use crate::helpers::sleep_1s;
use crate::status_monitor::TuiStatus;

/// Snapshot record for debugging
type SnapshotRecord = (String, String);

/// Execution mode for TUI E2E tests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Normal test mode: Execute keyboard actions and verify against reference screenshots
    Normal,
    /// Generate reference screenshots from predicted states without executing keyboard actions
    GenerateScreenshots,
}

/// Search condition types for snapshot matching
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SearchCondition {
    /// Match text content at specified line
    #[serde(rename = "text")]
    Text {
        value: String,
        #[serde(default)]
        negate: bool,
    },
    /// Match cursor position at specified line
    #[serde(rename = "cursor_line")]
    CursorLine { value: usize },
    /// Match placeholder at specified line
    #[serde(rename = "placeholder")]
    Placeholder {
        value: String,
        #[serde(default)]
        pattern: PlaceholderPattern,
    },
}

/// Special matching patterns for placeholders
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceholderPattern {
    /// Exact match - placeholder must match exactly
    Exact,
    /// Any boolean placeholder ({{0b#001}}, {{0b#002}}, etc.)
    AnyBoolean,
    /// Any decimal placeholder ({{#001}}, {{#002}}, etc.)
    AnyDecimal,
    /// Any hexadecimal placeholder ({{0x#001}}, {{0x#002}}, etc.)
    AnyHexadecimal,
    /// Any placeholder of any type
    Any,
    /// Range of placeholders (e.g., {{0b#001}} to {{0b#010}})
    Range { start: String, end: String },
    /// Pattern match using regex
    Pattern { regex: String },
}

impl Default for PlaceholderPattern {
    fn default() -> Self {
        PlaceholderPattern::Exact
    }
}

/// Snapshot definition for JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDefinition {
    /// Unique step name for identification (replaces index-based lookup)
    pub name: String,
    /// Description of what this snapshot should show
    pub description: String,
    /// Lines to check (0-indexed)
    pub line: Vec<usize>,
    /// Search conditions to verify
    pub search: Vec<SearchCondition>,
}

/// Context for managing snapshots during test execution
pub struct SnapshotContext {
    /// Current execution mode
    mode: ExecutionMode,
    /// Module name (e.g., "single_station/master_modes/coils")
    module_name: String,
    /// Test name (e.g., "test_basic_configuration")
    #[allow(dead_code)]
    test_name: String,
    /// Counter for snapshot numbering
    step_counter: u32,
    /// Base directory for snapshots
    snapshot_dir: PathBuf,
}

impl SnapshotContext {
    /// Create a new snapshot context
    pub fn new(mode: ExecutionMode, module_name: String, test_name: String) -> Self {
        let snapshot_dir = PathBuf::from("examples/tui_e2e/screenshots").join(&module_name);

        Self {
            mode,
            module_name,
            test_name,
            step_counter: 0,
            snapshot_dir,
        }
    }

    /// Get the execution mode associated with this context
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    /// Get the module path used for snapshot storage
    pub fn module_path(&self) -> &str {
        &self.module_name
    }

    /// Ensure the snapshot directory exists
    fn ensure_dir(&self) -> Result<()> {
        if !self.snapshot_dir.exists() {
            std::fs::create_dir_all(&self.snapshot_dir)?;
            log::debug!(
                "📁 Created snapshot directory: {}",
                self.snapshot_dir.display()
            );
        }
        Ok(())
    }

    /// Save snapshot definitions to JSON file
    pub fn save_snapshot_definitions(&self, definitions: &[SnapshotDefinition]) -> Result<()> {
        self.ensure_dir()?;

        let json_path = self.snapshot_dir.join("snapshots.json");
        let json_content = serde_json::to_string_pretty(definitions)?;
        std::fs::write(&json_path, json_content)?;

        log::info!("💾 Saved snapshot definitions: {}", json_path.display());
        Ok(())
    }

    /// Load snapshot definitions from JSON file
    pub fn load_snapshot_definitions(&self) -> Result<Vec<SnapshotDefinition>> {
        let json_path = self.snapshot_dir.join("snapshots.json");

        if !json_path.exists() {
            return Err(anyhow!(
                "Snapshot definitions not found: {}. Run with --generate-screenshots first.",
                json_path.display()
            ));
        }

        let content = std::fs::read_to_string(&json_path)?;
        let definitions: Vec<SnapshotDefinition> = serde_json::from_str(&content)?;

        log::info!("📖 Loaded {} snapshot definitions", definitions.len());
        Ok(definitions)
    }

    /// Load snapshot definitions from embedded JSON string (via include_str!)
    pub fn load_snapshot_definitions_from_str(json_str: &str) -> Result<Vec<SnapshotDefinition>> {
        let definitions: Vec<SnapshotDefinition> = serde_json::from_str(json_str)?;
        log::info!(
            "📖 Loaded {} snapshot definitions from embedded JSON",
            definitions.len()
        );
        Ok(definitions)
    }

    /// Find a snapshot definition by its step name
    pub fn find_definition_by_name<'a>(
        definitions: &'a [SnapshotDefinition],
        step_name: &str,
    ) -> Result<&'a SnapshotDefinition> {
        definitions
            .iter()
            .find(|def| def.name == step_name)
            .ok_or_else(|| {
                anyhow!(
                    "Snapshot definition not found for step '{}'. Available steps: {}",
                    step_name,
                    definitions
                        .iter()
                        .map(|d| d.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    /// Verify screen content against snapshot definitions
    pub fn verify_screen_against_definitions(
        &self,
        screen_content: &str,
        definitions: &[SnapshotDefinition],
        step_index: usize,
    ) -> Result<()> {
        if step_index >= definitions.len() {
            return Err(anyhow!(
                "Step index {} out of bounds (max {})",
                step_index,
                definitions.len() - 1
            ));
        }

        let definition = &definitions[step_index];
        let lines: Vec<&str> = screen_content.lines().collect();

        // Check specified lines exist
        for &line_num in &definition.line {
            if line_num >= lines.len() {
                return Err(anyhow!(
                    "Line {} not found in screen (only {} lines available)",
                    line_num,
                    lines.len()
                ));
            }
        }

        // Verify each search condition
        for condition in &definition.search {
            match condition {
                SearchCondition::Text { value, negate } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        if lines[line_num].contains(value) {
                            found = true;
                            break;
                        }
                    }

                    if *negate {
                        // For negate: text should NOT be found
                        if found {
                            return Err(anyhow!(
                                "Text '{}' should not be found in specified lines {:?}",
                                value,
                                definition.line
                            ));
                        }
                    } else {
                        // For normal: text should be found
                        if !found {
                            return Err(anyhow!(
                                "Text '{}' not found in specified lines {:?}",
                                value,
                                definition.line
                            ));
                        }
                    }
                }
                SearchCondition::CursorLine { value } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        if lines[line_num].contains('>') && line_num == *value {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(anyhow!(
                            "Cursor not found at line {} in specified lines {:?}",
                            value,
                            definition.line
                        ));
                    }
                }
                SearchCondition::Placeholder { value, pattern } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        let line = lines[line_num];

                        match pattern {
                            PlaceholderPattern::Exact => {
                                if line.contains(value) {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyBoolean => {
                                if line.contains("{{0b#") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyDecimal => {
                                if line.contains("{{#")
                                    && !line.contains("{{0x#")
                                    && !line.contains("{{0b#")
                                {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyHexadecimal => {
                                if line.contains("{{0x#") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Any => {
                                if line.contains("{{") && line.contains("}}") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Range { start: _, end: _ } => {
                                // For now, just check if any placeholder exists in the range
                                // In a full implementation, we would parse and compare placeholder indices
                                if line.contains("{{") && line.contains("}}") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Pattern { regex } => {
                                // For now, use simple contains as placeholder
                                // In a full implementation, we would use regex matching
                                if line.contains(&*regex) {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !found {
                        return Err(anyhow!(
                            "Placeholder pattern '{:?}' not found in specified lines {:?}",
                            pattern,
                            definition.line
                        ));
                    }
                }
            }
        }

        log::info!("✅ Snapshot verified: {}", definition.description);
        Ok(())
    }

    /// Verify screen content against a specific named snapshot step
    ///
    /// This is the new recommended method that uses step names instead of indices.
    pub fn verify_screen_by_step_name(
        screen_content: &str,
        definitions: &[SnapshotDefinition],
        step_name: &str,
    ) -> Result<()> {
        let definition = Self::find_definition_by_name(definitions, step_name)?;
        let lines: Vec<&str> = screen_content.lines().collect();

        // Check specified lines exist
        for &line_num in &definition.line {
            if line_num >= lines.len() {
                return Err(anyhow!(
                    "Line {} not found in screen (only {} lines available) for step '{}'",
                    line_num,
                    lines.len(),
                    step_name
                ));
            }
        }

        // Verify each search condition
        for condition in &definition.search {
            match condition {
                SearchCondition::Text { value, negate } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        if lines[line_num].contains(value) {
                            found = true;
                            break;
                        }
                    }

                    if *negate {
                        if found {
                            return Err(anyhow!(
                                "Step '{}': Text '{}' should not be found in specified lines {:?}",
                                step_name,
                                value,
                                definition.line
                            ));
                        }
                    } else {
                        if !found {
                            return Err(anyhow!(
                                "Step '{}': Text '{}' not found in specified lines {:?}",
                                step_name,
                                value,
                                definition.line
                            ));
                        }
                    }
                }
                SearchCondition::CursorLine { value } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        if lines[line_num].contains('>') && line_num == *value {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(anyhow!(
                            "Step '{}': Cursor not found at line {} in specified lines {:?}",
                            step_name,
                            value,
                            definition.line
                        ));
                    }
                }
                SearchCondition::Placeholder { value, pattern } => {
                    let mut found = false;
                    for &line_num in &definition.line {
                        let line = lines[line_num];

                        match pattern {
                            PlaceholderPattern::Exact => {
                                if line.contains(value) {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyBoolean => {
                                if line.contains("{{0b#") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyDecimal => {
                                if line.contains("{{#")
                                    && !line.contains("{{0x#")
                                    && !line.contains("{{0b#")
                                {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::AnyHexadecimal => {
                                if line.contains("{{0x#") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Any => {
                                if line.contains("{{") && line.contains("}}") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Range { start: _, end: _ } => {
                                if line.contains("{{") && line.contains("}}") {
                                    found = true;
                                    break;
                                }
                            }
                            PlaceholderPattern::Pattern { regex } => {
                                if line.contains(&**regex) {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !found {
                        return Err(anyhow!(
                            "Step '{}': Placeholder pattern '{:?}' not found in specified lines {:?}",
                            step_name,
                            pattern,
                            definition.line
                        ));
                    }
                }
            }
        }

        log::info!(
            "✅ Snapshot verified for step '{}': {}",
            step_name,
            definition.description
        );
        Ok(())
    }

    /// Capture or verify screenshot and state based on execution mode
    ///
    /// This is the main entry point for screenshot/state management.
    /// - In GenerateScreenshots mode: 
    ///   1. Write mock status to /tmp/status.json
    ///   2. Spawn TUI in --debug-screen-capture mode
    ///   3. Wait 3 seconds then kill the process
    ///   4. Capture terminal output via expectrl + vt100
    /// - In Normal mode: Verify actual terminal output against reference JSON rules
    pub async fn capture_or_verify<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        predicted_state: TuiStatus,
        step_name: &str,
    ) -> Result<String> {
        match self.mode {
            ExecutionMode::GenerateScreenshots => {
                // Capture mode: Write state and spawn TUI to generate screenshot
                log::info!("📸 Capture mode: Generating screenshot for step '{}'", step_name);
                
                // 1. Write mock global status to /tmp/status.json
                let status_json = serde_json::to_string_pretty(&predicted_state)?;
                std::fs::write("/tmp/status.json", &status_json)?;
                log::info!("💾 Wrote mock status to /tmp/status.json");
                
                // 2. Spawn TUI in debug capture mode
                log::info!("🚀 Spawning TUI in debug capture mode...");
                let mut tui_session = crate::terminal::spawn_expect_session_with_size(
                    &["--tui", "--debug-screen-capture", "--no-config-cache"],
                    Some(cap.parser.screen().size()),
                )?;
                
                // 3. Wait 3 seconds for rendering to complete
                log::info!("⏳ Waiting 3 seconds for TUI to render...");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                
                // 4. Capture terminal output
                let screen = cap.capture(&mut tui_session, &format!("capture_{}", step_name)).await?;
                
                // 5. Kill the TUI process
                log::info!("🛑 Killing TUI capture process...");
                drop(tui_session);
                
                log::info!("✅ Captured screenshot for step '{}'", step_name);
                Ok(screen)
            }
            ExecutionMode::Normal => {
                // Verify mode: Capture actual terminal and verify against JSON rules
                log::info!("🔍 Verify mode: Checking screenshot for step '{}'", step_name);
                
                // Capture current terminal state
                let screen = cap.capture(session, &format!("verify_{}", step_name)).await?;
                
                // Load snapshot definitions
                let definitions = self.load_snapshot_definitions()?;
                
                // Verify screen against the named step
                Self::verify_screen_by_step_name(&screen, &definitions, step_name)?;
                
                log::info!("✅ Verified screenshot for step '{}'", step_name);
                Ok(screen)
            }
        }
    }
}

// Legacy snapshot functionality (from original snapshot.rs)
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

/// Apply an incremental state change to a TuiStatus
///
/// This allows modifying the state using a closure without rewriting the entire object.
/// Similar to the `write_status` pattern used in the TUI.
///
/// # Example
/// ```ignore
/// let state = apply_state_change(state, |s| {
///     s.ports[0].enabled = true;
/// });
/// ```
pub fn apply_state_change<F>(mut state: TuiStatus, modifier: F) -> TuiStatus
where
    F: FnOnce(&mut TuiStatus),
{
    modifier(&mut state);
    state
}

/// Builder for creating base TUI states
pub struct StateBuilder {
    state: TuiStatus,
}

impl StateBuilder {
    /// Create a new state builder with default values
    pub fn new() -> Self {
        use chrono::Local;
        Self {
            state: TuiStatus {
                ports: Vec::new(),
                page: crate::status_monitor::TuiPage::Entry,
                timestamp: Local::now().to_rfc3339(),
            },
        }
    }

    /// Set the current page
    pub fn with_page(mut self, page: crate::status_monitor::TuiPage) -> Self {
        self.state.page = page;
        self
    }

    /// Add a port to the state
    pub fn add_port(mut self, port: crate::status_monitor::TuiPort) -> Self {
        self.state.ports.push(port);
        self
    }

    /// Build the final state
    pub fn build(self) -> TuiStatus {
        self.state
    }
}

impl Default for StateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify terminal screen content against JSON snapshot rules loaded via include_str!
///
/// This is a standalone verification function that can be used without SnapshotContext.
/// It's designed to work with JSON rules embedded in the test binary via `include_str!`.
///
/// # Arguments
/// * `screen_content` - The captured terminal screen content
/// * `json_rules` - The JSON rules string (typically from `include_str!("path/to/rules.json")`)
/// * `step_name` - The name of the step to verify (matches the "name" field in JSON)
///
/// # Example
/// ```no_run
/// use aoba_ci_utils::*;
///
/// const RULES: &str = include_str!("../screenshots/single_station/master_modes/coils.json");
///
/// async fn test_example() -> anyhow::Result<()> {
///     let mut session = spawn_expect_process(&["--tui"])?;
///     let mut cap = TerminalCapture::with_size(TerminalSize::Large);
///     
///     // Perform some actions...
///     let screen = cap.capture(&mut session, "after_action").await?;
///     
///     // Verify against specific step
///     verify_screen_with_json_rules(
///         &screen,
///         RULES,
///         "step_00_snapshot一次tmpvcom1_与_tmpvcom2_应当在屏幕上"
///     )?;
///     
///     Ok(())
/// }
/// ```
pub fn verify_screen_with_json_rules(
    screen_content: &str,
    json_rules: &str,
    step_name: &str,
) -> Result<()> {
    // Parse JSON rules
    let definitions = SnapshotContext::load_snapshot_definitions_from_str(json_rules)?;
    
    // Verify using the step name
    SnapshotContext::verify_screen_by_step_name(screen_content, &definitions, step_name)
}
