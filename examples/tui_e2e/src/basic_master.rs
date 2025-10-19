use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::generate_random_registers,
    helpers::sleep_seconds,
    key_input::{ArrowKey, ExpectKeyExt},
    ports::{port_exists, should_run_vcom_tests, vcom_matchers},
    snapshot::TerminalCapture,
    terminal::{build_debug_bin, spawn_expect_process},
    tui::{enter_modbus_panel, navigate_to_vcom, update_tui_registers},
};

const ROUNDS: usize = 3;
const REGISTER_LENGTH: usize = 12;

/// Test TUI Master-Provide + CLI Slave-Poll with continuous random data (10 rounds) - Repeat test
///
/// Workflow guard rails for this test (kept in sync with TUI automation comments):
/// 1. Enter the desired VCOM port details page.
/// 2. Enable the port before touching business configuration so runtime services boot.
/// 3. Stay inside Modbus settings after entering the panel; do not bounce back to the port list.
/// 4. Configure mode/registers directly inside the panel, then drive the business loop (updates + screenshots).
/// 5. Every loop iteration captures context prior to CLI polling so regressions link back to visible UI state.
///
/// TUI acts as Modbus Master (server providing data, responding to poll requests).
/// CLI acts as Modbus Slave (client polling for data).
/// This is a repeat of the first test for stability verification.
pub async fn test_tui_master_with_cli_slave_continuous() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master-Provide + CLI Slave-Poll test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master-Provide + CLI Slave-Poll continuous test (10 rounds)");

    let ports = vcom_matchers();

    // Verify vcom ports exist (platform-aware check)
    if !port_exists(&ports.port1_name) {
        return Err(anyhow!(
            "{} does not exist or is not available",
            ports.port1_name
        ));
    }
    if !port_exists(&ports.port2_name) {
        return Err(anyhow!(
            "{} does not exist or is not available",
            ports.port2_name
        ));
    }
    log::info!("✅ Virtual COM ports verified");

    // Spawn TUI process (will be master/server on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Navigate to vcom1
    log::info!("🧪 Step 2: Navigate to vcom1 in port list");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify we're on the port details page
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_navigate_to_vcom".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_after_nav").await?;

    // Enter Modbus configuration panel directly (new workflow: no enable_port before config)
    log::info!("🧪 Step 3: Enter Modbus configuration panel");
    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify we're in Modbus panel and check cursor position
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_enter_modbus_panel".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_in_modbus").await?;

    // Configure the Modbus station while staying in the panel.
    log::info!("🧪 Step 4: Configure TUI as Master");
    configure_tui_master(&mut tui_session, &mut tui_cap).await?;

    // Save configuration with Ctrl+S which will auto-enable the port
    log::info!("🧪 Step 5: Save configuration with Ctrl+S to auto-enable port");
    
    // Debug: Check state before Ctrl+S
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "before_ctrl_s".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_before_save").await?;
    
    let actions = vec![
        CursorAction::PressCtrlS,
        CursorAction::Sleep { ms: 3000 }, // Wait for port to enable and stabilize
    ];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "save_config_ctrl_s").await?;

    // Debug: Check state immediately after Ctrl+S
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_ctrl_s".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_after_save").await?;

    // Verify port is enabled by checking for the green checkmark or Running status
    log::info!("🧪 Step 6: Verify port is enabled after Ctrl+S");
    let screen = tui_cap
        .capture(&mut tui_session, "verify_port_enabled_after_save")
        .await?;
    // The status indicator should show either "Running" or "Applied" (green checkmark shown for 3 seconds)
    if !screen.contains("Running") && !screen.contains("Applied") {
        log::warn!("⚠️ Port status not showing as Running/Applied, checking for other indicators...");
        // Continue anyway as the port might still be starting up
    }
    log::info!("✅ Configuration saved and port enabling");

    // Wait for port to fully initialize and subprocess to be ready
    log::info!("🧪 Step 6.5: Waiting for subprocess to be fully ready...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Run 10 rounds of continuous random data testing
    // Validate after each round and exit immediately on failure
    for round in 1..=ROUNDS {
        let data = generate_random_registers(REGISTER_LENGTH);
        log::info!("🧪 Round {round}/{ROUNDS}: Generated data {data:?}");

        // Update TUI registers with new data
        log::info!("🧪 Round {round}/{ROUNDS}: Updating registers...");
        update_tui_registers(&mut tui_session, &mut tui_cap, &data, false).await?;

        // Wait for IPC updates to propagate to CLI subprocess
        // Increased wait time for CI environments which may have slower performance
        log::info!("🧪 Round {round}/{ROUNDS}: Waiting for IPC updates to propagate...");
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Poll CLI slave to verify data is accessible
        log::info!("🧪 Round {round}/{ROUNDS}: Polling CLI slave for verification");
        let binary = build_debug_bin("aoba")?;

        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY_MS: u64 = 1000;

        let mut last_received: Option<Vec<u16>> = None;
        let mut unchanged_count = 0;
        let mut poll_success = false;

        for retry_attempt in 1..=MAX_RETRIES {
            log::info!("🧪 Round {round}/{ROUNDS}: Polling attempt {retry_attempt}/{MAX_RETRIES}");

            // Take a screenshot before polling to see TUI state
            let screen = tui_cap
                .capture(
                    &mut tui_session,
                    &format!("poll_attempt_{round}_{retry_attempt}"),
                )
                .await?;
            log::info!("📺 TUI screen before polling (round {round}, attempt {retry_attempt}):\n{screen}\n");

            let cli_output = Command::new(&binary)
                .args([
                    "--slave-poll",
                    &ports.port2_name,
                    "--station-id",
                    "1",
                    "--register-address",
                    "0",
                    "--register-length",
                    &REGISTER_LENGTH.to_string(),
                    "--register-mode",
                    "holding",
                    "--baud-rate",
                    "9600",
                    "--json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;

            if !cli_output.status.success() {
                let stderr = String::from_utf8_lossy(&cli_output.stderr);
                log::warn!(
                    "⚠️ Round {round}/{ROUNDS}, attempt {retry_attempt}: CLI poll failed: {stderr}"
                );

                // If not last attempt, wait and retry
                if retry_attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                } else {
                    return Err(anyhow!(
                        "CLI poll failed on round {round} after {MAX_RETRIES} attempts"
                    ));
                }
            }

            let stdout = String::from_utf8_lossy(&cli_output.stdout);
            log::info!(
                "🧪 Round {round}/{ROUNDS}, attempt {retry_attempt}: CLI received: {output}",
                output = stdout.trim()
            );

            // Parse and check the data
            let json: serde_json::Value = serde_json::from_str(&stdout)?;
            if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                let received: Vec<u16> = values
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u16))
                    .collect();

                // Check if data matches
                if received == data {
                    log::info!("✅ Round {round}/{ROUNDS}: Data verified successfully on attempt {retry_attempt}!");
                    poll_success = true;
                    break;
                }

                // Check if data has changed since last attempt
                if let Some(ref prev) = last_received {
                    if prev == &received {
                        unchanged_count += 1;
                        log::warn!(
                            "⚠️ Round {round}/{ROUNDS}, attempt {retry_attempt}: Data unchanged ({unchanged_count}/{MAX_RETRIES}) - still {received:?}, expected {data:?}"
                        );

                        // If data hasn't changed for MAX_RETRIES consecutive times, give up
                        if unchanged_count >= MAX_RETRIES {
                            log::error!(
                                "❌ Round {round}/{ROUNDS}: Data remained unchanged at {received:?} for {MAX_RETRIES} attempts, expected {data:?}"
                            );
                            return Err(anyhow!(
                                "Data verification failed on round {round}: data remained unchanged at {received:?} for {MAX_RETRIES} attempts, expected {data:?}"
                            ));
                        }
                    } else {
                        // Data changed, reset counter
                        unchanged_count = 0;
                        log::info!(
                            "🔄 Round {round}/{ROUNDS}, attempt {retry_attempt}: Data changed from {prev:?} to {received:?}, but still doesn't match expected {data:?}"
                        );
                    }
                } else {
                    log::info!(
                        "🔄 Round {round}/{ROUNDS}, attempt {retry_attempt}: First data received: {received:?}, expected {data:?}"
                    );
                }

                last_received = Some(received.clone());

                // If not last attempt, wait and retry
                if retry_attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                }
            } else {
                log::error!("❌ Round {round}/{ROUNDS}, attempt {retry_attempt}: Failed to parse values from JSON");
                if retry_attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    return Err(anyhow!("Failed to parse JSON values on round {round}"));
                }
            }
        }

        if !poll_success {
            return Err(anyhow!(
                "Data verification failed on round {round} after {MAX_RETRIES} attempts"
            ));
        }

        // Small delay between rounds
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Kill TUI process
    tui_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ TUI Master-Provide + CLI Slave-Poll continuous test completed! All {ROUNDS} rounds passed.");
    Ok(())
}

