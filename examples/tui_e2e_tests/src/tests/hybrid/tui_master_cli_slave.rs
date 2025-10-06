// Test TUI Master (Slave/Server) with CLI Slave (Master/Client)
// Rewritten with step-by-step verification and regex probes after each action

use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture};

/// Test TUI Master with CLI Slave
/// TUI acts as Modbus Master (Slave/Server) responding to requests
/// CLI acts as Modbus Slave (Master/Client) polling for data
///
/// This test is rewritten with careful step-by-step verification
pub async fn test_tui_master_with_cli_slave() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master + CLI Slave test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave hybrid test");

    // Spawn TUI process (will be master on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Step 2: Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 2: Verify TUI loaded with port list");
    let actions = vec![
        CursorAction::Sleep { ms: 2000 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"AOBA")?,
            description: "TUI application title visible".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_tui_loaded",
    )
    .await?;

    let screen = tui_cap.capture(&mut tui_session, "initial_screen")?;
    log::info!("📸 Initial screen:\n{}", screen);

    // Step 3: Navigate to vcom1
    log::info!("🧪 Step 3: Navigate to vcom1 in port list");
    navigate_to_vcom1_carefully(&mut tui_session, &mut tui_cap).await?;

    // Step 4: Configure as Master mode with test data
    log::info!("🧪 Step 4: Configure TUI as Master with test values");
    configure_tui_master_carefully(&mut tui_session, &mut tui_cap).await?;

    // Step 5: Enable the port
    log::info!("🧪 Step 5: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Give TUI time to fully initialize the port
    log::info!("🧪 Step 6: Wait for port to initialize");
    sleep_a_while().await;
    thread::sleep(Duration::from_secs(3));

    // Step 7: Use CLI to poll the TUI master
    log::info!("🧪 Step 7: Run CLI slave poll command");
    let cli_result = run_cli_slave_poll().await?;

    // Step 8: Verify the CLI got the expected values
    log::info!("🧪 Step 8: Verify CLI output");
    verify_cli_output(&cli_result)?;

    // Cleanup: quit TUI
    log::info!("🧪 Step 9: Cleanup - quit TUI");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    sleep_a_while().await;

    log::info!("✅ TUI Master + CLI Slave test completed successfully");
    Ok(())
}

/// Navigate to vcom1 port in TUI with careful verification
async fn navigate_to_vcom1_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("  📍 Finding vcom1 in port list...");

    // Capture screen to see current state
    let screen = cap.capture(session, "before_navigation")?;
    log::info!("📸 Screen before navigation:\n{}", screen);

    // Verify vcom1 is visible
    if !screen.contains("/dev/vcom1") {
        return Err(anyhow!("vcom1 not found in port list"));
    }

    // Find vcom1 line and current cursor position
    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("/dev/vcom1") {
            vcom1_line = Some(idx);
            log::info!("  Found vcom1 at line {}", idx);
        }
        // Look for cursor indicator - could be ">" at start or in the line
        let trimmed = line.trim_start();
        if trimmed.starts_with("> ") || trimmed.starts_with(">") {
            cursor_line = Some(idx);
            log::info!("  Current cursor at line {}", idx);
        }
    }

    let vcom1_idx = vcom1_line.ok_or_else(|| anyhow!("Could not find vcom1 line index"))?;
    let curr_idx = cursor_line.unwrap_or(3); // Default to line 3 if not found

    // Navigate to vcom1
    if vcom1_idx > curr_idx {
        let steps = vcom1_idx - curr_idx;
        log::info!("  Moving DOWN {} steps to reach vcom1", steps);
        let actions = vec![
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Down,
                count: steps,
            },
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, "nav_down_to_vcom1").await?;
    } else if vcom1_idx < curr_idx {
        let steps = curr_idx - vcom1_idx;
        log::info!("  Moving UP {} steps to reach vcom1", steps);
        let actions = vec![
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Up,
                count: steps,
            },
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, "nav_up_to_vcom1").await?;
    } else {
        log::info!("  Already on vcom1 line");
    }

    // Verify cursor is now on vcom1
    let screen_after = cap.capture(session, "after_navigation")?;
    log::info!("📸 Screen after navigation:\n{}", screen_after);

    let on_vcom1 = screen_after.lines().any(|line| {
        let trimmed = line.trim_start();
        (trimmed.starts_with("> ") || trimmed.starts_with(">")) && line.contains("/dev/vcom1")
    });

    if !on_vcom1 {
        return Err(anyhow!(
            "Failed to navigate to vcom1 - cursor not on vcom1 line"
        ));
    }

    log::info!("  ✓ Cursor is now on vcom1");

    // Press Enter to enter vcom1 details
    log::info!("  Pressing Enter to open vcom1...");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1500 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"/dev/vcom1")?,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;

    let screen_details = cap.capture(session, "vcom1_details")?;
    log::info!("📸 Inside vcom1 details:\n{}", screen_details);

    log::info!("  ✓ Successfully entered vcom1 details");
    Ok(())
}

