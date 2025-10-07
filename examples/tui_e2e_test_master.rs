// Test TUI Master (Slave/Server) with CLI Slave (Master/Client)
// Rewritten with step-by-step verification and regex probes after each action

use anyhow::{anyhow, Result};
use regex::Regex;
use std::process::{Command, Stdio};

use expectrl::Expect;

use aoba::ci::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    {should_run_vcom_tests, sleep_a_while, spawn_expect_process, vcom_matchers, TerminalCapture},
};

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

    let vmatch = vcom_matchers();

    // Verify vcom ports exist (skip on Windows where ports may not be files)
    #[cfg(unix)]
    {
        if !std::path::Path::new(&vmatch.port1_name).exists() {
            return Err(anyhow!("{} was not created by socat", vmatch.port1_name));
        }
        if !std::path::Path::new(&vmatch.port2_name).exists() {
            return Err(anyhow!("{} was not created by socat", vmatch.port2_name));
        }
        log::info!(
            "✓ {} and {} created successfully",
            vmatch.port1_name,
            vmatch.port2_name
        );
    }
    #[cfg(not(unix))]
    {
        log::info!(
            "✓ Using virtual COM ports: {} and {}",
            vmatch.port1_name,
            vmatch.port2_name
        );
    }

    // Spawn TUI process (will be master on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 2: Verify TUI loaded with port list");
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

    let screen = tui_cap.capture(&mut tui_session, "initial_screen")?;
    log::info!("📸 Initial screen:\n{screen}");

    // Navigate to vcom1
    log::info!("🧪 Step 3: Navigate to vcom1 in port list");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Configure Modbus settings FIRST (before enabling)
    log::info!("🧪 Step 4: Configure TUI as Master with test values");
    configure_tui_master_carefully(&mut tui_session, &mut tui_cap).await?;

    // Enable the port (after configuration)
    log::info!("🧪 Step 5: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Give TUI time to fully initialize the port and start Modbus daemon
    log::info!("🧪 Step 6: Wait for port and Modbus daemon to initialize");
    log::info!("Waiting for Modbus daemon to start listening...");
    sleep_a_while().await;

    // Use CLI to poll the TUI master
    log::info!("🧪 Step 7: Run CLI slave poll command");
    let cli_result = run_cli_slave_poll().await?;

    // Verify the CLI got the expected values
    log::info!("🧪 Step 8: Verify CLI output");
    verify_cli_output(&cli_result)?;

    // Cleanup, quit TUI
    log::info!("🧪 Step 9: Cleanup - quit TUI");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    log::info!("✅ TUI Master + CLI Slave test completed successfully");
    Ok(())
}

/// Navigate to vcom1 port in TUI with careful verification
async fn navigate_to_vcom<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    log::info!("📍 Finding vcom1 in port list...");

    let vmatch = vcom_matchers();

    // Capture screen to see current state
    let screen = cap.capture(session, "before_navigation")?;
    log::info!("📸 Screen before navigation:\n{screen}");

    // Verify vcom1 is visible
    if !screen.contains(&vmatch.port1_name) {
        return Err(anyhow!(
            "vcom1 ({}) not found in port list",
            vmatch.port1_name
        ));
    }

    // Find vcom1 line and current cursor position
    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(&vmatch.port1_name) {
            vcom1_line = Some(idx);
            log::info!(
                "Found vcom1 ({}) at line {idx}: {trim}",
                vmatch.port1_name,
                trim = line.trim()
            );
        }
        // Look for cursor indicator - look for "> " or "│ > " pattern
        if line.contains("> ") {
            // Make sure it's a cursor marker, not just any > character
            // The cursor is typically "│ > portname" or "> portname"
            let trimmed = line.trim();
            if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
                log::info!("Current cursor at line {idx}: {trim}", trim = line.trim());
            }
        }
    }

    let vcom1_idx = vcom1_line.ok_or_else(|| anyhow!("Could not find vcom1 line index"))?;
    let curr_idx = cursor_line.unwrap_or(3); // Default to line 3 if not found

    log::info!("vcom1 is at line index: {vcom1_idx}");
    log::info!("cursor is at line index: {curr_idx}");

    // Navigate to vcom1
    if vcom1_idx > curr_idx {
        let steps = vcom1_idx - curr_idx;
        log::info!("Moving DOWN {steps} steps to reach vcom1");
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
        log::info!("Moving UP {steps} steps to reach vcom1");
        let actions = vec![
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Up,
                count: steps,
            },
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, "nav_up_to_vcom1").await?;
    } else {
        log::info!("Already on vcom1 line, no navigation needed");
    }

    // Verify cursor is now on vcom1
    let screen_after = cap.capture(session, "after_navigation")?;
    log::info!("📸 Screen after navigation:\n{screen_after}");

    let on_vcom1 = screen_after.lines().any(|line| {
        let trimmed = line.trim();
        (trimmed.starts_with("│ > ") || trimmed.starts_with("> "))
            && line.contains(&vmatch.port1_name)
    });

    if !on_vcom1 {
        return Err(anyhow!(
            "Failed to navigate to vcom1 - cursor not on vcom1 line ({})",
            vmatch.port1_name
        ));
    }

    log::info!("✓ Cursor is now on vcom1 ({})", vmatch.port1_name);

    // Press Enter to enter vcom1 details
    log::info!("Pressing Enter to open vcom1...");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: vmatch.port1_rx.clone(),
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;

    let screen_details = cap.capture(session, "vcom1_details")?;
    log::info!("📸 Inside vcom1 details:\n{screen_details}");

    log::info!("✓ Successfully entered vcom1 details");
    Ok(())
}

