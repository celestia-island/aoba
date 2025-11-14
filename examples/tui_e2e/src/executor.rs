//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use anyhow::{bail, Result};
use std::fmt;

use crate::{
    mock_state::{init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state},
    renderer::render_tui_to_string,
    retry_state_machine::{group_steps, is_verification_step, StepGroup},
    workflow::{Workflow, WorkflowStep},
};
use _main::utils::{
    {sleep_1s, sleep_3s}, {E2EToTuiMessage, IpcChannelId, IpcSender},
};

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
    pub is_slave_test: bool,
    pub in_modbus_panel: bool,
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

            // Generate and spawn CLI emulator for this test
            // This is needed for ALL tests (single and multi-station) to provide
            // a Modbus counterpart for the TUI to communicate with
            spawn_cli_emulator(ctx, workflow).await?;
        }
    }

    // Execute init_order steps
    log::info!("📋 Executing init_order steps...");
    for step_name in &workflow.manifest.init_order {
        log::info!("  ▶️  Step: {step_name}");

        // Track when we enter the modbus panel
        if step_name == "enter_modbus_panel" {
            ctx.in_modbus_panel = true;
            log::debug!(
                "📍 Entering modbus panel - slave line adjustment enabled: {}",
                ctx.is_slave_test
            );
        }

        let steps = workflow
            .workflow
            .get(step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{step_name}' not found in workflow"))?;

        execute_step_sequence(ctx, &workflow.manifest.id, steps).await?;
    }

    // Execute recycle_order steps if any
    if !workflow.manifest.recycle_order.is_empty() {
        log::info!("📋 Executing recycle_order steps...");
        for step_name in &workflow.manifest.recycle_order {
            log::info!("  ▶️  Step: {step_name}");

            // Track when we enter the modbus panel (may happen in recycle too)
            if step_name == "enter_modbus_panel" {
                ctx.in_modbus_panel = true;
                log::debug!(
                    "📍 Entering modbus panel - slave line adjustment enabled: {}",
                    ctx.is_slave_test
                );
            }

            let steps = workflow
                .workflow
                .get(step_name)
                .ok_or_else(|| anyhow::anyhow!("Step '{step_name}' not found in workflow"))?;

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
        .map_err(|e| anyhow::anyhow!("Failed to spawn TUI process: {e}"))?;

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

/// Spawn CLI emulator process to act as Modbus counterpart for TUI
///
/// Generates a JSON config file and spawns CLI with --config flag.
/// For master mode tests, CLI acts as slave.
/// For slave mode tests, CLI acts as master.
async fn spawn_cli_emulator(ctx: &ExecutionContext, workflow: &Workflow) -> Result<()> {
    use _main::protocol::status::types::modbus::StationMode;
    use serde_json::json;

    // Determine TUI mode (default to Master if not specified)
    let tui_mode = workflow.manifest.mode.unwrap_or(StationMode::Master);

    // CLI acts as opposite mode: if TUI is master, CLI is slave (and vice versa)
    let cli_mode = if tui_mode.is_master() {
        "slave"
    } else {
        "master"
    };

    log::info!("📡 Preparing CLI emulator (TUI is {tui_mode:?}, CLI is {cli_mode})");

    // Build station configurations
    let stations = if let Some(ref multi_stations) = workflow.manifest.stations {
        // Multi-station test
        log::info!("   Multi-station config: {} stations", multi_stations.len());

        let mut station_configs = Vec::new();
        for station in multi_stations {
            let register_field = match station.register_type.as_str() {
                "Holding" | "holding" => "holding",
                "Coils" | "coils" => "coils",
                "DiscreteInputs" | "discrete_inputs" => "discrete_inputs",
                "Input" | "input" => "input",
                _ => bail!("Unknown register type: {}", station.register_type),
            };

            let config = json!({
                "id": station.station_id,
                "mode": cli_mode,
                "map": {
                    register_field: [{
                        "address_start": station.start_address,
                        "length": station.register_count,
                        "initial_values": []
                    }]
                }
            });

            station_configs.push(config);
        }
        station_configs
    } else {
        // Single-station test
        log::info!("   Single-station config");

        let station_id = workflow
            .manifest
            .station_id
            .ok_or_else(|| anyhow::anyhow!("Missing station_id in manifest"))?;
        let register_type = workflow
            .manifest
            .register_type
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing register_type in manifest"))?;
        let start_address = workflow
            .manifest
            .start_address
            .ok_or_else(|| anyhow::anyhow!("Missing start_address in manifest"))?;
        let register_count = workflow
            .manifest
            .register_count
            .ok_or_else(|| anyhow::anyhow!("Missing register_count in manifest"))?;

        let register_field = match register_type.as_str() {
            "Holding" | "holding" => "holding",
            "Coils" | "coils" => "coils",
            "DiscreteInputs" | "discrete_inputs" => "discrete_inputs",
            "Input" | "input" => "input",
            _ => bail!("Unknown register type: {register_type}"),
        };

        vec![json!({
            "id": station_id,
            "mode": cli_mode,
            "map": {
                register_field: [{
                    "address_start": start_address,
                    "length": register_count,
                    "initial_values": []
                }]
            }
        })]
    };

    // Build complete CLI config
    let config = json!({
        "port_name": ctx.port2,  // CLI uses port2 to connect to TUI on port1
        "baud_rate": 9600,
        "communication_params": {
            "mode": "stdio",
            "dynamic_pull": false,
            "wait_time": 1.0,
            "timeout": 3.0,
            "persistence": "persistent"
        },
        "stations": stations
    });

    // Write config to fixed location
    let config_path = "/tmp/tui_e2e_emulator.json";
    std::fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
    log::info!("📝 Wrote CLI config to {config_path}");
    log::debug!(
        "Config: {config}",
        config = serde_json::to_string_pretty(&config)?
    );

    // Find CLI binary
    let cli_binary = if std::path::Path::new("target/debug/aoba").exists() {
        "target/debug/aoba"
    } else if std::path::Path::new("target/release/aoba").exists() {
        "target/release/aoba"
    } else {
        bail!("aoba binary not found in target/debug or target/release");
    };

    // Spawn CLI emulator process
    log::info!("🚀 Spawning CLI emulator: {cli_binary} --config {config_path}");

    let _child = tokio::process::Command::new(cli_binary)
        .args(["--config", config_path])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn CLI emulator: {e}"))?;

    log::info!("✅ CLI emulator spawned successfully");

    // Give CLI time to initialize
    sleep_3s().await;

    Ok(())
}

/// Execute a sequence of workflow steps
async fn execute_step_sequence(
    ctx: &mut ExecutionContext,
    workflow_id: &str,
    steps: &[WorkflowStep],
) -> Result<()> {
    // Group steps into retryable units
    let groups = group_steps(steps);

    if !groups.is_empty() {
        log::debug!("📦 Identified {} retryable step groups", groups.len());
    }

    let mut current_group_idx = 0;
    let mut i = 0;

    while i < steps.len() {
        let step = &steps[i];

        // Check if this step is part of a retryable group
        if current_group_idx < groups.len() && i == groups[current_group_idx].step_indices[0] {
            // Execute this group with retry logic
            let group = &groups[current_group_idx];
            execute_step_group_with_retry(ctx, workflow_id, steps, group).await?;

            // Skip to next step after group
            i = group.step_indices.last().unwrap() + 1;
            current_group_idx += 1;
        } else {
            // Execute single step normally (not part of any group)
            if let Some(desc) = &step.description {
                log::debug!("    [{i}] {desc}");
            }
            execute_single_step(ctx, workflow_id, step).await?;
            i += 1;
        }
    }
    Ok(())
}

/// Execute a step group with retry logic
async fn execute_step_group_with_retry(
    ctx: &mut ExecutionContext,
    workflow_id: &str,
    all_steps: &[WorkflowStep],
    group: &StepGroup,
) -> Result<()> {
    let mut retry_count = 0;
    let max_retries = group.max_retries;
    let mut first_failure_screenshot: Option<String> = None;
    let mut last_error: Option<anyhow::Error> = None;

    loop {
        if retry_count > 0 {
            let attempt = retry_count + 1;
            log::info!(
                "🔁 Retrying step group (attempt {}/{})",
                attempt,
                max_retries + 1
            );

            if !group.action_indices.is_empty() {
                log::info!(
                    "   ⏪ Replaying {} action step(s) before verification",
                    group.action_indices.len()
                );
                for &action_idx in &group.action_indices {
                    log::info!("     ↻ {}", step_summary(&all_steps[action_idx]));
                }
            }

            // Wait 1 second before retrying the action steps
            sleep_3s().await;
        }

        let mut verification_failed = false;

        for &step_idx in &group.step_indices {
            let step = &all_steps[step_idx];

            if let Some(desc) = &step.description {
                log::debug!("    [{step_idx}] {desc}");
            }

            match execute_single_step(ctx, workflow_id, step).await {
                Ok(()) => {}
                Err(err) => {
                    if is_verification_step(step) {
                        verification_failed = true;
                        last_error = Some(err);

                        if first_failure_screenshot.is_none() {
                            match capture_screenshot(ctx).await {
                                Ok(captured) => {
                                    log::debug!(
                                        "📸 Captured first failure screenshot for later reporting"
                                    );
                                    first_failure_screenshot = Some(captured);
                                }
                                Err(capture_err) => {
                                    log::warn!(
                                        "⚠️  Failed to capture screenshot after verification error: {capture_err}",
                                    );
                                }
                            }
                        }

                        if let Some(err_ref) = last_error.as_ref() {
                            log::warn!("   Verification failed: {err_ref}");
                        }
                        break;
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        if !verification_failed {
            if retry_count > 0 {
                let suffix = if retry_count == 1 {
                    " retry"
                } else {
                    " retries"
                };
                log::info!("✅ Step group succeeded after {retry_count}{suffix}");
            }
            return Ok(());
        }

        retry_count += 1;
        if retry_count > max_retries {
            log::error!("❌ Step group failed after {} attempts", max_retries + 1);

            if let Some(screenshot) = first_failure_screenshot {
                log::error!("📸 Screenshot from first failure:\n{screenshot}");
            }

            return Err(last_error.unwrap_or_else(|| {
                anyhow::anyhow!("Step group failed after {max_retries} retries")
            }));
        }
    }
}

/// Capture screenshot from current TUI state
async fn capture_screenshot(ctx: &mut ExecutionContext) -> Result<String> {
    match ctx.mode {
        ExecutionMode::ScreenCaptureOnly => render_tui_to_string(160, 50),
        ExecutionMode::DrillDown => {
            if let Some(sender) = ctx.ipc_sender.as_mut() {
                let (content, _width, _height) =
                    crate::renderer::render_tui_via_ipc(sender).await?;
                Ok(content)
            } else {
                bail!("DrillDown mode requires IPC sender");
            }
        }
    }
}

fn step_summary(step: &WorkflowStep) -> String {
    if let Some(description) = &step.description {
        return description.clone();
    }

    if let Some(key) = &step.key {
        if let Some(times) = step.times {
            return format!("Key '{key}' ×{times}");
        }
        return format!("Key '{key}'");
    }

    if let Some(input_kind) = &step.input {
        if let Some(value) = &step.value {
            return format!("Input {input_kind} = {value}");
        }
        return format!("Input {input_kind}");
    }

    if let Some(expected) = &step.verify {
        return format!("Verify '{expected}'");
    }

    "Unnamed step".to_string()
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
                sleep_1s().await;
            }
            // Short delay after key press to allow TUI to process
            // Use explicit sleep_ms if specified, otherwise 200ms default
            if step.sleep_ms.is_none() {
                sleep_1s().await;
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

        log::info!("🎹 Typing value: '{value}' (type: {input_type})");

        // In DrillDown mode, simulate typing the value
        if ctx.mode == ExecutionMode::DrillDown {
            for ch in value.chars() {
                simulate_char_input(ctx, ch).await?;
                sleep_1s().await;
            }
            // Add delay after typing to ensure all characters are processed by TUI
            // TUI has 100ms delay after each char, so we need to wait for that plus render time
            sleep_3s().await;
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
            sleep_1s().await;
        }

        // Render the TUI to a string based on execution mode
        let screen_content = match ctx.mode {
            ExecutionMode::ScreenCaptureOnly => {
                // Use TestBackend directly
                render_tui_to_string(160, 50)?
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

        let verification_result: Result<(), VerificationFailure> =
            if let Some(mut line_num) = step.at_line {
                // For slave tests in the modbus panel, adjust line numbers for group 2+ content
                // Group 2+ starts after the separator, which is at line 2 in master mode
                // In slave mode, we add RequestInterval and Timeout fields, so group 2+ starts at line 4
                // Therefore, for any line_num >= 2 checking station content, we add 2
                if ctx.is_slave_test && ctx.in_modbus_panel && line_num >= 2 {
                    line_num += 2;
                    log::debug!(
                        "🔧 Adjusted line number for slave mode: {} -> {}",
                        line_num - 2,
                        line_num
                    );
                }

                let lines: Vec<&str> = screen_content.lines().collect();
                if line_num >= lines.len() {
                    Err(VerificationFailure::new(
                        expected_text.clone(),
                        Some(format!(
                            "line {} out of bounds (screen has {} lines)",
                            line_num,
                            lines.len()
                        )),
                    ))
                } else {
                    let actual_line = lines[line_num];
                    if actual_line.contains(&expected_text) {
                        Ok(())
                    } else {
                        Err(VerificationFailure::new(
                            expected_text.clone(),
                            Some(format!(
                                "line {} mismatch: '{}'",
                                line_num,
                                actual_line.trim_end()
                            )),
                        ))
                    }
                }
            } else if screen_content.contains(&expected_text) {
                Ok(())
            } else {
                // Log the actual screen content for debugging
                log::error!(
                    "❌ Verification failed. Expected text not found: '{}'",
                    expected_text
                );
                log::error!("📺 Actual screen content:\n{}", screen_content);
                log::error!(
                    "📏 Screen size: {} lines, {} chars total",
                    screen_content.lines().count(),
                    screen_content.len()
                );

                Err(VerificationFailure::new(
                    expected_text.clone(),
                    Some("expected text not present on screen".to_string()),
                ))
            };

        verification_result.map_err(anyhow::Error::from)?;

        log::debug!("✅ Screen verified: '{expected_text}'");
    }

    // Handle triggers - custom actions
    if let Some(trigger_name) = &step.trigger {
        if ctx.mode == ExecutionMode::DrillDown {
            log::info!("🔔 Executing trigger: {trigger_name}");
            execute_trigger(ctx, trigger_name, &step.trigger_params).await?;
        }
    }

    // Handle sleep - use explicit sleep_ms if provided, otherwise no extra sleep
    if let Some(sleep_ms) = step.sleep_ms {
        if sleep_ms <= 1000 {
            sleep_1s().await;
        } else {
            sleep_3s().await;
        }
    }

    Ok(())
}

/// Simulate keyboard input by sending it via IPC
///
/// This function is used in DrillDown mode to simulate keyboard events.
/// It sends the keyboard event through the IPC channel to the TUI process.
async fn simulate_key_input(ctx: &mut ExecutionContext, key: &str) -> Result<()> {
    log::debug!("🎹 Simulating key press: {key}");

    if let Some(sender) = ctx.ipc_sender.as_mut() {
        sender
            .send(E2EToTuiMessage::KeyPress {
                key: key.to_string(),
            })
            .await?;
        log::debug!("   ✅ Key sent via IPC: {key}");
    } else {
        log::warn!("   ⚠️  No IPC sender available");
    }

    Ok(())
}

/// Simulate character input (typing) by sending it via IPC
///
/// This function is used in DrillDown mode to simulate typing characters.
async fn simulate_char_input(ctx: &mut ExecutionContext, ch: char) -> Result<()> {
    log::debug!("🎹 Simulating character input: '{ch}'");

    if let Some(sender) = ctx.ipc_sender.as_mut() {
        sender.send(E2EToTuiMessage::CharInput { ch }).await?;
        log::debug!("   ✅ Character sent via IPC: '{ch}'");
    } else {
        log::warn!("   ⚠️  No IPC sender available");
    }

    Ok(())
}

/// Execute a custom trigger action
///
/// Triggers are special actions that can be invoked from workflow steps.
/// Currently supported triggers:
/// - "match_master_registers": Spawn a CLI slave process to read from TUI master and verify registers
async fn execute_trigger(
    ctx: &mut ExecutionContext,
    trigger_name: &str,
    params: &Option<serde_json::Value>,
) -> Result<()> {
    match trigger_name {
        "match_master_registers" => {
            execute_match_master_registers_trigger(ctx, params).await?;
        }
        _ => {
            bail!("Unknown trigger: {trigger_name}");
        }
    }
    Ok(())
}

/// Trigger: match_master_registers
///
/// Creates a temporary CLI slave process on /tmp/vcom2 to read registers from the TUI master
/// on /tmp/vcom1, then compares the read values with expected values from trigger parameters.
///
/// Expected params format:
/// {
///   "station_id": 1,
///   "register_type": "Coils" | "DiscreteInputs" | "Holding" | "Input",
///   "start_address": 0,
///   "expected_values": [1, 0, 1, 0, ...]
/// }
async fn execute_match_master_registers_trigger(
    ctx: &mut ExecutionContext,
    params: &Option<serde_json::Value>,
) -> Result<()> {
    let params = params
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("match_master_registers requires parameters"))?;

    // Parse parameters
    let station_id = params["station_id"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("station_id parameter required"))?
        as u8;

    let register_type = params["register_type"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("register_type parameter required"))?;

    let start_address = params["start_address"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("start_address parameter required"))?
        as u16;

    let expected_values = params["expected_values"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected_values parameter required"))?;

    let register_count = expected_values.len() as u16;

    log::info!("🔍 Verifying master registers: station_id={station_id}, type={register_type}, addr=0x{start_address:04X}, count={register_count}");

    // Determine the appropriate CLI command based on register type
    let register_mode_arg = match register_type {
        "Coils" => "01",
        "DiscreteInputs" => "02",
        "Holding" => "03",
        "Input" => "04",
        _ => bail!("Unsupported register type: {register_type}"),
    };

    // Build CLI command to act as slave and read from master
    let cli_binary = if std::path::Path::new("target/debug/aoba").exists() {
        "target/debug/aoba"
    } else if std::path::Path::new("target/release/aoba").exists() {
        "target/release/aoba"
    } else {
        bail!("aoba binary not found in target/debug or target/release");
    };

    // Give TUI time to be ready for Modbus communication
    sleep_1s().await;

    // Spawn CLI process as master to read from TUI master (which will respond as if it's a slave)
    // Actually, we need to spawn as master-provide mode to read from the TUI master
    let output = tokio::process::Command::new(cli_binary)
        .args([
            "--master-provide",
            &ctx.port2, // Use port2 (/tmp/vcom2) which is connected to port1 (/tmp/vcom1)
            "--station-id",
            &station_id.to_string(),
            "--register-mode",
            register_mode_arg,
            "--register-address",
            &start_address.to_string(),
            "--register-length",
            &register_count.to_string(),
            "--once", // Read once and exit
        ])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn CLI process: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("CLI process failed: {stderr}");
        bail!("CLI slave process failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::debug!("CLI output: {stdout}");

    // Parse the CLI output to extract register values
    // Expected format varies by register type, but generally includes register values in hex or decimal
    let actual_values = parse_cli_register_output(&stdout, register_type, register_count)?;

    // Compare actual vs expected
    if actual_values.len() != expected_values.len() {
        bail!(
            "Register count mismatch: expected {expected}, got {got}",
            expected = expected_values.len(),
            got = actual_values.len()
        );
    }

    for (i, (actual, expected)) in actual_values.iter().zip(expected_values.iter()).enumerate() {
        let expected_u16 = expected
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Expected value at index {i} is not a number"))?
            as u16;

        if *actual != expected_u16 {
            bail!(
                "Register value mismatch at index {i}: expected 0x{expected_u16:04X}, got 0x{actual:04X}"
            );
        }
    }

    log::info!("✅ Master register verification passed: {register_count} registers matched");
    Ok(())
}

/// Parse CLI register output to extract register values
fn parse_cli_register_output(
    output: &str,
    register_type: &str,
    expected_count: u16,
) -> Result<Vec<u16>> {
    let mut values = Vec::new();

    // For coils/discrete inputs, look for ON/OFF or 0/1 patterns
    // For holding/input registers, look for hex values

    match register_type {
        "Coils" | "DiscreteInputs" => {
            // Look for patterns like "0x0000: ON" or "Register 0: 1"
            for line in output.lines() {
                if line.contains("ON") || line.contains("1") {
                    values.push(1);
                } else if line.contains("OFF") || line.contains("0") {
                    values.push(0);
                }

                if values.len() >= expected_count as usize {
                    break;
                }
            }
        }
        "Holding" | "Input" => {
            // Look for patterns like "0x0000: 0x1234" or similar hex values
            for line in output.lines() {
                if let Some(pos) = line.find("0x") {
                    let hex_str = &line[pos..].split_whitespace().next().unwrap_or("");
                    if let Ok(val) = u16::from_str_radix(hex_str.trim_start_matches("0x"), 16) {
                        values.push(val);
                    }
                }

                if values.len() >= expected_count as usize {
                    break;
                }
            }
        }
        _ => bail!("Unsupported register type for parsing: {register_type}"),
    }

    if values.len() < expected_count as usize {
        bail!(
            "Could not parse enough register values from CLI output. Expected {}, got {}. Output: {}",
            expected_count,
            values.len(),
            output
        );
    }

    Ok(values)
}

#[derive(Debug)]
struct VerificationFailure {
    expected: String,
    context: Option<String>,
}

impl VerificationFailure {
    fn new(expected: String, context: Option<String>) -> Self {
        Self { expected, context }
    }
}

impl fmt::Display for VerificationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.context {
            Some(context) => write!(
                f,
                "Screen verification failed for '{}': {}",
                self.expected, context
            ),
            None => write!(f, "Screen verification failed for '{}'", self.expected),
        }
    }
}

impl std::error::Error for VerificationFailure {}
