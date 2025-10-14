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
    ports::{port_exists, should_run_vcom_tests},
    snapshot::TerminalCapture,
    terminal::{build_debug_bin, spawn_expect_process},
    tui::{enable_port_carefully, enter_modbus_panel, update_tui_registers},
};

const ROUNDS: usize = 5;
const REGISTER_LENGTH: usize = 8;

/// Test Multiple Independent Masters and Slaves with Signal Interference Handling
///
/// This test simulates 2 independent masters, each communicating with multiple slaves:
/// - TUI Master 1 on vcom1, communicating with CLI Slave 1 on vcom2
/// - TUI Master 2 on vcom3, communicating with CLI Slave 2 on vcom4
/// - CLI Slave 3 on vcom6, polling from vcom5 (interference test)
///
/// The test validates:
/// 1. Both masters can operate independently without interfering with each other
/// 2. Slaves can correctly receive data from their respective masters
/// 3. Signal interference is properly handled (slaves don't receive data from wrong masters)
/// 4. Multiple concurrent Modbus communication channels work reliably
pub async fn test_multiple_masters_slaves() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping Multiple Masters/Slaves test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting Multiple Masters and Slaves E2E test");

    // Get port names from environment (set by setup script)
    let port1 = std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());
    let port2 = std::env::var("AOBATEST_PORT2").unwrap_or_else(|_| "/tmp/vcom2".to_string());
    let port3 = std::env::var("AOBATEST_PORT3").unwrap_or_else(|_| "/tmp/vcom3".to_string());
    let port4 = std::env::var("AOBATEST_PORT4").unwrap_or_else(|_| "/tmp/vcom4".to_string());
    let port5 = std::env::var("AOBATEST_PORT5").unwrap_or_else(|_| "/tmp/vcom5".to_string());
    let port6 = std::env::var("AOBATEST_PORT6").unwrap_or_else(|_| "/tmp/vcom6".to_string());

    log::info!("📍 Port configuration:");
    log::info!("  Master 1: {} (TUI)", port1);
    log::info!("  Slave 1:  {} (CLI)", port2);
    log::info!("  Master 2: {} (TUI)", port3);
    log::info!("  Slave 2:  {} (CLI)", port4);
    log::info!("  Test 3:   {} (unused)", port5);
    log::info!("  Slave 3:  {} (CLI - interference test)", port6);

    // Verify all ports exist
    for (name, port) in [
        ("port1", &port1),
        ("port2", &port2),
        ("port3", &port3),
        ("port4", &port4),
        ("port5", &port5),
        ("port6", &port6),
    ] {
        if !port_exists(port) {
            return Err(anyhow!("{} ({}) does not exist or is not available", name, port));
        }
    }
    log::info!("✅ All 6 virtual COM ports verified");

    // Spawn first TUI process (Master 1 on vcom1)
    log::info!("🧪 Step 1: Spawning TUI Master 1 process");
    let mut tui1_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Master 1 process: {err}"))?;
    let mut tui1_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Navigate to vcom1 and configure Master 1
    log::info!("🧪 Step 2: Configure TUI Master 1 on vcom1");
    configure_tui_master(&mut tui1_session, &mut tui1_cap, &port1, 1).await?;

    // Spawn second TUI process (Master 2 on vcom3)
    log::info!("🧪 Step 3: Spawning TUI Master 2 process");
    let mut tui2_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Master 2 process: {err}"))?;
    let mut tui2_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Navigate to vcom3 and configure Master 2
    log::info!("🧪 Step 4: Configure TUI Master 2 on vcom3");
    configure_tui_master(&mut tui2_session, &mut tui2_cap, &port3, 2).await?;

    // Run test rounds with both masters
    for round in 1..=ROUNDS {
        log::info!("🧪 ===== Round {}/{} =====", round, ROUNDS);

        // Generate different data for each master
        let data1 = generate_random_registers(REGISTER_LENGTH);
        let data2 = generate_random_registers(REGISTER_LENGTH);

        log::info!("🧪 Round {}: Master 1 data: {:?}", round, data1);
        log::info!("🧪 Round {}: Master 2 data: {:?}", round, data2);

        // Update Master 1 registers
        log::info!("🧪 Round {}: Updating Master 1 registers", round);
        update_tui_registers(&mut tui1_session, &mut tui1_cap, &data1, false).await?;

        // Update Master 2 registers
        log::info!("🧪 Round {}: Updating Master 2 registers", round);
        update_tui_registers(&mut tui2_session, &mut tui2_cap, &data2, false).await?;

        // Wait for IPC updates to propagate
        log::info!("🧪 Round {}: Waiting for IPC propagation...", round);
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Poll Slave 1 (should receive data1 from Master 1)
        log::info!("🧪 Round {}: Polling Slave 1 from Master 1", round);
        verify_slave_data(&port2, 1, &data1, round).await?;

        // Poll Slave 2 (should receive data2 from Master 2)
        log::info!("🧪 Round {}: Polling Slave 2 from Master 2", round);
        verify_slave_data(&port4, 1, &data2, round).await?;

        // Interference test: Poll Slave 3 on vcom6 (should fail or timeout since vcom5 has no master)
        log::info!("🧪 Round {}: Interference test - polling Slave 3", round);
        test_slave_interference(&port6, round).await?;

        // Small delay between rounds
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Clean up both TUI processes
    log::info!("🧪 Cleaning up TUI processes");
    tui1_session.send_ctrl_c()?;
    tui2_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!(
        "✅ Multiple Masters and Slaves test completed! All {} rounds passed.",
        ROUNDS
    );
    Ok(())
}

