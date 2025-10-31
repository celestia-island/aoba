///! Screenshot generation and verification for TUI E2E tests
///!
///! This module provides infrastructure for:
///! - Generating reference screenshots from predicted TUI states
///! - Verifying actual terminal output against reference screenshots
///! - Incremental state modification using closure-based updates
///! - Strict verification of both screenshot content and global state

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::status_monitor::TuiStatus;
use crate::terminal::spawn_expect_session_with_size;
use crate::snapshot::{ExpectSession, TerminalCapture, TerminalSize};

/// Execution mode for TUI E2E tests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Normal test mode: Execute keyboard actions and verify against reference screenshots
    Normal,
    /// Generate reference screenshots from predicted states without executing keyboard actions
    GenerateScreenshots,
}

/// Context for managing screenshots during test execution
pub struct ScreenshotContext {
    /// Current execution mode
    mode: ExecutionMode,
    /// Module name (e.g., "tui_master_coils")
    #[allow(dead_code)]
    module_name: String,
    /// Test name (e.g., "test_basic_configuration")
    #[allow(dead_code)]
    test_name: String,
    /// Counter for screenshot numbering (001.txt, 002.txt, etc.)
    step_counter: AtomicU32,
    /// Base directory for screenshots
    screenshot_dir: PathBuf,
}

impl ScreenshotContext {
    /// Create a new screenshot context
    ///
    /// # Arguments
    /// * `mode` - Execution mode (Normal or GenerateScreenshots)
    /// * `module_name` - Module name for organizing screenshots
    /// * `test_name` - Test name for organizing screenshots
    pub fn new(mode: ExecutionMode, module_name: String, test_name: String) -> Self {
        let screenshot_dir = PathBuf::from("examples/tui_e2e/screenshots")
            .join(&module_name)
            .join(&test_name);
        
        Self {
            mode,
            module_name,
            test_name,
            step_counter: AtomicU32::new(0),
            screenshot_dir,
        }
    }

    /// Get the next screenshot filename
    fn next_filename(&self) -> String {
        let step = self.step_counter.fetch_add(1, Ordering::SeqCst);
        format!("{:03}.txt", step)
    }

    /// Get the full path for a screenshot file
    fn screenshot_path(&self, filename: &str) -> PathBuf {
        self.screenshot_dir.join(filename)
    }

    /// Ensure the screenshot directory exists
    fn ensure_dir(&self) -> Result<()> {
        if !self.screenshot_dir.exists() {
            std::fs::create_dir_all(&self.screenshot_dir)?;
            log::debug!("📁 Created screenshot directory: {}", self.screenshot_dir.display());
        }
        Ok(())
    }

    /// Generate a reference screenshot from a predicted state
    ///
    /// This spawns a TUI process in screen-capture mode with the predicted state,
    /// captures the terminal output, and saves it as a reference screenshot.
    async fn generate_reference_screenshot(
        &self,
        predicted_state: &TuiStatus,
        filename: &str,
    ) -> Result<String> {
        self.ensure_dir()?;

        // Serialize state to /tmp/status.json
        let status_json = serde_json::to_string_pretty(predicted_state)?;
        std::fs::write("/tmp/status.json", status_json)?;
        log::debug!("📄 Wrote predicted state to /tmp/status.json");

        // Spawn TUI in screen-capture mode
        let size = TerminalSize::Large;
        let (rows, cols) = size.dimensions();
        
        let mut session = spawn_expect_session_with_size(
            &["--tui", "--debug-screen-capture", "--no-config-cache"],
            Some((rows, cols)),
        )?;

        // Create terminal capture
        let mut cap = TerminalCapture::with_size(size);

        // Wait for TUI to initialize and render
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Capture the screen
        let screen_content = cap.capture(&mut session, "generate_screenshot").await?;

        // Send Ctrl+C to terminate TUI
        use crate::key_input::ExpectKeyExt;
        session.send_ctrl_c()?;

        // Wait for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Save screenshot
        let path = self.screenshot_path(filename);
        std::fs::write(&path, &screen_content)?;
        log::info!("💾 Saved reference screenshot: {}", path.display());

        Ok(screen_content)
    }

