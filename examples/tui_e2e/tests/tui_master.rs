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
    tui::{enable_port_carefully, navigate_to_vcom, update_tui_registers},
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

    // Configure as master (server mode providing data)
    log::info!("🧪 Step 3: Configure TUI as Master");
    configure_tui_master(&mut tui_session, &mut tui_cap).await?;

    // Enable the port
    log::info!("🧪 Step 4: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Run 10 rounds of continuous random data testing
    // Validate after each round and exit immediately on failure
    for round in 1..=ROUNDS {
        let data = generate_random_registers(REGISTER_LENGTH);
        log::info!("🧪 Round {round}/{ROUNDS}: Generated data {data:?}");

        // Update TUI registers with new data
        update_tui_registers(&mut tui_session, &mut tui_cap, &data, false).await?;

        // Start CLI slave to poll data
        log::info!("🧪 Round {round}/{ROUNDS}: Starting CLI slave to poll");
        let binary = build_debug_bin("aoba")?;

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
            log::error!("❌ Round {round}/{ROUNDS}: CLI slave failed: {stderr}");
            return Err(anyhow!("CLI slave failed on round {round}"));
        }

        let stdout = String::from_utf8_lossy(&cli_output.stdout);
        log::info!(
            "🧪 Round {round}/{ROUNDS}: CLI received: {output}",
            output = stdout.trim()
        );

        // Verify the data matches immediately
        let json: serde_json::Value = serde_json::from_str(&stdout)?;
        if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
            let received: Vec<u16> = values
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u16))
                .collect();

            if received == data {
                log::info!("✅ Round {round}/{ROUNDS}: Data verified successfully!");
            } else {
                log::error!(
                    "❌ Round {round}/{ROUNDS}: Data mismatch! Expected {data:?}, got {received:?}"
                );
                // Exit immediately on first failure
                return Err(anyhow!(
                    "Data verification failed on round {round}: expected {data:?}, got {received:?}"
                ));
            }
        } else {
            log::error!("❌ Round {round}/{ROUNDS}: Failed to parse values from JSON");
            return Err(anyhow!("Failed to parse JSON values on round {round}"));
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

    // Press Escape to exit Modbus settings
    session.send_escape()?;
    sleep_a_while().await;

    Ok(())
}
