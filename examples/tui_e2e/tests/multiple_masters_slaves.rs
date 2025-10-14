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

const REGISTER_LENGTH: usize = 12;
const MAX_RETRIES: usize = 10;
const RETRY_INTERVAL_MS: u64 = 1000;
const TIMEOUT_MS: u64 = 3000;

/// Test Multiple Independent Masters and Slaves with Intentional Conflicts
///
/// This test simulates 2 independent TUI masters with multiple CLI slaves polling them:
/// - TUI Master 1 on vcom1: Station ID 1, Register Type 03 (Holding), 12 registers
/// - TUI Master 2 on vcom3: Station ID 2, Register Type 02 (Input/Discrete), 12 registers
/// - CLI Slaves polling from various ports creating intentional conflicts
///
/// Test Design:
/// - vcom2 & vcom4: Slaves polling Station 1 (type 03) from their paired masters
/// - vcom5 & vcom6: Slaves polling Station 2 (type 02) from their paired masters
/// - Each slave attempts 10 times with 1s interval and 3s timeout
/// - Success criteria: At least 1 successful communication per port
///
/// The test validates:
/// 1. Multiple masters can operate independently
/// 2. Slaves can poll from masters successfully despite potential conflicts
/// 3. Communication reliability with retry logic
/// 4. Different register types (03 Holding, 02 Input) work correctly
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
    log::info!("  Master 1: {port1} (TUI, Station 1, Type 03 Holding)");
    log::info!("  Slave 1a: {port2} (CLI, polls Station 1)");
    log::info!("  Master 2: {port3} (TUI, Station 2, Type 02 Input)");
    log::info!("  Slave 2a: {port4} (CLI, polls Station 2)");
    log::info!("  Slave 1b: {port5} (CLI, polls Station 1)");
    log::info!("  Slave 2b: {port6} (CLI, polls Station 2)");

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
            return Err(anyhow!(
                "{name} ({port}) does not exist or is not available"
            ));
        }
    }
    log::info!("✅ All 6 virtual COM ports verified");

    // Spawn first TUI process (Master 1 on vcom1)
    log::info!("🧪 Step 1: Spawning TUI Master 1 process");
    let mut tui1_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Master 1 process: {err}"))?;
    let mut tui1_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Navigate to vcom1 and configure Master 1 (Station 1, Type 03 Holding)
    log::info!("🧪 Step 2: Configure TUI Master 1 on vcom1 (Station 1, Type 03)");
    configure_tui_master(&mut tui1_session, &mut tui1_cap, &port1, 1, 3).await?;

    // Spawn second TUI process (Master 2 on vcom3)
    log::info!("🧪 Step 3: Spawning TUI Master 2 process");
    let mut tui2_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI Master 2 process: {err}"))?;
    let mut tui2_cap = TerminalCapture::new(24, 80);

    sleep_seconds(3).await;

    // Navigate to vcom3 and configure Master 2 (Station 2, Type 02 Input)
    log::info!("🧪 Step 4: Configure TUI Master 2 on vcom3 (Station 2, Type 02)");
    configure_tui_master(&mut tui2_session, &mut tui2_cap, &port3, 2, 2).await?;

    // Run test rounds with both masters
    // Generate data once and keep it consistent for all retry attempts
    let data1 = generate_random_registers(REGISTER_LENGTH);
    let data2 = generate_random_registers(REGISTER_LENGTH);

    log::info!("🧪 Master 1 (Station 1, Type 03) data: {data1:?}");
    log::info!("🧪 Master 2 (Station 2, Type 02) data: {data2:?}");

    // Update Master 1 registers
    log::info!("🧪 Updating Master 1 registers");
    update_tui_registers(&mut tui1_session, &mut tui1_cap, &data1, false).await?;

    // Update Master 2 registers
    log::info!("🧪 Updating Master 2 registers");
    update_tui_registers(&mut tui2_session, &mut tui2_cap, &data2, false).await?;

    // Wait for IPC updates to propagate
    log::info!("🧪 Waiting for IPC propagation...");
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Test all 6 ports with retry logic
    // Track success for each port
    let mut port_success = std::collections::HashMap::new();

    // vcom2: Poll Station 1 (Type 03 Holding) from paired master on vcom1
    log::info!("🧪 Testing vcom2 → Station 1 (Type 03)");
    port_success.insert(
        "vcom2",
        test_port_with_retries(&port2, 1, "holding", &data1).await?,
    );

    // vcom4: Poll Station 1 (Type 03 Holding) - may have conflicts
    log::info!("🧪 Testing vcom4 → Station 1 (Type 03)");
    port_success.insert(
        "vcom4",
        test_port_with_retries(&port4, 1, "holding", &data1).await?,
    );

    // vcom5: Poll Station 2 (Type 02 Input) - may have conflicts
    log::info!("🧪 Testing vcom5 → Station 2 (Type 02)");
    port_success.insert(
        "vcom5",
        test_port_with_retries(&port5, 2, "input", &data2).await?,
    );

    // vcom6: Poll Station 2 (Type 02 Input) from paired master on vcom3
    log::info!("🧪 Testing vcom6 → Station 2 (Type 02)");
    port_success.insert(
        "vcom6",
        test_port_with_retries(&port6, 2, "input", &data2).await?,
    );

    // Check if all ports passed
    let all_passed = port_success.values().all(|&v| v);

    if all_passed {
        log::info!("✅ All ports passed!");
        for (port, success) in port_success.iter() {
            log::info!("  {port}: {}", if *success { "✅ PASS" } else { "❌ FAIL" });
        }
    } else {
        log::error!("❌ Some ports failed:");
        for (port, success) in port_success.iter() {
            log::error!("  {port}: {}", if *success { "✅ PASS" } else { "❌ FAIL" });
        }
        return Err(anyhow!("Not all ports passed the test"));
    }

    // Clean up both TUI processes
    log::info!("🧪 Cleaning up TUI processes");
    tui1_session.send_ctrl_c()?;
    tui2_session.send_ctrl_c()?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ Multiple Masters and Slaves test completed successfully!");
    Ok(())
}

