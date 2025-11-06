//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use std::time::Duration;

use anyhow::{bail, Result};

use crate::mock_state::{
    init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state,
};
use crate::renderer::render_tui_to_string;
use crate::workflow::{Workflow, WorkflowStep};
use aoba_ci_utils::E2EToTuiMessage;
use aoba_ci_utils::{IpcChannelId, IpcSender};

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Screen capture only - test rendering with mocked state
    /// Uses TestBackend directly without TUI process
    ScreenCaptureOnly,
    /// Drill down - test real TUI with keyboard input via IPC
    ///
    /// This mode spawns a real TUI process with `--debug-ci` flag and communicates
    /// via IPC channel, sending keyboard events and receiving rendered frames.
    DrillDown,
}

/// Execution context
pub struct ExecutionContext {
    pub mode: ExecutionMode,
    pub port1: String,
    pub port2: String,
    pub debug: bool,
    pub ipc_sender: Option<IpcSender>,
}

/// Execute a complete workflow
pub async fn execute_workflow(ctx: &mut ExecutionContext, workflow: &Workflow) -> Result<()> {
    log::info!("🚀 Starting workflow execution: {}", workflow.manifest.id);
    log::info!("   Mode: {:?}", ctx.mode);

    // Initialize based on mode
    match ctx.mode {
        ExecutionMode::ScreenCaptureOnly => {
            // Screen capture mode: use mock state and TestBackend directly
            init_mock_state();
            log::info!("🔧 Initialized mock state for screen-capture testing");
        }
        ExecutionMode::DrillDown => {
            // DrillDown mode: spawn TUI process with IPC
            log::info!("🚀 Starting TUI process with IPC communication");
            spawn_tui_with_ipc(ctx, &workflow.manifest.id).await?;
        }
    }

    // Execute init_order steps
    log::info!("📋 Executing init_order steps...");
    for step_name in &workflow.manifest.init_order {
        log::info!("  ▶️  Step: {}", step_name);

        let steps = workflow
            .workflow
            .get(step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in workflow", step_name))?;

        execute_step_sequence(ctx, &workflow.manifest.id, steps).await?;
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

            execute_step_sequence(ctx, &workflow.manifest.id, steps).await?;
        }
    }

    // Save mock state if in debug mode
    if ctx.debug {
        save_mock_state_to_file("/tmp/tui_e2e_new_mock_state.json")?;
    }

    // Cleanup: shutdown TUI process if in DrillDown mode
    if ctx.mode == ExecutionMode::DrillDown {
        if let Some(sender) = &mut ctx.ipc_sender {
            log::info!("🛑 Shutting down TUI process");
            if let Err(err) = sender.send(E2EToTuiMessage::Shutdown).await {
                log::warn!("Failed to deliver shutdown message over IPC: {err}");
            }
        }
    }

    log::info!("✅ Workflow execution completed successfully");
    Ok(())
}

/// Spawn TUI process with IPC communication
async fn spawn_tui_with_ipc(ctx: &mut ExecutionContext, _workflow_id: &str) -> Result<()> {
    // Use a fixed IPC channel ID - no need to make it unique per test
    // The TUI process doesn't care which test is running
    let channel_id = IpcChannelId("tui_e2e".to_string());

    log::debug!("Using IPC channel ID: {}", channel_id.0);

    // Create IPC server sockets FIRST (before spawning TUI)
    // This eliminates timing issues - sockets are ready before TUI even starts
    log::info!("🔌 Creating IPC server sockets...");
    let ipc_sender_future = IpcSender::new(channel_id.clone());

    // Start TUI process with --debug-ci flag
    // Try to use pre-built binaries first (release preferred, debug fallback), then cargo run
    let release_bin = std::path::Path::new("target/release/aoba");
    let debug_bin = std::path::Path::new("target/debug/aoba");
    
    let mut cmd = if release_bin.exists() {
        let mut c = tokio::process::Command::new(release_bin);
        c.args(["--tui", "--debug-ci", &channel_id.0]);
        log::info!(
            "🚀 Spawning TUI process (release binary): target/release/aoba --tui --debug-ci {}",
            channel_id.0
        );
        c
    } else if debug_bin.exists() {
        let mut c = tokio::process::Command::new(debug_bin);
        c.args(["--tui", "--debug-ci", &channel_id.0]);
        log::info!(
            "🚀 Spawning TUI process (debug binary): target/debug/aoba --tui --debug-ci {}",
            channel_id.0
        );
        c
    } else {
        // No pre-built binary - fall back to cargo run (debug is much faster than release)
        let mut c = tokio::process::Command::new("cargo");
        c.args([
            "run",
            "--package",
            "aoba",
            "--",
            "--tui",
            "--debug-ci",
            &channel_id.0,
        ]);
        log::info!(
            "🚀 Spawning TUI process (cargo debug): cargo run --package aoba -- --tui --debug-ci {}",
            channel_id.0
        );
        log::warn!("⚠️  No pre-built binary found. Using cargo run (slow). Run 'cargo build --package aoba' first for faster testing.");
        c
    };

    // Force English locale so string-based assertions remain deterministic across hosts.
    cmd.env("LANGUAGE", "en_US");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    let child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn TUI process: {}", e))?;

    log::info!(
        "✅ TUI process spawned with PID {}",
        child.id().unwrap_or(0)
    );

    // Wait for TUI to connect to our IPC server
    // This will complete when TUI starts and connects (no timeout issues!)
    let sender = ipc_sender_future.await?;
    log::info!("✅ IPC connection established");

    ctx.ipc_sender = Some(sender);

    Ok(())
}