    /// Verify actual terminal output against reference screenshot
    ///
    /// This captures the current terminal content and compares it strictly
    /// with the reference screenshot. Any mismatch is a test failure.
    async fn verify_against_reference<T: ExpectSession>(
        &self,
        cap: &mut TerminalCapture,
        session: &mut T,
        filename: &str,
    ) -> Result<()> {
        let path = self.screenshot_path(filename);
        
        if !path.exists() {
            return Err(anyhow!(
                "Reference screenshot not found: {}. Run with --generate-screenshots first.",
                path.display()
            ));
        }

        // Read reference screenshot
        let reference = std::fs::read_to_string(&path)?;

        // Capture current screen
        let actual = cap.capture(session, "verify_screenshot").await?;

        // Strict comparison
        if actual.trim() != reference.trim() {
            log::error!("❌ Screenshot mismatch at {}", filename);
            log::error!("Expected:\n{}", reference);
            log::error!("Actual:\n{}", actual);
            return Err(anyhow!(
                "Screenshot verification failed for {}: content does not match reference",
                filename
            ));
        }

        log::info!("✅ Screenshot verified: {}", filename);
        Ok(())
    }

    /// Verify global state matches predicted state
    ///
    /// This reads the actual TUI global state and compares it strictly
    /// with the predicted state. Any mismatch is a test failure.
    fn verify_state(&self, predicted_state: &TuiStatus) -> Result<()> {
        let actual_state = crate::status_monitor::read_tui_status()?;

        // Strict comparison (serialize both and compare JSON)
        let predicted_json = serde_json::to_value(predicted_state)?;
        let actual_json = serde_json::to_value(&actual_state)?;

        // Compare page
        if predicted_json["page"] != actual_json["page"] {
            log::error!("❌ State mismatch: page");
            log::error!("Expected: {:?}", predicted_json["page"]);
            log::error!("Actual: {:?}", actual_json["page"]);
            return Err(anyhow!("State verification failed: page mismatch"));
        }

        // Compare ports (structure and key properties)
        let predicted_ports = predicted_json["ports"].as_array().unwrap();
        let actual_ports = actual_json["ports"].as_array().unwrap();

        if predicted_ports.len() != actual_ports.len() {
            return Err(anyhow!(
                "State verification failed: port count mismatch (expected {}, got {})",
                predicted_ports.len(),
                actual_ports.len()
            ));
        }

        for (i, (pred_port, actual_port)) in predicted_ports.iter().zip(actual_ports.iter()).enumerate() {
            // Check critical fields
            if pred_port["name"] != actual_port["name"] {
                return Err(anyhow!("State verification failed: port[{}] name mismatch", i));
            }
            if pred_port["enabled"] != actual_port["enabled"] {
                return Err(anyhow!("State verification failed: port[{}] enabled mismatch", i));
            }
            // Note: We don't check log_count as it may vary during test execution
        }

        log::info!("✅ State verified");
        Ok(())
    }

    /// Capture or verify screenshot and state based on execution mode
    ///
    /// This is the main entry point for screenshot/state management.
    /// - In GenerateScreenshots mode: Generates reference screenshot and saves predicted state
    /// - In Normal mode: Verifies actual output against reference and checks state
    pub async fn capture_or_verify<T: ExpectSession>(
        &self,
        session: &mut T,
        cap: &mut TerminalCapture,
        predicted_state: TuiStatus,
    ) -> Result<()> {
        let filename = self.next_filename();

        match self.mode {
            ExecutionMode::GenerateScreenshots => {
                // Generate reference screenshot from predicted state
                self.generate_reference_screenshot(&predicted_state, &filename).await?;
                log::info!("📸 Generated screenshot {}", filename);
            }
            ExecutionMode::Normal => {
                // Verify actual terminal output against reference
                self.verify_against_reference(cap, session, &filename).await?;
                
                // Verify actual state against predicted state
                self.verify_state(&predicted_state)?;
                
                log::info!("✅ Verified screenshot and state {}", filename);
            }
        }

        Ok(())
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