/// Test a port with retry logic
/// Returns true if at least one attempt succeeded
async fn test_port_with_retries(
    port: &str,
    station_id: u8,
    register_mode: &str,
    expected_data: &[u16],
) -> Result<bool> {
    let binary = build_debug_bin("aoba")?;

    for attempt in 1..=MAX_RETRIES {
        log::info!(
            "  Attempt {attempt}/{MAX_RETRIES}: Polling {port} for Station {station_id} ({register_mode})"
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
                register_mode,
                "--baud-rate",
                "9600",
                "--timeout",
                &TIMEOUT_MS.to_string(),
                "--json",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if cli_output.status.success() {
            let stdout = String::from_utf8_lossy(&cli_output.stdout);

            // Parse and check the data
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let received: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();

                    if received == expected_data {
                        log::info!("  ✅ SUCCESS on attempt {attempt}: Data verified!");
                        return Ok(true);
                    } else {
                        log::warn!(
                            "  ⚠️ Data mismatch on attempt {attempt}: expected {expected_data:?}, got {received:?}"
                        );
                    }
                }
            }
        } else {
            let stderr = String::from_utf8_lossy(&cli_output.stderr);
            log::warn!("  ⚠️ Poll failed on attempt {attempt}: {stderr}");
        }

        // Wait before next attempt (except after last attempt)
        if attempt < MAX_RETRIES {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    log::error!("  ❌ FAILED: All {MAX_RETRIES} attempts failed for {port}");
    Ok(false)
}

/// Configure a TUI process as a Modbus Master on a specific port
async fn configure_tui_master<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
    station_id: u8,
    register_type: u8, // 2 = Input, 3 = Holding
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Master (Station {station_id}, Type {register_type:02}) on port {target_port}");

    // Navigate to the target port (vcom1 or vcom3)
    log::info!("📍 Navigating to port {target_port}");
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
        .capture(session, &format!("verify_modbus_panel_master{station_id}"))
        .await?;
    if !screen.contains("ModBus Master/Slave Settings") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for Master (Station {station_id})"
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
        &format!("create_station_master{station_id}"),
    )
    .await?;

    // Set Station ID if not 1
    if station_id != 1 {
        log::info!("📝 Setting station ID to {station_id}");
        let actions = vec![
            // Navigate to Station ID field (down 2 from top)
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 10,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::TypeString(station_id.to_string()),
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_station_id_{station_id}"),
        )
        .await?;
    }

    // Set Register Type
    log::info!("📝 Setting register type to {register_type:02}");

    let actions = vec![
        // Navigate to Register Type field
        CursorAction::PressArrow {
            direction: ArrowKey::Up,
            count: 10,
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 3,
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        // Navigate through register types
        CursorAction::PressArrow {
            direction: ArrowKey::Right,
            count: if register_type == 3 { 0 } else { 1 }, // Input is one right from Holding
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_register_type_{register_type}"),
    )
    .await?;

    // Set Register Length
    log::info!("📝 Setting register length to {REGISTER_LENGTH}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
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
        &format!("set_register_length_master{station_id}"),
    )
    .await?;

    log::info!("✅ Master (Station {station_id}, Type {register_type:02}) configured successfully");
    Ok(())
}

/// Navigate to a specific port in the TUI
async fn navigate_to_port<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
) -> Result<()> {
    log::info!("🔍 Navigating to port: {target_port}");

    // Capture screen - allow a few refresh cycles for port to appear
    let mut screen = String::new();
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 1..=MAX_ATTEMPTS {
        screen = cap
            .capture(session, &format!("nav_port_attempt_{attempt}"))
            .await?;

        if screen.contains(target_port) {
            if attempt > 1 {
                log::info!("✅ Port {target_port} detected after {attempt} attempts");
            }
            break;
        }

        if attempt == MAX_ATTEMPTS {
            return Err(anyhow!(
                "Port {target_port} not found in port list after {MAX_ATTEMPTS} attempts. Available: {}",
                screen
                    .lines()
                    .filter(|l| l.contains("/tmp/") || l.contains("/dev/"))
                    .take(10)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        log::info!("⏳ Port {target_port} not visible yet (attempt {attempt}/{MAX_ATTEMPTS})");
        sleep_seconds(1).await;
    }

    // Parse screen to find port line and cursor line
    let lines: Vec<&str> = screen.lines().collect();
    let mut port_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(target_port) {
            port_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let port_idx = port_line
        .ok_or_else(|| anyhow!("Could not find {target_port} line index in screen"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    log::info!("📍 Port at line {port_idx}, cursor at line {curr_idx}");

    // Check if already on the target port
    if port_idx == curr_idx {
        log::info!("✅ Cursor already on {target_port}, pressing Enter");
        session.send_enter()?;
        sleep_seconds(1).await;
        return Ok(());
    }

    // Calculate delta and move cursor
    let delta = port_idx.abs_diff(curr_idx);
    let direction = if port_idx > curr_idx {
        ArrowKey::Down
    } else {
        ArrowKey::Up
    };

    log::info!("➡️ Moving cursor {direction:?} by {delta} lines");

    let actions = vec![
        CursorAction::PressArrow {
            direction,
            count: delta,
        },
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "navigate_to_target_port").await?;

    // Press Enter to select the port
    session.send_enter()?;
    sleep_seconds(1).await;

    log::info!("✅ Successfully navigated to and selected {target_port}");
    Ok(())
}