/// Configure TUI as Modbus Master with test values - carefully
async fn configure_tui_master_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("📝 Configuring as Master...");

    // We should be in vcom1 details page after enabling the port
    // Navigate to "Enter Business Configuration" (should be 2 down from Enable Port)
    log::info!("Navigate to 'Enter Business Configuration'");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_business_config").await?;

    let screen = cap.capture(session, "on_business_config_option")?;
    log::info!("📸 On Business Configuration option:\n{screen}");

    // Enter Business Configuration (Modbus settings)
    log::info!("Enter Business Configuration (Modbus settings)");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_business_config").await?;

    let screen = cap.capture(session, "in_modbus_settings")?;
    log::info!("📸 In Modbus settings:\n{screen}");

    // Create station (should be on "Create Station" by default)
    log::info!("Create new station");
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

    let screen = cap.capture(session, "station_created")?;
    log::info!("📸 Station created:\n{screen}");

    // Navigate to Register Length field (5 down from current)
    log::info!("Navigate to Register Length");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 5,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_reg_length").await?;

    // Set Register Length to 12 (0x000C) as required by test spec
    log::info!("Set Register Length to 12");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()), // Enter 12 in decimal
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Register Length\s+0x000C")?,
            description: "Register Length set to 12".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    let screen = cap.capture(session, "reg_length_set")?;
    log::info!("📸 Register Length set:\n{screen}");

    // Navigate to register values (1 down)
    log::info!("Navigate to register values");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_registers").await?;

    // Set register values: 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110 (in decimal)
    // UI accepts HEX input and displays as hex, so we need to convert decimal to hex strings
    // Layout is 4 columns per row: [0,1,2,3] [4,5,6,7] [8,9,10,11]
    let test_values = [0u16, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110];
    for (i, &val) in test_values.iter().enumerate() {
        let hex_val = format!("{val:X}"); // Convert to HEX string
        log::info!("Set register {i} to {val} (0x{val:04X})");

        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(hex_val.clone()), // Type hex value
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, &format!("set_reg_{i}")).await?;

        // Navigate to next register - just use RIGHT arrow for all registers
        if i < test_values.len() - 1 {
            log::info!("Moving Right to next register");
            let actions = vec![
                CursorAction::PressArrow {
                    direction: aoba::ci::ArrowKey::Right,
                    count: 1,
                },
                CursorAction::Sleep { ms: 250 },
            ];
            execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("nav_to_reg_{num}", num = i + 1),
            )
            .await?;
        }
    }

    let screen = cap.capture(session, "registers_set")?;
    log::info!("📸 All 12 registers set:\n{screen}");

    // Verify at least some key values are visible (pattern check)
    let has_values = screen.contains("0x000A")
        || screen.contains("0x0014")
        || screen.contains("0x001E")
        || screen.contains("0x0028")
        || screen.contains("0x0032")
        || screen.contains("0x003C");
    if !has_values {
        log::warn!("⚠️  Register values may not be set correctly");
    } else {
        log::info!("✓ Register values verified (pattern visible)");
    }

    // Exit Modbus settings back to port details
    // We're in navigation mode (not editing), so one Escape goes back to port details
    log::info!("Exit Modbus settings back to port details (press Escape)");
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_settings").await?;

    let screen = cap.capture(session, "after_exit")?;
    log::info!("📸 After exiting Modbus settings:\n{screen}");

    // Check if we're back on port details or main port list
    if screen.contains("Enable Port") {
        log::info!("✓ Already back on port details page");
    } else if screen.contains("COM Ports") {
        log::info!("We went back to main port list, need to enter vcom1 again");

        let vmatch = vcom_matchers();

        // Find vcom1 and navigate to it
        let lines: Vec<&str> = screen.lines().collect();
        let mut vcom1_line = None;
        let mut cursor_line = None;

        for (idx, line) in lines.iter().enumerate() {
            if line.contains(&vmatch.port1_name) {
                vcom1_line = Some(idx);
            }
            if line.contains("> ") {
                let trimmed = line.trim();
                if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                    cursor_line = Some(idx);
                }
            }
        }

        if let (Some(vcom1_idx), Some(curr_idx)) = (vcom1_line, cursor_line) {
            if vcom1_idx != curr_idx {
                let delta = vcom1_idx.abs_diff(curr_idx);

                let direction = if vcom1_idx > curr_idx {
                    aoba::ci::ArrowKey::Down
                } else {
                    aoba::ci::ArrowKey::Up
                };

                log::info!("Navigating to vcom1 ({delta} steps)");
                let actions = vec![
                    CursorAction::PressArrow {
                        direction,
                        count: delta,
                    },
                    CursorAction::Sleep { ms: 500 },
                ];
                execute_cursor_actions(session, cap, &actions, "nav_back_to_vcom1").await?;
            }
        }

        // Press Enter to enter vcom1 details
        log::info!("Press Enter to enter vcom1 details");
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1000 }];
        execute_cursor_actions(session, cap, &actions, "reenter_vcom1").await?;

        let screen = cap.capture(session, "back_in_vcom1_details")?;
        log::info!("📸 Back in vcom1 details:\n{screen}");

        if !screen.contains("Enable Port") {
            return Err(anyhow!("Failed to re-enter vcom1 details page"));
        }
    } else {
        return Err(anyhow!("Unexpected screen after exiting Modbus settings"));
    }

    log::info!("✓ Master configuration complete, ready to enable port");
    Ok(())
}