/// Execute a sequence of workflow steps
async fn execute_step_sequence(
    ctx: &mut ExecutionContext,
    workflow_id: &str,
    steps: &[WorkflowStep],
) -> Result<()> {
    for (i, step) in steps.iter().enumerate() {
        if let Some(desc) = &step.description {
            log::debug!("    [{}] {}", i, desc);
        }

        execute_single_step(ctx, workflow_id, step).await?;
    }
    Ok(())
}

/// Execute a single workflow step
async fn execute_single_step(
    ctx: &mut ExecutionContext,
    _workflow_id: &str,
    step: &WorkflowStep,
) -> Result<()> {
    // Handle keyboard input (DrillDown mode)
    if let Some(key) = &step.key {
        if ctx.mode == ExecutionMode::DrillDown {
            let times = step.times.unwrap_or(1);
            for _ in 0..times {
                // Send keyboard input via IPC
                simulate_key_input(ctx, key).await?;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            // Short delay after key press to allow TUI to process
            // Use explicit sleep_ms if specified, otherwise 200ms default
            if step.sleep_ms.is_none() {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        // In ScreenCaptureOnly mode, keyboard actions are ignored
    }

    // Handle input value typing
    if let Some(input_type) = &step.input {
        // Use provided value
        let value = if let Some(val) = &step.value {
            match val {
                serde_json::Value::Number(n) => {
                    // Format numbers according to input type
                    if input_type == "hex" {
                        format!("{:04X}", n.as_u64().unwrap_or(0))
                    } else {
                        n.to_string()
                    }
                }
                serde_json::Value::String(s) => s.clone(),
                _ => val.to_string(),
            }
        } else {
            bail!("input specified but no value provided");
        };

        // In DrillDown mode, simulate typing the value
        if ctx.mode == ExecutionMode::DrillDown {
            for ch in value.chars() {
                simulate_char_input(ctx, ch).await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    // Handle mock state operations (screen-capture mode only)
    if ctx.mode == ExecutionMode::ScreenCaptureOnly {
        // Set mock state value
        if let Some(path) = &step.mock_path {
            let value = if let Some(value) = &step.mock_set_value {
                value.clone()
            } else {
                bail!("mock_path specified but no value provided");
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

    // Handle screen verification - render and verify content
    if step.verify.is_some() {
        // In DrillDown mode, give extra time for TUI to render before requesting screen
        if ctx.mode == ExecutionMode::DrillDown {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        
        // Render the TUI to a string based on execution mode
        let screen_content = match ctx.mode {
            ExecutionMode::ScreenCaptureOnly => {
                // Use TestBackend directly
                render_tui_to_string(120, 40)?
            }
            ExecutionMode::DrillDown => {
                // Request screen from TUI process via IPC
                if let Some(sender) = ctx.ipc_sender.as_mut() {
                    let (content, _width, _height) =
                        crate::renderer::render_tui_via_ipc(sender).await?;
                    content
                } else {
                    bail!("DrillDown mode requires IPC sender");
                }
            }
        };

        // Get expected text
        let expected_text = step.verify.as_ref().unwrap().clone();

        // Verify the expected text is present
        if let Some(line_num) = step.at_line {
            let lines: Vec<&str> = screen_content.lines().collect();
            if line_num >= lines.len() {
                bail!(
                    "Line {} out of bounds (screen has {} lines)",
                    line_num,
                    lines.len()
                );
            }
            let actual_line = lines[line_num];
            if !actual_line.contains(&expected_text) {
                bail!(
                    "Screen verification failed at line {}:\n  Expected text: '{}'\n  Actual line: '{}'\n  Full screen:\n{}",
                    line_num,
                    expected_text,
                    actual_line,
                    screen_content
                );
            }
        } else if !screen_content.contains(&expected_text) {
            bail!(
                "Screen verification failed:\n  Expected text: '{}'\n  Not found in screen content:\n{}",
                expected_text,
                screen_content
            );
        }

        log::debug!("✅ Screen verified: '{}'", expected_text);
    }

    // Handle sleep - use explicit sleep_ms if provided, otherwise no extra sleep
    if let Some(sleep_ms) = step.sleep_ms {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }

    Ok(())
}

/// Simulate keyboard input by sending it via IPC
///
/// This function is used in DrillDown mode to simulate keyboard events.
/// It sends the keyboard event through the IPC channel to the TUI process.
async fn simulate_key_input(ctx: &mut ExecutionContext, key: &str) -> Result<()> {
    log::debug!("🎹 Simulating key press: {}", key);

    if let Some(sender) = ctx.ipc_sender.as_mut() {
        sender
            .send(E2EToTuiMessage::KeyPress {
                key: key.to_string(),
            })
            .await?;
        log::debug!("   ✅ Key sent via IPC: {}", key);
    } else {
        log::warn!("   ⚠️  No IPC sender available");
    }

    Ok(())
}

/// Simulate character input (typing) by sending it via IPC
///
/// This function is used in DrillDown mode to simulate typing characters.
async fn simulate_char_input(ctx: &mut ExecutionContext, ch: char) -> Result<()> {
    log::debug!("🎹 Simulating character input: '{}'", ch);

    if let Some(sender) = ctx.ipc_sender.as_mut() {
        sender.send(E2EToTuiMessage::CharInput { ch }).await?;
        log::debug!("   ✅ Character sent via IPC: '{}'", ch);
    } else {
        log::warn!("   ⚠️  No IPC sender available");
    }

    Ok(())
}
