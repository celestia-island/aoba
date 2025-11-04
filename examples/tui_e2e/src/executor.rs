//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use std::time::Duration;

use anyhow::Result;

use crate::ipc::{IpcChannelId, IpcSender};
use crate::mock_state::{
    init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state,
};
use crate::placeholder::{
    clear_placeholders, generate_value, replace_placeholders, set_placeholder,
};
use crate::renderer::render_tui_to_string;
use crate::workflow::{Workflow, WorkflowStep};
use aoba_ci_utils::E2EToTuiMessage;

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

    // Clear placeholders from previous runs
    clear_placeholders();

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
async fn spawn_tui_with_ipc(ctx: &mut ExecutionContext, workflow_id: &str) -> Result<()> {
    // Generate unique IPC channel ID
    let channel_id = IpcChannelId(format!("{}_{}", workflow_id, std::process::id()));

    log::debug!("Generated IPC channel ID: {}", channel_id.0);

    // Start TUI process with --debug-ci flag
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.args(&[
        "run",
        "--package",
        "aoba",
        "--",
        "--tui",
        "--debug-ci",
        &channel_id.0,
    ]);

    log::info!(
        "🚀 Spawning TUI process: cargo run --package aoba -- --tui --debug-ci {}",
        channel_id.0
    );

    let child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn TUI process: {}", e))?;

    log::info!("✅ TUI process spawned with PID {}", child.id().unwrap_or(0));

    // Give TUI time to start and create IPC sockets
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Create IPC sender
    log::debug!("Connecting to IPC channel...");
    let sender = IpcSender::new(channel_id.clone()).await?;
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
        }
        // In ScreenCaptureOnly mode, keyboard actions are ignored
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

            // In DrillDown mode, simulate typing the value
            if ctx.mode == ExecutionMode::DrillDown {
                for ch in value.chars() {
                    simulate_char_input(ctx, ch).await?;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
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

    // Handle screen verification - render and verify content
    if step.verify.is_some() || step.verify_with_placeholder.is_some() {
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
                    anyhow::bail!("DrillDown mode requires IPC sender");
                }
            }
        };

        // Determine expected text
        let expected_text = if let Some(template) = &step.verify_with_placeholder {
            replace_placeholders(template)?
        } else if let Some(text) = &step.verify {
            text.clone()
        } else {
            String::new()
        };

        // Verify the expected text is present
        if let Some(line_num) = step.at_line {
            let lines: Vec<&str> = screen_content.lines().collect();
            if line_num >= lines.len() {
                anyhow::bail!(
                    "Line {} out of bounds (screen has {} lines)",
                    line_num,
                    lines.len()
                );
            }
            let actual_line = lines[line_num];
            if !actual_line.contains(&expected_text) {
                anyhow::bail!(
                    "Screen verification failed at line {}:\n  Expected text: '{}'\n  Actual line: '{}'\n  Full screen:\n{}",
                    line_num,
                    expected_text,
                    actual_line,
                    screen_content
                );
            }
        } else if !screen_content.contains(&expected_text) {
            anyhow::bail!(
                "Screen verification failed:\n  Expected text: '{}'\n  Not found in screen content:\n{}",
                expected_text,
                screen_content
            );
        }
        
        log::debug!("✅ Screen verified: '{}'", expected_text);
    }

    // Handle sleep
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

