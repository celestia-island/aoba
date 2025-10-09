// Test CLI Master (Slave/Server) with TUI Slave (Master/Client) - Continuous mode
// This test performs continuous random data updates in CLI Master and verifies TUI Slave polls them correctly
// Uses log file analysis for data verification while keeping TUI interaction tests

use anyhow::{anyhow, Result};
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::{generate_random_coils, generate_random_registers},
    ports::vcom_matchers,
    {should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture},
};

/// Parse TUI log file for received register values
fn parse_tui_log_for_values(log_path: &str, register_mode: &str) -> Result<Vec<Vec<u16>>> {
    let file = File::open(log_path)?;
    let reader = BufReader::new(file);
    let mut result = Vec::new();

    // Pattern to match log entries like "Received holding registers (BE 0,1): [1234, 5678]"
    let pattern_str = match register_mode {
        "holding" => r"Received holding registers.*?: \[(.*?)\]",
        "coils" => r"Received coils: \[(.*?)\]",
        _ => {
            return Err(anyhow!(
                "Unsupported register mode for log parsing: {register_mode}"
            ))
        }
    };

    let pattern = Regex::new(pattern_str)?;

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

/// Test CLI Master with TUI Slave - Continuous mode
/// This test runs continuous random updates from CLI and verifies TUI receives them
pub async fn test_cli_master_continuous_with_tui_slave(register_mode: &str) -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping CLI Master continuous test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting CLI Master + TUI Slave continuous test (mode: {register_mode})");

    let ports = vcom_matchers();
    
    // Verify vcom ports exist
    if !std::path::Path::new(&ports.port1_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port1_name));
    }
    if !std::path::Path::new(&ports.port2_name).exists() {
        return Err(anyhow!("{} was not created by socat", ports.port2_name));
    }
    log::info!("✓ Virtual COM ports verified");

    // Determine if this is a coil type register
    let is_coil = register_mode == "coils";
    let register_length = if is_coil { 8 } else { 6 };

    // Clear TUI log file before starting
    let tui_log_path = "/tmp/tui_e2e.log";
    if std::path::Path::new(tui_log_path).exists() {
        std::fs::remove_file(tui_log_path)?;
    }

    // Prepare data file for CLI master
    log::info!("🧪 Step 1: Prepare test data file with multiple random updates");
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join(format!("cli_master_continuous_{register_mode}.json"));

    let mut all_expected_values = Vec::new();
    {
        let mut file = File::create(&data_file)?;
        // Create 5 lines of random data
        for _ in 0..5 {
            let values = if is_coil {
                generate_random_coils(register_length)
            } else {
                generate_random_registers(register_length)
            };
            writeln!(file, "{{\"values\": {values:?}}}")?;
            all_expected_values.push(values);
        }
    }
    log::info!(
        "✓ Test data file created with {} value sets",
        all_expected_values.len()
    );
    for (i, values) in all_expected_values.iter().enumerate() {
        log::info!("  Set {}: {:?}", i + 1, values);
    }

    // Spawn TUI process first (will be slave on vcom1)
    log::info!("🧪 Step 2: Spawn TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 3: Verify TUI loaded");
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
    log::info!("🧪 Step 4: Navigate to vcom1");
    ci_utils::tui::navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Enable the port FIRST (before configuration)
    log::info!("🧪 Step 5: Enable the port");
    enable_port(&mut tui_session, &mut tui_cap).await?;

    // Configure as Slave mode (AFTER enabling port)
    log::info!("🧪 Step 6: Configure TUI as Slave (mode: {register_mode})");
    configure_tui_slave(
        &mut tui_session,
        &mut tui_cap,
        register_mode,
        register_length,
    )
    .await?;

    // Now start CLI master after TUI slave is ready
    log::info!("🧪 Step 7: Start CLI master on {}", ports.port2_name);
    let binary = ci_utils::build_debug_bin("aoba")?;

    let mut cli_master = Command::new(&binary)
        .args([
            "--master-provide-persist",
            &ports.port2_name,
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
            "--data-source",
            &format!("file:{}", data_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Show CLI master logs in test output
        .spawn()?;

    // Check if CLI master is still running
    sleep_a_while().await;
    match cli_master.try_wait()? {
        Some(status) => {
            // CLI master exited - note: stderr is inherited so already visible in logs
            return Err(anyhow!(
                "CLI master exited prematurely with status {status}"
            ));
        }
        None => {
            log::info!("✅ CLI master is running");
        }
    }

    // Wait for communication to happen (polls should occur automatically)
    // Need to wait at least 1-2 poll cycles (1 second each) plus time for CLI master to send updates
    log::info!("🧪 Step 8: Wait for master-slave communication...");
    log::info!("   Waiting 4 seconds for polling cycles to complete...");
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    // Parse TUI log file for received values
    log::info!("🧪 Step 9: Verify data from TUI log file");
    let tui_log_path = "/tmp/tui_e2e.log";

    // Check if log file exists
    if !std::path::Path::new(tui_log_path).exists() {
        log::warn!("⚠️ TUI log file doesn't exist at {tui_log_path}");
        log::warn!("   TUI may not have logged any data yet");
    } else {
        let log_size = std::fs::metadata(tui_log_path)?.len();
        log::info!("TUI log file exists with {log_size} bytes");
    }

    let received_values = parse_tui_log_for_values(tui_log_path, register_mode)?;
    log::info!("Found {} value sets in TUI log", received_values.len());

    // Log some entries from the log file for debugging
    if received_values.is_empty() {
        log::error!("❌ No values found in TUI log. Dumping last 20 lines of log:");
        if let Ok(content) = std::fs::read_to_string(tui_log_path) {
            for line in content
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .iter()
                .rev()
            {
                log::error!("  {line}");
            }
        }
        return Err(anyhow!(
            "No values found in TUI log file - TUI slave did not receive any data from CLI master"
        ));
    }

    // Verify that at least some expected values were received
    let mut found_count = 0;
    for (i, expected) in all_expected_values.iter().enumerate() {
        let found = received_values.iter().any(|received| {
            // Check if all expected values appear in the received set
            expected.iter().all(|exp_val| received.contains(exp_val))
        });
        if found {
            log::info!(
                "✅ Expected value set {} found in logs: {expected:?}",
                i + 1
            );
            found_count += 1;
        } else {
            log::warn!(
                "⚠️ Expected value set {} NOT found in logs: {expected:?}",
                i + 1
            );
        }
    }

    if found_count == 0 {
        log::error!("❌ No expected value sets were found in TUI logs");
        log::error!("   Received value sets from log:");
        for (i, received) in received_values.iter().enumerate() {
            log::error!("     Set {}: {received:?}", i + 1);
        }
        return Err(anyhow!(
            "None of the expected value sets were found in TUI log - data mismatch between CLI master and TUI slave"
        ));
    } else {
        log::info!(
            "✅ Found {found_count}/{} expected value sets in TUI log",
            all_expected_values.len()
        );
    }

    // Capture screen to verify both register display and log panel
    log::info!("🧪 Step 10: Capture screen to verify display consistency");
    let screen = tui_cap.capture(&mut tui_session, "final_screen")?;
    log::info!("📸 Final screen captured");

    // Verify screen shows some activity (non-zero values)
    let has_values = screen.contains("0x") && !screen.lines().all(|l| l.contains("0x0000"));
    if has_values {
        log::info!("✅ Screen shows register values (hex patterns found)");
    } else {
        log::warn!("⚠️ Screen may not show expected values");
    }

    // Cleanup
    log::info!("🧪 Step 11: Cleanup");
    cli_master.kill()?;
    cli_master.wait()?;

    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    // Clean up data file
    if data_file.exists() {
        std::fs::remove_file(&data_file)?;
    }

    log::info!("✅ CLI Master + TUI Slave continuous test completed (mode: {register_mode})");
    Ok(())
}

/// Configure TUI as Modbus Slave
async fn configure_tui_slave<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    register_mode: &str,
    register_length: usize,
) -> Result<()> {
    log::info!("📝 Configuring as Slave (mode: {register_mode})...");

    // Navigate to Modbus settings
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
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
    execute_cursor_actions(session, cap, &actions, "enter_modbus_settings").await?;

    // Create station first
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

    // Navigate to Connection Mode and change to Slave
    log::info!("Setting connection mode to Slave");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 1,
        },
        CursorAction::PressEnter,
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Right,
            count: 1,
        },
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_slave_mode").await?;

    // Navigate to Register Mode and set it
    log::info!("Setting register mode to: {register_mode}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 2,
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
            direction: ci_utils::ArrowKey::Right,
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
            direction: ci_utils::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    // Exit Modbus settings
    let actions = vec![CursorAction::PressEscape, CursorAction::Sleep { ms: 500 }];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_settings").await?;

    // Give the TUI time to process configuration changes before enabling
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    log::info!("✓ Slave configuration complete");
    Ok(())
}

/// Enable the serial port in TUI
async fn enable_port<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    log::info!("🔌 Enabling port...");

    let screen = cap.capture(session, "before_enable")?;

    if !screen.contains("Enable Port") {
        return Err(anyhow!("Not in port details page"));
    }

    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Enabled")?,
            description: "Port enabled".to_string(),
            line_range: Some((2, 5)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enable_port").await?;

    log::info!("✓ Port enabled");
    Ok(())
}

/// Capture current values from TUI Modbus panel
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🧪 Running TUI E2E Continuous Tests: CLI Master + TUI Slave (Holding Registers)");

    // Test only holding registers
    let mode = "holding";
    log::info!("\n========== Testing register mode: {mode} ==========\n");
    match test_cli_master_continuous_with_tui_slave(mode).await {
        Ok(_) => {
            log::info!("✅ Test passed for mode: {mode}");
        }
        Err(e) => {
            log::error!("❌ Test failed for mode {mode}: {e}");
            return Err(e);
        }
    }

    log::info!("\n✅ TUI Slave continuous test (holding) passed!");
    Ok(())
}