/// Enable the serial port in TUI - carefully
/// This should be called AFTER configuring Modbus settings
async fn enable_port_carefully<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    log::info!("🔌 Enabling port...");

    let screen = cap.capture(session, "before_enable")?;
    log::info!("📸 Before enabling:\n{screen}");

    // We should be back in vcom1 details page after exiting Modbus settings
    // Verify we see "Enable Port" option
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
            log::info!("✓ Cursor already on 'Enable Port' line");
            break;
        }
    }

    // If not on Enable Port, navigate to it
    if !on_enable_port {
        log::info!("Navigate UP to 'Enable Port' option");
        let actions = vec![
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Up,
                count: 3, // Go all the way up
            },
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(session, cap, &actions, "nav_to_enable_port").await?;

        let screen = cap.capture(session, "on_enable_port")?;
        log::info!("📸 On Enable Port option:\n{screen}");
    }

    // Press Enter to toggle Enable Port to Enabled
    log::info!("Press Enter to toggle Enable Port to Enabled");
    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1500 }];
    execute_cursor_actions(session, cap, &actions, "toggle_enable_port").await?;

    let screen = cap.capture(session, "after_toggle")?;
    log::info!("📸 After toggling:\n{screen}");

    // Check that we're still on port details page
    if !screen.contains("Protocol Mode") {
        return Err(anyhow!(
            "Unexpected screen after toggling - not on port details"
        ));
    }

    // Check if it shows "Enabled"
    if screen.contains("Enabled") {
        log::info!("✓ Port shows as 'Enabled'");
    } else {
        log::warn!("⚠️  'Enabled' text not found - port may not be enabled");
    }

    Ok(())
}

/// Run CLI slave poll command
async fn run_cli_slave_poll() -> Result<String> {
    let vmatch = vcom_matchers();
    let binary = aoba::ci::build_debug_bin("aoba")?;

    log::info!("🖥️  Executing CLI command: slave poll (request data from master)");

    let output = Command::new(&binary)
        .args([
            "--slave-poll",
            &vmatch.port2_name,
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            "holding",
            "--register-address",
            "0",
            "--register-length",
            "12", // Updated to match the 12 registers configured in TUI
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::info!("CLI exit status: {status}", status = output.status);
    log::info!("CLI stdout: {stdout}");
    if !stderr.is_empty() {
        log::info!("CLI stderr: {stderr}");
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
    log::info!("🔍 Verifying CLI output contains expected values");

    // Expected values in decimal: 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110
    let expected_values = vec![0u16, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110];

    let mut all_found = true;
    for &val in &expected_values {
        // Check for various formats
        let patterns = vec![
            format!("0x{:04X}", val), // 0x000A
            format!("0x{:04x}", val), // 0x000a
            format!("{val}"),         // 10
        ];

        let mut found = false;
        for pattern in &patterns {
            if output.contains(pattern) {
                found = true;
                log::info!("✓ Found value {val} (pattern: {pattern})");
                break;
            }
        }

        if !found {
            all_found = false;
            log::error!("✗ Value {val} not found in CLI output");
        }
    }

    if !all_found {
        return Err(anyhow!(
            "CLI output does not contain all expected register values"
        ));
    }

    log::info!("✅ All expected values verified in CLI output");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    log::info!("🧪 Running TUI E2E Test 1: TUI Master + CLI Slave");

    match test_tui_master_with_cli_slave().await {
        Ok(_) => {
            log::info!("✅ Test 1 passed");
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Test 1 failed: {e}");
            Err(e)
        }
    }
}
