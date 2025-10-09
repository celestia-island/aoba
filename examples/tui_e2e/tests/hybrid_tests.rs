use anyhow::{anyhow, Result};
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::generate_random_registers,
    helpers::sleep_a_while,
    key_input::{ArrowKey, ExpectKeyExt},
    ports::{should_run_vcom_tests, vcom_matchers, VcomMatchers},
    snapshot::TerminalCapture,
    terminal::{build_debug_bin, spawn_expect_process},
    tui::{enable_port_carefully, navigate_to_vcom, update_tui_registers},
};

const ROUNDS: usize = 10;
const REGISTER_LENGTH: usize = 5;

/// Parse TUI log file for received register values
fn parse_tui_log_for_values(log_path: &str) -> Result<Vec<Vec<u16>>> {
    let file = File::open(log_path)?;
    let reader = BufReader::new(file);
    let mut result = Vec::new();

    // Pattern to match log entries like "Received holding registers (BE 0,1): [1234, 5678]"
    let pattern = Regex::new(r"Received holding registers.*?: \[(.*?)\]")?;

    for line in reader.lines() {
        let line = line?;
        if let Some(captures) = pattern.captures(&line) {
            if let Some(values_str) = captures.get(1) {
                // Parse the comma-separated values
                let values: Vec<u16> = values_str
                    .as_str()
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u16>().ok())
                    .collect();
                if !values.is_empty() {
                    result.push(values);
                }
            }
        }
    }

    Ok(result)
}

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

    // Create log file for TUI output
    let temp_dir = std::env::temp_dir();
    let tui_log_path = temp_dir.join("tui_slave_continuous.log");
    if tui_log_path.exists() {
        std::fs::remove_file(&tui_log_path)?;
    }

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

    Ok(())
}

/// Test TUI Master + CLI Slave with continuous random data (10 rounds)
/// TUI acts as Modbus Master (server) providing data
/// CLI acts as Modbus Slave (client) polling for data
pub async fn test_tui_master_with_cli_slave_continuous() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master + CLI Slave test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave continuous test (10 rounds)");

    let ports = vcom_matchers();

    // Verify vcom ports exist
    if !std::path::Path::new(&ports.port1_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port1_name));
    }
    if !std::path::Path::new(&ports.port2_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port2_name));
    }
    log::info!("✅ Virtual COM ports verified");

    // Create data file for CLI to write output
    let temp_dir = std::env::temp_dir();
    let cli_output_file = temp_dir.join("cli_slave_continuous_output.jsonl");
    if cli_output_file.exists() {
        std::fs::remove_file(&cli_output_file)?;
    }

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

    // Generate random data sets and update TUI, then poll with CLI
    let mut expected_data_sets: Vec<Vec<u16>> = Vec::new();
    for round in 1..=ROUNDS {
        let data = generate_random_registers(REGISTER_LENGTH);
        expected_data_sets.push(data.clone());
        log::info!("🧪 Round {}: Generated data {:?}", round, data);

        // Update TUI registers with new data
        update_tui_registers(&mut tui_session, &mut tui_cap, &data, false).await?;

        // Start CLI slave to poll data
        log::info!("🧪 Round {}: Starting CLI slave to poll", round);
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
            log::warn!("Round {}: CLI slave failed: {}", round, stderr);
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

    log::info!("✅ TUI Master + CLI Slave continuous test completed!");
    Ok(())
}

/// Configure TUI as Master (server providing data)
async fn configure_tui_master<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    // Enter port configuration
    session.send_line("")?;
    sleep_a_while().await;

    // Navigate to Mode selection and set to Master
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Move to Mode
        CursorAction::PressEnter, // Enter Mode
        CursorAction::PressEnter, // Select Master (default)
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Move to Role
        CursorAction::PressEnter, // Enter Role
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // Select Server
        CursorAction::PressEnter, // Confirm
    ];

    execute_cursor_actions(session, cap, &actions, "configure_master").await?;

    // Save configuration
    let save_actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 3 },  // Move through config
        CursorAction::PressEnter, // Save
    ];

    execute_cursor_actions(session, cap, &save_actions, "save_master_config").await?;

    Ok(())
}
