//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use std::time::Duration;

use anyhow::{bail, Result};

use crate::mock_state::{
    init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state,
};
use crate::placeholder::{
    clear_placeholders, generate_value, get_register_value, init_register_pools, register_truthy,
    register_value_as_string, register_value_to_json, replace_placeholders, set_placeholder,
    RegisterPoolKind, RegisterValue,
};
use crate::renderer::render_tui_to_string;
use crate::workflow::{Manifest, MockSyncValueSpec, PressIfSpec, Workflow, WorkflowStep};
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

    // Clear placeholders from previous runs
    clear_placeholders();

    // Initialize register pools so workflows can draw from deterministic random data
    initialize_register_pools_for_workflow(&workflow.manifest);

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

    // Force English locale so string-based assertions remain deterministic across hosts.
    cmd.env("LANGUAGE", "en_US");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

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

    log::info!(
        "✅ TUI process spawned with PID {}",
        child.id().unwrap_or(0)
    );

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
    // Skip the entire step if the press_if guard evaluates to false
    if let Some(condition) = &step.press_if {
        if !evaluate_press_if(condition)? {
            log::debug!("    ⏭️  press_if condition evaluated to false, skipping step");
            return Ok(());
        }
    }

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
    let mut handled_input = false;

    if let Some(input_spec) = &step.input_register {
        handled_input = true;

        let register_value = get_register_value(input_spec.pool, input_spec.index)?;
        let formatted = register_value_as_string(register_value, input_spec.format.as_deref());

        if let Some(index) = step.index {
            set_placeholder(index, formatted.clone());
        }

        if ctx.mode == ExecutionMode::DrillDown && input_spec.type_to_tui {
            for ch in formatted.chars() {
                simulate_char_input(ctx, ch).await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    if !handled_input {
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

        // Synchronize register value into mock state when requested
        if let Some(spec) = &step.mock_sync_value {
            sync_register_into_mock_state(spec)?;
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
                    bail!("DrillDown mode requires IPC sender");
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

    // Handle sleep
    if let Some(sleep_ms) = step.sleep_ms {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }

    Ok(())
}

fn initialize_register_pools_for_workflow(manifest: &Manifest) {
    let (bool_count, int_count) = determine_register_pool_sizes(manifest);
    init_register_pools(bool_count, int_count);
}

fn determine_register_pool_sizes(manifest: &Manifest) -> (usize, usize) {
    let mut bool_total = 0usize;
    let mut int_total = 0usize;

    if let Some(stations) = &manifest.stations {
        for station in stations {
            match classify_register_type(&station.register_type) {
                RegisterPoolKind::Bool => bool_total += station.register_count as usize,
                RegisterPoolKind::Int => int_total += station.register_count as usize,
            }
        }
    } else if let (Some(register_type), Some(count)) =
        (manifest.register_type.as_deref(), manifest.register_count)
    {
        match classify_register_type(register_type) {
            RegisterPoolKind::Bool => bool_total = bool_total.max(count as usize),
            RegisterPoolKind::Int => int_total = int_total.max(count as usize),
        }
    }

    // Provide a baseline pool so workflows referencing generic indices always succeed.
    let minimum_pool = 16usize;
    bool_total = bool_total.max(minimum_pool);
    int_total = int_total.max(minimum_pool);

    (bool_total, int_total)
}

fn classify_register_type(register_type: &str) -> RegisterPoolKind {
    match register_type.to_ascii_lowercase().as_str() {
        "coils" | "discrete_inputs" | "discreteinputs" => RegisterPoolKind::Bool,
        _ => RegisterPoolKind::Int,
    }
}

fn evaluate_press_if(condition: &PressIfSpec) -> Result<bool> {
    if let Some(expected) = &condition.equals {
        let value = get_register_value(condition.pool, condition.index)?;
        match value {
            RegisterValue::Bool(actual) => {
                if let Some(expected_bool) = expected.as_bool() {
                    Ok(actual == expected_bool)
                } else if let Some(expected_number) = expected.as_i64() {
                    Ok(actual == (expected_number != 0))
                } else if expected.is_null() {
                    Ok(!actual)
                } else if let Some(expected_str) = expected.as_str() {
                    match expected_str.to_ascii_lowercase().as_str() {
                        "true" | "1" => Ok(actual),
                        "false" | "0" => Ok(!actual),
                        other => Ok(actual.to_string() == other),
                    }
                } else {
                    bail!("Unsupported equals comparison for boolean press_if")
                }
            }
            RegisterValue::Int(actual) => {
                if let Some(expected_number) = expected.as_u64() {
                    Ok(actual as u64 == expected_number)
                } else if let Some(expected_number) = expected.as_i64() {
                    if expected_number < 0 {
                        Ok(false)
                    } else {
                        Ok(actual as u64 == expected_number as u64)
                    }
                } else if let Some(expected_str) = expected.as_str() {
                    let normalized = expected_str.trim();
                    if let Some(hex) = normalized.strip_prefix("0x") {
                        let expected_value = u16::from_str_radix(hex, 16)?;
                        Ok(actual == expected_value)
                    } else if let Some(bin) = normalized.strip_prefix("0b") {
                        let expected_value = u16::from_str_radix(bin, 2)?;
                        Ok(actual == expected_value)
                    } else {
                        let parsed = normalized.parse::<u16>()?;
                        Ok(actual == parsed)
                    }
                } else {
                    bail!("Unsupported equals comparison for integer press_if")
                }
            }
        }
    } else {
        register_truthy(condition.pool, condition.index)
    }
}

fn sync_register_into_mock_state(spec: &MockSyncValueSpec) -> Result<()> {
    let value = get_register_value(spec.pool, spec.index)?;
    let json_value = register_value_to_json(value);
    set_mock_state(&spec.path, json_value)
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
