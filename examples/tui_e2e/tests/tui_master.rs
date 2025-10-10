use anyhow::{anyhow, Result};
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::generate_random_registers,
    helpers::sleep_a_while,
    key_input::{ArrowKey, ExpectKeyExt},
    ports::{should_run_vcom_tests, vcom_matchers},
    snapshot::TerminalCapture,
    terminal::{build_debug_bin, spawn_expect_process},
    tui::{enable_port_carefully, enter_modbus_panel, navigate_to_vcom, update_tui_registers},
};

const ROUNDS: usize = 10;
const REGISTER_LENGTH: usize = 5;

/// Test TUI Master-Provide + CLI Slave-Poll with continuous random data (10 rounds) - Repeat test
/// TUI acts as Modbus Master (server providing data, responding to poll requests)
/// CLI acts as Modbus Slave (client polling for data)
/// This is a repeat of the first test for stability verification
pub async fn test_tui_master_with_cli_slave_continuous() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master-Provide + CLI Slave-Poll test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master-Provide + CLI Slave-Poll continuous test (10 rounds)");

    let ports = vcom_matchers();

    // Verify vcom ports exist
    if !std::path::Path::new(&ports.port1_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port1_name));
    }
    if !std::path::Path::new(&ports.port2_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port2_name));
    }
    log::info!("✅ Virtual COM ports verified");

    // Spawn TUI process (will be master/server on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Navigate to vcom1 and configure as master
    log::info!("🧪 Step 2: Navigate to vcom1 in port list");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify we're on vcom1 port details page
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_navigate_to_vcom1".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_nav_vcom1").await?;

    // Configure as master (server mode providing data)
    log::info!("🧪 Step 3: Configure TUI as Master");
    configure_tui_master(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify we're back on port details page after configuration
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_configure_master".to_string(),
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "debug_after_config",
    )
    .await?;

    // Enable the port
    log::info!("🧪 Step 4: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify port is enabled
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_enable_port".to_string(),
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "debug_enable_port",
    )
    .await?;

    // CRUCIAL: Enter Modbus panel to access registers for updates
    log::info!("🧪 Step 5: Enter Modbus configuration panel");
    enter_modbus_panel(&mut tui_session, &mut tui_cap).await?;

    // Debug: Verify we're in the Modbus panel
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_enter_modbus_panel".to_string(),
    }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "debug_in_panel").await?;

    // Run 10 rounds of continuous random data testing
    // Validate after each round and exit immediately on failure
    for round in 1..=ROUNDS {
        let data = generate_random_registers(REGISTER_LENGTH);
        log::info!("🧪 Round {round}/{ROUNDS}: Generated data {data:?}");

        // Debug: Verify we're ready to update registers
        let actions = vec![CursorAction::DebugBreakpoint {
            description: format!("before_update_registers_round_{round}"),
        }];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            "debug_before_update",
        )
        .await?;

        // Update TUI registers with new data
        update_tui_registers(&mut tui_session, &mut tui_cap, &data, false).await?;

        // Debug: Verify registers were updated
        let actions = vec![CursorAction::DebugBreakpoint {
            description: format!("after_update_registers_round_{round}"),
        }];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            "debug_after_update",
        )
        .await?;

        // Poll CLI slave with retry logic - wait for data to propagate
        log::info!("🧪 Round {round}/{ROUNDS}: Polling CLI slave with retry logic");
        let binary = build_debug_bin("aoba")?;
        
        const MAX_RETRIES: usize = 5;
        const RETRY_DELAY_MS: u64 = 1000;
        
        let mut last_received: Option<Vec<u16>> = None;
        let mut unchanged_count = 0;
        let mut poll_success = false;
        
        for retry_attempt in 1..=MAX_RETRIES {
            log::info!("🧪 Round {round}/{ROUNDS}: Polling attempt {retry_attempt}/{MAX_RETRIES}");
            
            // Take a screenshot before polling to see TUI state
            let screen = tui_cap
                .capture(&mut tui_session, &format!("poll_attempt_{round}_{retry_attempt}"))
                .await?;
            log::info!("📺 TUI screen before polling (round {round}, attempt {retry_attempt}):\n{}\n", screen);
            
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
                log::warn!("⚠️ Round {round}/{ROUNDS}, attempt {retry_attempt}: CLI poll failed: {stderr}");
                
                // If not last attempt, wait and retry
                if retry_attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                } else {
                    return Err(anyhow!("CLI poll failed on round {round} after {MAX_RETRIES} attempts"));
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

/// Configure TUI as Master (server providing data, responding to requests)
async fn configure_tui_master<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring as Master (to provide data)...");

    // Navigate to Modbus settings (should be 2 down from current position)
    log::info!("Navigate to Modbus Settings");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_modbus").await?;

    // Enter Modbus settings
    log::info!("Enter Modbus Settings");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_modbus_settings").await?;

    // Create station (should be on "Create Station" by default)
    log::info!("Create new Modbus station");
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

    // Debug: Verify station was created
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_create_station".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_station_created").await?;

    // Set Register Length to 5 (matching REGISTER_LENGTH constant)
    log::info!("Navigate to Register Length and set to 5");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5,
        }, // Navigate to Register Length field
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::TypeString("5".to_string()),
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_register_length").await?;

    // Debug: Verify register length was set
    let actions = vec![CursorAction::DebugBreakpoint {
        description: "after_set_register_length".to_string(),
    }];
    execute_cursor_actions(session, cap, &actions, "debug_reg_length_set").await?;

    // Press Escape to exit Modbus settings
    session.send_escape()?;
    sleep_a_while().await;

    Ok(())
}