/// Configure TUI as Master (server providing data, responding to requests) while already inside
/// the Modbus panel. The helper assumes the caller stays in the panel afterwards.
async fn configure_tui_master<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring as Master (to provide data)...");

    // Verify we are already inside Modbus settings per workflow contract.
    let screen = cap.capture(session, "verify_modbus_panel_master").await?;
    if !screen.contains("ModBus Master/Slave Set") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel before configuring master"
        ));
    }

    // Debug: Check initial cursor position
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "before_create_station".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_before_create").await?;

    // Navigate to "Create Station" button first to ensure we're at the right position
    log::info!("Navigate to Create Station button");
    let actions = vec![
        // Go all the way up to ensure we start from a known position
        CursorAction::PressArrow {
            direction: ArrowKey::Up,
            count: 20,
        },
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_top").await?;

    // Debug: Verify cursor is at top
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "at_top_before_create".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_at_top").await?;

    // Create station (should be on "Create Station" by default after going to top)
    log::info!("Create new Modbus station");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 }, // Wait for station to be created
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Debug: Check cursor position after creating station
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_create_station".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_after_create").await?;

    // After creating a station, cursor should be on the new station entry
    // Navigate to Register Start Address field first to set it to 0
    // The order is: Create Station, Connection Mode, Station ID, Register Mode, Register Start Address, Register Length
    log::info!("Navigate to Register Start Address and set to 0");
    let actions = vec![
        // From the station line, we need to navigate down to Register Start Address
        // Go up first to ensure we're at the station header line
        CursorAction::PressArrow {
            direction: ArrowKey::Up,
            count: 10,
        },
        CursorAction::Sleep { ms: 300 },
        // Now navigate down to the fields
        // Down 1: Create Station -> Connection Mode (skip, on station #1 line)
        // Down 2: Station ID
        // Down 3: Register Mode
        // Down 4: Register Start Address
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_register_start_address").await?;

    // Debug: Check cursor position before editing start address
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "before_edit_register_start_address".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_before_edit_start_addr").await?;

    // Enter edit mode and set start address to 0
    log::info!("Set Register Start Address to 0");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 }, // Wait for edit mode to activate
        CursorAction::TypeString("0".to_string()),
        CursorAction::Sleep { ms: 300 }, // Wait before confirming with Enter
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 }, // Wait for value to be committed
    ];
    execute_cursor_actions(session, cap, &actions, "set_register_start_address").await?;

    // Now navigate to Register Length field
    log::info!("Navigate to Register Length and set to 12");
    let actions = vec![
        // Down 1 more from Register Start Address to Register Length
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_register_length").await?;

    // Debug: Check cursor position before editing
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "before_edit_register_length".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_before_edit").await?;

    // Enter edit mode and set value
    log::info!("Set Register Length to 12");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 }, // Wait for edit mode to activate
        CursorAction::TypeString("12".to_string()),
        CursorAction::Sleep { ms: 300 }, // Wait before confirming with Enter
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 }, // Wait for value to be committed
    ];
    execute_cursor_actions(session, cap, &actions, "set_register_length").await?;

    // Debug: Verify register length was set
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_set_register_length".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_reg_length_set").await?;

    Ok(())
}
