// Test TUI Master (Slave/Server) with CLI Slave (Master/Client) - Continuous mode
// This test performs continuous random data updates in TUI Master and verifies CLI Slave receives them correctly
// Tests all 4 register types: holding, input, coils, discrete

use anyhow::{anyhow, Result};
use rand::random;
use regex::Regex;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::Expect;

use aoba::ci::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    {should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture},
};

/// Generate pseudo-random modbus data using rand crate
fn generate_random_data(length: usize, is_coil: bool) -> Vec<u16> {
    if is_coil {
        // For coils/discrete, generate only 0 or 1
        (0..length)
            .map(|_| if random::<u8>() % 2 == 0 { 0 } else { 1 })
            .collect()
    } else {
        // For holding/input, generate any u16 value
        (0..length).map(|_| random::<u16>()).collect()
    }
}

/// Test TUI Master with CLI Slave - Continuous mode
/// This test runs continuous random updates and verifies data integrity
pub async fn test_tui_master_continuous_with_cli_slave(register_mode: &str) -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master continuous test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave continuous test (mode: {register_mode})");

    // Verify vcom ports exist
    if !std::path::Path::new("/tmp/vcom1").exists() {
        return Err(anyhow!("/tmp/vcom1 was not created by socat"));
    }
    if !std::path::Path::new("/tmp/vcom2").exists() {
        return Err(anyhow!("/tmp/vcom2 was not created by socat"));
    }
    log::info!("✓ /tmp/vcom1 and /tmp/vcom2 verified");

    // Determine if this is a coil type register
    let is_coil = register_mode == "coils" || register_mode == "discrete";
    let register_length = if is_coil { 8 } else { 6 };

    // Spawn TUI process (will be master on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 2: Verify TUI loaded");
    let actions = vec![CursorAction::MatchPattern {
        pattern: Regex::new(r"AOBA")?,
        description: "TUI application title visible".to_string(),
        line_range: Some((0, 3)),
        col_range: None,
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_tui_loaded",
    )
    .await?;

    // Navigate to vcom1
    log::info!("🧪 Step 3: Navigate to vcom1");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Configure TUI as Master with initial values (BEFORE enabling port)
    log::info!("🧪 Step 4: Configure TUI as Master (mode: {register_mode})");
    let initial_values = generate_random_data(register_length, is_coil);
    log::info!("Initial values: {initial_values:?}");
    configure_tui_master(
        &mut tui_session,
        &mut tui_cap,
        register_mode,
        register_length,
        &initial_values,
    )
    .await?;

    // Enable the port (AFTER configuration is complete)
    log::info!("🧪 Step 5: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Wait for port initialization
    log::info!("🧪 Step 6: Wait for Modbus daemon to initialize");
    // Need to wait longer for the Modbus daemon to actually start listening
    tokio::time::sleep(Duration::from_secs(3)).await;
    log::info!("  Waited 3 seconds for daemon initialization");

    // Verify TUI master is responding before starting persistent polling
    log::info!("🧪 Step 6.5: Verify TUI master is responding");
    let binary = aoba::ci::build_debug_bin("aoba")?;
    let test_poll = Command::new(&binary)
        .args([
            "--slave-poll",
            "/tmp/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            register_mode,
            "--register-address",
            "0",
            "--register-length",
            &register_length.to_string(),
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !test_poll.status.success() {
        let stderr = String::from_utf8_lossy(&test_poll.stderr);
        return Err(anyhow!(
            "TUI master is not responding to test poll. Status: {}, stderr: {}",
            test_poll.status,
            stderr
        ));
    }

    let test_output = String::from_utf8_lossy(&test_poll.stdout);
    log::info!(
        "✅ TUI master responding, test poll output: {}",
        test_output.trim()
    );

    // Prepare output file for CLI slave
    let temp_dir = std::env::temp_dir();
    let output_file = temp_dir.join(format!("tui_master_continuous_{register_mode}.json"));
    if output_file.exists() {
        std::fs::remove_file(&output_file)?;
    }

    // Start CLI slave in persistent mode to continuously poll
    log::info!("🧪 Step 7: Start CLI slave in persistent mode");
    let mut cli_slave = Command::new(&binary)
        .args([
            "--slave-poll-persist",
            "/tmp/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            register_mode,
            "--register-address",
            "0",
            "--register-length",
            &register_length.to_string(),
            "--output",
            &format!("file:{}", output_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give CLI slave time to start
    sleep_a_while().await;

    // Check if CLI slave is still running
    match cli_slave.try_wait()? {
        Some(status) => {
            // CLI slave exited - capture stderr for debugging
            let stderr = if let Some(mut stderr_handle) = cli_slave.stderr.take() {
                let mut buf = String::new();
                use std::io::Read;
                stderr_handle.read_to_string(&mut buf).ok();
                buf
            } else {
                String::new()
            };
            return Err(anyhow!(
                "CLI slave exited prematurely with status {status}, stderr: {stderr}"
            ));
        }
        None => {
            log::info!("✅ CLI slave is running");
        }
    }

    // Perform continuous random updates (3 iterations)
    let mut all_expected_values = vec![initial_values.clone()];
    log::info!("🧪 Step 8: Perform continuous random updates");

    for iteration in 0..3 {
        log::info!("--- Iteration {} ---", iteration + 1);

        // Wait a bit for previous values to be polled
        sleep_a_while().await;

        // Generate new random values
        let new_values = generate_random_data(register_length, is_coil);
        log::info!("New values (iteration {}): {:?}", iteration + 1, new_values);
        all_expected_values.push(new_values.clone());

        // Update registers in TUI
        update_tui_registers(&mut tui_session, &mut tui_cap, &new_values, is_coil).await?;

        log::info!("✓ Updated registers in TUI");
    }

    // Wait for final values to be polled
    sleep_a_while().await;

    // Check if output file was created
    if !output_file.exists() {
        log::warn!("⚠️ Output file doesn't exist yet, waiting longer...");
        sleep_a_while().await;
    }

    // Stop CLI slave
    log::info!("🧪 Step 9: Stop CLI slave");
    cli_slave.kill()?;
    cli_slave.wait()?;

    // Verify collected data from CLI output
    log::info!("🧪 Step 10: Verify collected data from CLI output");

    // Check if file exists and has content
    if !output_file.exists() {
        return Err(anyhow!(
            "Output file does not exist: {}. CLI slave may not have successfully polled any data.",
            output_file.display()
        ));
    }

    let file_size = std::fs::metadata(&output_file)?.len();
    if file_size == 0 {
        return Err(anyhow!(
            "Output file is empty: {}. CLI slave may not have received responses from TUI master.",
            output_file.display()
        ));
    }

    log::info!("Output file exists with {file_size} bytes");
    verify_continuous_data(&output_file, &all_expected_values, is_coil)?;

    // Capture screen to verify TUI display consistency
    log::info!("🧪 Step 11: Capture screen to verify TUI display");
    let screen = tui_cap.capture(&mut tui_session, "final_screen")?;
    log::info!("📸 Final screen captured");

    // Verify screen shows register values
    let has_values = screen.contains("0x") && !screen.lines().all(|l| l.contains("0x0000"));
    if has_values {
        log::info!("✅ TUI screen shows register values (hex patterns found)");
    } else {
        log::warn!("⚠️ TUI screen may not show expected values");
    }

    // Cleanup
    log::info!("🧪 Step 12: Cleanup");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    // Clean up output file
    if output_file.exists() {
        std::fs::remove_file(&output_file)?;
    }

    log::info!(
        "✅ TUI Master + CLI Slave continuous test completed successfully (mode: {register_mode})"
    );
    Ok(())
}

/// Navigate to vcom1 port in TUI
async fn navigate_to_vcom<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    log::info!("📍 Finding vcom1 in port list...");

    let screen = cap.capture(session, "before_navigation")?;
    let vcom_pattern = std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());

    if !screen.contains(&vcom_pattern) {
        return Err(anyhow!("vcom1 ({vcom_pattern}) not found in port list"));
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(&vcom_pattern) {
            vcom1_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let vcom1_idx = vcom1_line.ok_or_else(|| anyhow!("Could not find vcom1 line index"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    if vcom1_idx != curr_idx {
        let delta = vcom1_idx.abs_diff(curr_idx);
        let direction = if vcom1_idx > curr_idx {
            aoba::ci::ArrowKey::Down
        } else {
            aoba::ci::ArrowKey::Up
        };

        let actions = vec![
            CursorAction::PressArrow {
                direction,
                count: delta,
            },
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, "nav_to_vcom1").await?;
    }

    // Press Enter to enter vcom1 details
    let vcom_pattern_regex = Regex::new(&regex::escape(&vcom_pattern))?;
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: vcom_pattern_regex,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;

    log::info!("✓ Successfully entered vcom1 details");
    Ok(())
}

/// Configure TUI as Modbus Master with initial values
async fn configure_tui_master<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    register_mode: &str,
    register_length: usize,
    initial_values: &[u16],
) -> Result<()> {
    log::info!("📝 Configuring as Master (mode: {register_mode})...");

    // Navigate to Business Configuration
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_business_config").await?;

    // Create station
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Navigate to Register Mode and set it
    log::info!("Setting register mode to: {register_mode}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 3,
        },
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_reg_mode").await?;

    // Select the appropriate register mode
    let arrow_count = match register_mode {
        "holding" => 0,
        "input" => 1,
        "coils" => 2,
        "discrete" => 3,
        _ => return Err(anyhow!("Invalid register mode: {register_mode}")),
    };

    if arrow_count > 0 {
        let actions = vec![CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Right,
            count: arrow_count,
        }];
        execute_cursor_actions(session, cap, &actions, "select_reg_mode").await?;
    }

    let actions = vec![CursorAction::PressEnter];
    execute_cursor_actions(session, cap, &actions, "confirm_reg_mode").await?;

    // Navigate to Register Length and set it
    log::info!("Setting register length to: {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    // Navigate to register values
    let actions = vec![CursorAction::PressArrow {
        direction: aoba::ci::ArrowKey::Down,
        count: 1,
    }];
    execute_cursor_actions(session, cap, &actions, "nav_to_registers").await?;

    // Set initial register values
    log::info!("Setting initial register values: {initial_values:?}");
    for (i, &val) in initial_values.iter().enumerate() {
        let dec_val = format!("{val}"); // Format as decimal, not hex
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(dec_val),
            CursorAction::PressEnter,
        ];
        execute_cursor_actions(session, cap, &actions, &format!("set_reg_{i}")).await?;

        if i < initial_values.len() - 1 {
            let actions = vec![CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Right,
                count: 1,
            }];
            execute_cursor_actions(session, cap, &actions, &format!("nav_to_reg_{}", i + 1))
                .await?;
        }
    }

    // Exit Modbus settings - stay where we are, don't navigate back
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_settings").await?;

    log::info!("✓ Master configuration complete");
    Ok(())
}

/// Enable the serial port in TUI
async fn enable_port_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("🔌 Enabling port...");

    let screen = cap.capture(session, "before_enable")?;

    if !screen.contains("Enable Port") {
        return Err(anyhow!(
            "Not in port details page - 'Enable Port' not found"
        ));
    }

    // Check if cursor is on "Enable Port" line
    let lines: Vec<&str> = screen.lines().collect();
    let mut on_enable_port = false;
    for line in lines {
        let trimmed = line.trim();
        if (trimmed.starts_with("│ > ") || trimmed.starts_with("> "))
            && line.contains("Enable Port")
        {
            on_enable_port = true;
            break;
        }
    }

    if !on_enable_port {
        let actions = vec![CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Up,
            count: 3,
        }];
        execute_cursor_actions(session, cap, &actions, "nav_to_enable_port").await?;
    }

    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1500 }];
    execute_cursor_actions(session, cap, &actions, "toggle_enable_port").await?;

    log::info!("✓ Port enabled");
    Ok(())
}

/// Update TUI registers with new values
async fn update_tui_registers<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    new_values: &[u16],
    _is_coil: bool,
) -> Result<()> {
    // Navigate to Business Configuration (2 down from Enable Port)
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_modbus_for_update").await?;

    // Navigate to first register (station should be selected, go down to registers)
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 6,
        },
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_first_register").await?;

    // Update each register value
    for (i, &val) in new_values.iter().enumerate() {
        let dec_val = format!("{val}"); // Format as decimal, not hex
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(dec_val),
            CursorAction::PressEnter,
        ];
        execute_cursor_actions(session, cap, &actions, &format!("update_reg_{i}")).await?;

        if i < new_values.len() - 1 {
            let actions = vec![CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Right,
                count: 1,
            }];
            execute_cursor_actions(session, cap, &actions, &format!("nav_to_reg_{}", i + 1))
                .await?;
        }
    }

    // Exit - stay where we are, don't navigate back
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "exit_after_update").await?;

    Ok(())
}