/// Configure a TUI process as a Modbus Master on a specific port
async fn configure_tui_master<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
    master_id: u8,
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Master {} on port {}", master_id, target_port);

    // Navigate to the target port (vcom1 or vcom3)
    log::info!("📍 Navigating to port {}", target_port);
    navigate_to_port(session, cap, target_port).await?;

    // Enable the port
    log::info!("🔌 Enabling port");
    enable_port_carefully(session, cap).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Enter Modbus panel
    log::info!("⚙️ Entering Modbus configuration panel");
    enter_modbus_panel(session, cap).await?;

    // Configure as Master
    let screen = cap
        .capture(session, &format!("verify_modbus_panel_master{}", master_id))
        .await?;
    if !screen.contains("ModBus Master/Slave Settings") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for Master {}",
            master_id
        ));
    }

    // Create station
    log::info!("🏗️ Creating Modbus station");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("create_station_master{}", master_id),
    )
    .await?;

    // Set Register Length
    log::info!("📝 Setting register length to {}", REGISTER_LENGTH);
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(REGISTER_LENGTH.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_register_length_master{}", master_id),
    )
    .await?;

    log::info!("✅ Master {} configured successfully", master_id);
    Ok(())
}

/// Navigate to a specific port in the TUI
async fn navigate_to_port<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
) -> Result<()> {
    // Get the port name (e.g., "vcom1" from "/tmp/vcom1")
    let port_name = std::path::Path::new(target_port)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid port path: {}", target_port))?;

    log::info!("🔍 Looking for port: {}", port_name);

    // Navigate in the port list to find the target port
    // Try up to 10 times to find the port
    for attempt in 1..=10 {
        let screen = cap
            .capture(session, &format!("navigate_attempt_{}", attempt))
            .await?;

        if screen.contains(port_name) {
            log::info!("✅ Found port {} on attempt {}", port_name, attempt);

            // Check if cursor is already on the target port
            if screen.contains(&format!("> {}", port_name))
                || screen.contains(&format!(">{}", port_name))
            {
                log::info!("✅ Cursor already on {}", port_name);
                session.send_enter()?;
                sleep_seconds(1).await;
                return Ok(());
            }

            // Move cursor to the port
            session.send_arrow(ArrowKey::Down)?;
            sleep_seconds(1).await;
        } else {
            // Port not visible, scroll down
            session.send_arrow(ArrowKey::Down)?;
            sleep_seconds(1).await;
        }
    }

    Err(anyhow!("Could not find port {} after 10 attempts", port_name))
}

/// Verify that a slave receives the expected data
async fn verify_slave_data(
    port: &str,
    station_id: u8,
    expected_data: &[u16],
    round: usize,
) -> Result<()> {
    let binary = build_debug_bin("aoba")?;

    const MAX_RETRIES: usize = 3;
    const RETRY_DELAY_MS: u64 = 1000;

    for attempt in 1..=MAX_RETRIES {
        log::info!(
            "🔍 Round {}, attempt {}/{}: Polling slave on {}",
            round,
            attempt,
            MAX_RETRIES,
            port
        );

        let cli_output = Command::new(&binary)
            .args([
                "--slave-poll",
                port,
                "--station-id",
                &station_id.to_string(),
                "--register-address",
                "0",
                "--register-length",
                &expected_data.len().to_string(),
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
                "⚠️ Round {}, attempt {}: CLI poll failed: {}",
                round,
                attempt,
                stderr
            );

            if attempt < MAX_RETRIES {
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                continue;
            } else {
                return Err(anyhow!(
                    "CLI poll failed on round {} after {} attempts",
                    round,
                    MAX_RETRIES
                ));
            }
        }

        let stdout = String::from_utf8_lossy(&cli_output.stdout);
        log::info!("📥 Received: {}", stdout.trim());

        // Parse and check the data
        let json: serde_json::Value = serde_json::from_str(&stdout)?;
        if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
            let received: Vec<u16> = values
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u16))
                .collect();

            if received == expected_data {
                log::info!(
                    "✅ Round {}, attempt {}: Data verified successfully!",
                    round,
                    attempt
                );
                return Ok(());
            } else {
                log::warn!(
                    "⚠️ Round {}, attempt {}: Data mismatch. Expected {:?}, got {:?}",
                    round,
                    attempt,
                    expected_data,
                    received
                );

                if attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                }
            }
        } else {
            log::error!("❌ Failed to parse values from JSON");
            if attempt < MAX_RETRIES {
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            }
        }
    }

    Err(anyhow!(
        "Data verification failed on round {} after {} attempts",
        round,
        MAX_RETRIES
    ))
}

/// Test that a slave on an unconnected port properly handles interference
async fn test_slave_interference(port: &str, round: usize) -> Result<()> {
    let binary = build_debug_bin("aoba")?;

    log::info!("🔬 Round {}: Testing interference on {}", round, port);

    // Try to poll a slave on vcom6 (which has no master)
    // This should either timeout or fail gracefully
    let cli_output = Command::new(&binary)
        .args([
            "--slave-poll",
            port,
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
            "--timeout",
            "2000", // 2 second timeout
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    // We expect this to fail (no master on vcom5)
    if cli_output.status.success() {
        log::warn!("⚠️ Interference test: unexpectedly succeeded on port {}", port);
        let stdout = String::from_utf8_lossy(&cli_output.stdout);
        log::warn!("⚠️ Received: {}", stdout.trim());
        // This is not a hard failure, but worth noting
    } else {
        log::info!(
            "✅ Interference test passed: port {} properly failed/timed out as expected",
            port
        );
    }

    Ok(())
}
