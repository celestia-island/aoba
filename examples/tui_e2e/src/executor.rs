//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use crate::mock_state::{
    init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state,
};
use crate::placeholder::{
    clear_placeholders, generate_value, replace_placeholders, set_placeholder,
};
use crate::workflow::{Workflow, WorkflowStep};
use anyhow::{Context, Result};
use expectrl::Expect;
use std::time::Duration;

#[cfg(unix)]
type TuiSession =
    expectrl::Session<expectrl::process::unix::UnixProcess, expectrl::process::unix::PtyStream>;

#[cfg(windows)]
type TuiSession = expectrl::Session<
    expectrl::process::windows::WinProcess,
    expectrl::process::windows::WinptyStream,
>;

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Screen capture only - test rendering with mocked state
    ScreenCaptureOnly,
    /// Drill down - test real TUI with keyboard input
    DrillDown,
}

/// Execution context
pub struct ExecutionContext {
    pub mode: ExecutionMode,
    pub port1: String,
    pub port2: String,
    pub debug: bool,
}

/// Execute a complete workflow
pub async fn execute_workflow(ctx: &ExecutionContext, workflow: &Workflow) -> Result<()> {
    log::info!("🚀 Starting workflow execution: {}", workflow.manifest.id);
    log::info!("   Mode: {:?}", ctx.mode);

    // Clear placeholders from previous runs
    clear_placeholders();

    // Initialize mock state if in screen-capture mode
    if ctx.mode == ExecutionMode::ScreenCaptureOnly {
        init_mock_state();
        log::info!("🔧 Initialized mock state for screen-capture testing");
    }

    // Spawn TUI process (or mock it in screen-capture mode)
    let mut session = if ctx.mode == ExecutionMode::DrillDown {
        spawn_tui_process(&ctx.port1)?
    } else {
        spawn_mock_tui_process()?
    };

    // Execute init_order steps
    log::info!("📋 Executing init_order steps...");
    for step_name in &workflow.manifest.init_order {
        log::info!("  ▶️  Step: {}", step_name);

        let steps = workflow
            .workflow
            .get(step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in workflow", step_name))?;

        execute_step_sequence(ctx, &mut session, steps).await?;
    }

    // Execute recycle_order steps if any
    if !workflow.manifest.recycle_order.is_empty() {
        log::info!("📋 Executing recycle_order steps...");
        for step_name in &workflow.manifest.recycle_order {
            log::info!("  ▶️  Step: {}", step_name);

            let steps = workflow
                .workflow
                .get(step_name)
                .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in workflow", step_name))?;

            execute_step_sequence(ctx, &mut session, steps).await?;
        }
    }

    // Save mock state if in debug mode
    if ctx.debug && ctx.mode == ExecutionMode::ScreenCaptureOnly {
        save_mock_state_to_file("/tmp/tui_e2e_new_mock_state.json")?;
    }

    // Cleanup
    if ctx.mode == ExecutionMode::DrillDown {
        log::debug!("🧹 Terminating TUI process...");
        let _ = session.send("\x03"); // ETX (Ctrl+C)
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    log::info!("✅ Workflow execution completed successfully");
    Ok(())
}

/// Execute a sequence of workflow steps
async fn execute_step_sequence(
    ctx: &ExecutionContext,
    session: &mut TuiSession,
    steps: &[WorkflowStep],
) -> Result<()> {
    for (i, step) in steps.iter().enumerate() {
        if let Some(desc) = &step.description {
            log::debug!("    [{}] {}", i, desc);
        }

        execute_single_step(ctx, session, step).await?;
    }
    Ok(())
}

/// Execute a single workflow step
async fn execute_single_step(
    ctx: &ExecutionContext,
    session: &mut TuiSession,
    step: &WorkflowStep,
) -> Result<()> {
    // Handle key press (drill-down mode only)
    if let Some(key) = &step.key {
        if ctx.mode == ExecutionMode::DrillDown {
            let times = step.times.unwrap_or(1);
            for _ in 0..times {
                send_key(session, key)?;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    // Handle input generation and storage
    if let Some(input_type) = &step.input {
        if let Some(index) = step.index {
            // Generate value based on input type
            let value = if let Some(val) = &step.value {
                // Use provided value
                match val {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => val.to_string(),
                }
            } else {
                // Generate random value
                generate_value(input_type, None)
            };

            set_placeholder(index, value.clone());

            // Type the value if in drill-down mode
            if ctx.mode == ExecutionMode::DrillDown {
                for ch in value.chars() {
                    session.send(&ch.to_string())?;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
    }

    // Handle screen verification
    if let Some(expected_text) = &step.verify {
        if let Some(line_num) = step.at_line {
            verify_screen_text(session, expected_text, line_num)?;
        } else {
            verify_screen_contains(session, expected_text)?;
        }
    }

    // Handle screen verification with placeholders
    if let Some(template) = &step.verify_with_placeholder {
        let expected_text = replace_placeholders(template)?;
        if let Some(line_num) = step.at_line {
            verify_screen_text(session, &expected_text, line_num)?;
        } else {
            verify_screen_contains(session, &expected_text)?;
        }
    }

    // Handle cursor verification
    if let Some(line_num) = step.cursor_at_line {
        verify_cursor_position(session, line_num)?;
    }

    // Handle sleep
    if let Some(sleep_ms) = step.sleep_ms {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }

    // Handle mock state operations (screen-capture mode only)
    if ctx.mode == ExecutionMode::ScreenCaptureOnly {
        // Set mock state value
        if let Some(path) = &step.mock_path {
            let value = if let Some(template) = &step.mock_set_value_with_placeholder {
                let value_str = replace_placeholders(template)?;
                serde_json::json!(value_str)
            } else if let Some(value) = &step.mock_set_value {
                value.clone()
            } else {
                anyhow::bail!("mock_path specified but no value provided");
            };

            set_mock_state(path, value)?;
        }

        // Verify mock state value
        if let Some(path) = &step.mock_verify_path {
            let expected = step.mock_verify_value.as_ref().ok_or_else(|| {
                anyhow::anyhow!("mock_verify_path specified but no expected value")
            })?;
            verify_mock_state(path, expected)?;
        }
    }

    Ok(())
}

/// Spawn a TUI process for drill-down testing
fn spawn_tui_process(_port: &str) -> Result<TuiSession> {
    log::debug!("🚀 Spawning TUI process...");

    // Build the TUI binary
    let binary_path = build_tui_binary()?;

    // Create command
    let mut cmd = std::process::Command::new(&binary_path);
    cmd.args(&["--tui", "--debug-ci-e2e-test", "--no-config-cache"]);

    // Force deterministic locale
    cmd.env("LANGUAGE", "en_US:en");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    // Spawn with expectrl
    let session = expectrl::Session::spawn(cmd).context("Failed to spawn TUI process")?;

    // Wait for initial render
    std::thread::sleep(Duration::from_secs(2));

    log::debug!("✅ TUI process spawned");
    Ok(session)
}

/// Build TUI binary
fn build_tui_binary() -> Result<String> {
    use std::process::Command;

    log::debug!("🔨 Building TUI binary...");

    let output = Command::new("cargo")
        .args(&["build", "--package", "aoba"])
        .output()
        .context("Failed to build TUI binary")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to build TUI binary:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Return path to built binary
    Ok("target/debug/aoba".to_string())
}

/// Spawn a mock TUI process (for screen-capture testing)
fn spawn_mock_tui_process() -> Result<TuiSession> {
    log::debug!("🎭 Creating mock TUI session (screen-capture mode)");

    // For screen-capture mode, we don't actually spawn a process
    // Instead, we create a pseudo-terminal that we can write to/read from
    // This is a stub - in a real implementation, you'd mock the terminal

    // For now, return an error indicating this needs implementation
    anyhow::bail!("Mock TUI session not yet implemented - screen-capture mode requires further implementation")
}

/// Send a key to the terminal session
fn send_key(session: &mut TuiSession, key: &str) -> Result<()> {
    match key {
        "enter" => session.send("\r")?,
        "escape" => session.send("\x1b")?,
        "up" => session.send("\x1b[A")?,
        "down" => session.send("\x1b[B")?,
        "left" => session.send("\x1b[D")?,
        "right" => session.send("\x1b[C")?,
        "ctrl-a" => session.send("\x01")?, // SOH
        "ctrl-s" => session.send("\x13")?, // DC3
        "ctrl-pgup" => session.send("\x1b[5;5~")?,
        "backspace" => session.send("\x7f")?,
        "tab" => session.send("\t")?,
        _ => anyhow::bail!("Unknown key: {}", key),
    }
    Ok(())
}

/// Verify screen contains expected text
fn verify_screen_contains(session: &mut TuiSession, expected: &str) -> Result<()> {
    let screen = capture_screen(session)?;

    if !screen.contains(expected) {
        anyhow::bail!(
            "Screen verification failed: expected text '{}' not found\nScreen content:\n{}",
            expected,
            screen
        );
    }

    log::debug!("✅ Screen contains: '{}'", expected);
    Ok(())
}

/// Verify specific line contains expected text
fn verify_screen_text(session: &mut TuiSession, expected: &str, line_num: usize) -> Result<()> {
    let screen = capture_screen(session)?;
    let lines: Vec<&str> = screen.lines().collect();

    if line_num >= lines.len() {
        anyhow::bail!(
            "Line {} out of bounds (screen has {} lines)",
            line_num,
            lines.len()
        );
    }

    let actual_line = lines[line_num];
    if !actual_line.contains(expected) {
        anyhow::bail!(
            "Screen verification failed at line {}:\n  Expected: '{}'\n  Actual: '{}'",
            line_num,
            expected,
            actual_line
        );
    }

    log::debug!("✅ Line {} contains: '{}'", line_num, expected);
    Ok(())
}

/// Verify cursor position
fn verify_cursor_position(_session: &mut TuiSession, _line_num: usize) -> Result<()> {
    // This would need terminal cursor tracking
    log::warn!("⚠️  Cursor position verification not yet implemented");
    Ok(())
}

/// Capture current screen content
fn capture_screen(_session: &mut TuiSession) -> Result<String> {
    // For now, return a stub
    // Real implementation would need to properly parse the terminal buffer
    log::warn!("⚠️  Screen capture not fully implemented yet");
    Ok(String::new())
}