/// Verify continuous data collected by CLI slave
fn verify_continuous_data(
    output_file: &std::path::Path,
    expected_values_list: &[Vec<u16>],
    _is_coil: bool,
) -> Result<()> {
    log::info!(
        "🔍 Verifying collected data from: {path}",
        path = output_file.display()
    );

    if !output_file.exists() {
        return Err(anyhow!("Output file does not exist"));
    }

    let content = std::fs::read_to_string(output_file)?;
    log::info!(
        "Output file content ({len} bytes):\n{content}",
        len = content.len(),
        content = content
    );

    if content.trim().is_empty() {
        return Err(anyhow!("Output file is empty"));
    }

    // Parse JSON lines
    let mut parsed_outputs = Vec::new();
    for (i, line) in content.lines().enumerate() {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(json) => {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let values_u16: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();
                    parsed_outputs.push(values_u16);
                }
            }
            Err(e) => {
                log::warn!(
                    "⚠️ Line {line} is not valid JSON: {err}",
                    line = i + 1,
                    err = e
                );
            }
        }
    }

    log::info!("Parsed {} output lines", parsed_outputs.len());
    log::info!("Expected {} value sets", expected_values_list.len());

    // Verify that at least some of the expected values were captured
    let mut found_count = 0;
    for (i, expected_values) in expected_values_list.iter().enumerate() {
        let found = parsed_outputs
            .iter()
            .any(|output| output == expected_values);
        if found {
            log::info!(
                "✅ Expected value set {idx} found: {vals:?}",
                idx = i + 1,
                vals = expected_values
            );
            found_count += 1;
        } else {
            log::warn!(
                "⚠️ Expected value set {idx} NOT found: {vals:?}",
                idx = i + 1,
                vals = expected_values
            );
        }
    }

    if found_count == 0 {
        return Err(anyhow!(
            "None of the expected value sets were found in output"
        ));
    }

    log::info!(
        "✅ Found {found}/{total} expected value sets in output",
        found = found_count,
        total = expected_values_list.len()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🧪 Running TUI E2E Continuous Tests: TUI Master + CLI Slave");

    // Test all 4 register modes
    let register_modes = ["holding", "input", "coils", "discrete"];

    for mode in &register_modes {
        log::info!("\n========== Testing register mode: {mode} ==========\n");
        match test_tui_master_continuous_with_cli_slave(mode).await {
            Ok(_) => {
                log::info!("✅ Test passed for mode: {mode}");
            }
            Err(e) => {
                log::error!("❌ Test failed for mode {mode}: {e}");
                return Err(e);
            }
        }

        // Wait between tests to ensure ports are released
        sleep_a_while().await;
    }

    log::info!("\n✅ All TUI Master continuous tests passed!");
    Ok(())
}