/// Configure TUI as Modbus Master with test values - carefully
async fn configure_tui_master_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("  📝 Configuring as Master...");

    // Navigate to Modbus settings (should be 2 down from current position)
    log::info!("  Navigate to Modbus Settings");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_modbus").await?;

    let screen = cap.capture(session, "on_modbus_option")?;
    log::info!("📸 On Modbus option:\n{}", screen);

    // Enter Modbus settings
    log::info!("  Enter Modbus Settings");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1500 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_modbus_settings").await?;

    let screen = cap.capture(session, "in_modbus_settings")?;
    log::info!("📸 In Modbus settings:\n{}", screen);

    // Create station (should be on "Create Station" by default)
    log::info!("  Create new station");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1500 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    let screen = cap.capture(session, "station_created")?;
    log::info!("📸 Station created:\n{}", screen);

    // Navigate to Register Length field (5 down from current)
    log::info!("  Navigate to Register Length");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 5,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_reg_length").await?;

    // Set Register Length to 4
    log::info!("  Set Register Length to 4");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::TypeString("4".to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Register Length\s+0x0004")?,
            description: "Register Length set to 4".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    let screen = cap.capture(session, "reg_length_set")?;
    log::info!("📸 Register Length set:\n{}", screen);

    // Navigate to register values (1 down)
    log::info!("  Navigate to register values");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_registers").await?;

    // Set register values: 0, 10, 20, 30 (in hex: 0, A, 14, 1E)
    let test_values = vec![0u16, 10, 20, 30];
    for (i, &val) in test_values.iter().enumerate() {
        let hex_val = format!("{:X}", val);
        log::info!("  Set register {} to {} (0x{:04X})", i, val, val);

        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(hex_val.clone()),
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("set_reg_{}", i)).await?;

        // Move to next register
        if i < test_values.len() - 1 {
            let actions = vec![
                CursorAction::PressArrow {
                    direction: aoba::ci::ArrowKey::Right,
                    count: 1,
                },
                CursorAction::Sleep { ms: 300 },
            ];
            execute_cursor_actions(session, cap, &actions, &format!("nav_to_reg_{}", i + 1))
                .await?;
        }
    }

    let screen = cap.capture(session, "registers_set")?;
    log::info!("📸 Registers set:\n{}", screen);

    // Verify values are correct
    if !screen.contains("0x0000")
        || !screen.contains("0x000A")
        || !screen.contains("0x0014")
        || !screen.contains("0x001E")
    {
        log::warn!("⚠️  Register values may not be set correctly");
    } else {
        log::info!("  ✓ Register values verified");
    }

    // Exit register editing mode
    log::info!("  Exit register editing");
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 500 }];
    execute_cursor_actions(session, cap, &actions, "exit_register_edit").await?;

    // Navigate back up to "Enable Port" (should be about 2 up)
    log::info!("  Navigate back to Enable Port");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Up,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_enable").await?;

    log::info!("  ✓ Master configuration complete");
    Ok(())
}

/// Enable the serial port in TUI - carefully
async fn enable_port_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("  🔌 Enabling port...");

    let screen = cap.capture(session, "before_enable")?;
    log::info!("📸 Before enabling:\n{}", screen);

    // Should be on "Enable Port" option
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 2000 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Enabled")?,
            description: "Port enabled".to_string(),
            line_range: Some((2, 5)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enable_port").await?;

    let screen = cap.capture(session, "after_enable")?;
    log::info!("📸 After enabling:\n{}", screen);

    log::info!("  ✓ Port enabled successfully");
    Ok(())
}

/// Run CLI slave poll command
async fn run_cli_slave_poll() -> Result<String> {
    let binary = aoba::ci::build_debug_bin("aoba")?;

    log::info!("  🖥️  Executing CLI command: modbus slave poll");

    let output = Command::new(&binary)
        .args([
            "modbus",
            "slave",
            "poll",
            "--port",
            "/dev/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            "holding",
            "--register-address",
            "0",
            "--register-length",
            "4",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::info!("  CLI exit status: {}", output.status);
    log::info!("  CLI stdout: {}", stdout);
    if !stderr.is_empty() {
        log::info!("  CLI stderr: {}", stderr);
    }

    if !output.status.success() {
        return Err(anyhow!(
            "CLI command failed with status {}: {}",
            output.status,
            stderr
        ));
    }

    Ok(stdout)
}

/// Verify CLI output contains expected register values
fn verify_cli_output(output: &str) -> Result<()> {
    log::info!("  🔍 Verifying CLI output contains expected values");

    // Expected values in decimal: 0, 10, 20, 30
    let expected_values = vec![0u16, 10, 20, 30];

    let mut all_found = true;
    for &val in &expected_values {
        // Check for various formats
        let patterns = vec![
            format!("0x{:04X}", val), // 0x000A
            format!("0x{:04x}", val), // 0x000a
            format!("{}", val),       // 10
        ];

        let mut found = false;
        for pattern in &patterns {
            if output.contains(pattern) {
                found = true;
                log::info!("    ✓ Found value {} (pattern: {})", val, pattern);
                break;
            }
        }

        if !found {
            all_found = false;
            log::error!("    ✗ Value {} not found in CLI output", val);
        }
    }

    if !all_found {
        return Err(anyhow!(
            "CLI output does not contain all expected register values"
        ));
    }

    log::info!("  ✅ All expected values verified in CLI output");
    Ok(())
}
