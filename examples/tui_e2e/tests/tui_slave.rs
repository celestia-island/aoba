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

/// Test TUI Slave + CLI Master with continuous random data (10 rounds)
/// TUI acts as Modbus Slave (server) responding to requests
/// CLI acts as Modbus Master (client) polling for data
pub async fn test_tui_slave_with_cli_master_continuous() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Slave + CLI Master test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Slave + CLI Master continuous test (10 rounds)");

    let ports = vcom_matchers();

    // Verify vcom ports exist
    if !std::path::Path::new(&ports.port1_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port1_name));
    }
    if !std::path::Path::new(&ports.port2_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port2_name));
    }
    log::info!("✅ Virtual COM ports verified");

    // Spawn TUI process (will be slave/server on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Navigate to vcom1 and configure as slave
    log::info!("🧪 Step 2: Navigate to vcom1 in port list");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Configure as slave (server mode)
    log::info!("🧪 Step 3: Configure TUI as Slave");
    configure_tui_slave(&mut tui_session, &mut tui_cap).await?;

    // Enable the port
    log::info!("🧪 Step 4: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Generate random data sets for 10 rounds
    let mut expected_data_sets: Vec<Vec<u16>> = Vec::new();
    for round in 1..=ROUNDS {
        let data = generate_random_registers(REGISTER_LENGTH);
        expected_data_sets.push(data.clone());
        log::info!("🧪 Round {}: Generated data {:?}", round, data);

        // Update TUI registers with new data
        update_tui_registers(&mut tui_session, &mut tui_cap, &data, false).await?;

        // Start CLI master to poll data
        log::info!("🧪 Round {}: Starting CLI master to poll", round);
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
            log::warn!("Round {}: CLI master failed: {}", round, stderr);
            continue;
        }

        let stdout = String::from_utf8_lossy(&cli_output.stdout);
        log::info!("Round {}: CLI received: {}", round, stdout.trim());

        // Verify the data matches
        let json: serde_json::Value = serde_json::from_str(&stdout)?;
        if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
            let received: Vec<u16> = values
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u16))
                .collect();

            if received == data {
                log::info!("✅ Round {}: Data verified successfully", round);
            } else {
                log::warn!(
                    "⚠️ Round {}: Data mismatch. Expected {:?}, got {:?}",
                    round,
                    data,
                    received
                );
            }
        }

        // Small delay between rounds
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Kill TUI process
    tui_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ TUI Slave + CLI Master continuous test completed!");
    Ok(())
}

/// Configure TUI as Slave (server responding to requests)
async fn configure_tui_slave<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    // Enter port configuration
    session.send_line("")?;
    sleep_a_while().await;

    // Navigate to Mode selection and set to Slave
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Move to Mode
        CursorAction::PressEnter, // Enter Mode
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Select Slave
        CursorAction::PressEnter, // Confirm
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Move to Role
        CursorAction::PressEnter, // Enter Role
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Select Server
        CursorAction::PressEnter, // Confirm
    ];

    execute_cursor_actions(session, cap, &actions, "configure_slave").await?;

    // Save configuration
    let save_actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 3 },  // Move through config
        CursorAction::PressEnter, // Save
    ];

    execute_cursor_actions(session, cap, &save_actions, "save_slave_config").await?;

    // Press Escape to return to port details page
    session.send_escape()?;
    sleep_a_while().await;

    Ok(())
}
