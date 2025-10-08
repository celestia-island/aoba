// Test CLI Master (Slave/Server) with TUI Slave (Master/Client) - Continuous mode
// This test performs continuous random data updates in CLI Master and verifies TUI Slave polls them correctly
// Uses log file analysis for data verification while keeping TUI interaction tests

use anyhow::{anyhow, Result};
use rand::random;
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use expectrl::Expect;

use aoba::ci::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    {should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture},
};

/// Generate pseudo-random modbus data using rand crate
fn generate_random_data(length: usize, is_coil: bool) -> Vec<u16> {
    if is_coil {
        // For coils/discrete, generate only 0 or 1
        (0..length)
            .map(|_| if random::<u8>() % 2 == 0 { 0 } else { 1 })
            .collect()
    } else {
        // For holding/input, generate any u16 value
        (0..length).map(|_| random::<u16>()).collect()
    }
}

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

    // Verify vcom ports exist
    if !std::path::Path::new("/tmp/vcom1").exists() {
        return Err(anyhow!("/tmp/vcom1 was not created by socat"));
    }
    if !std::path::Path::new("/tmp/vcom2").exists() {
        return Err(anyhow!("/tmp/vcom2 was not created by socat"));
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
            let values = generate_random_data(register_length, is_coil);
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

    // Start CLI master in persistent mode
    log::info!("🧪 Step 2: Start CLI master on vcom2");
    let binary = aoba::ci::build_debug_bin("aoba")?;

    let mut cli_master = Command::new(&binary)
        .args([
            "--master-provide-persist",
            "/tmp/vcom2",
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
        .stderr(Stdio::piped())
        .spawn()?;

    // Check if CLI master is still running
    thread::sleep(Duration::from_secs(2));
    match cli_master.try_wait()? {
        Some(status) => {
            // CLI master exited - capture stderr for debugging
            let stderr = if let Some(mut stderr_handle) = cli_master.stderr.take() {
                let mut buf = String::new();
                use std::io::Read;
                stderr_handle.read_to_string(&mut buf).ok();
                buf
            } else {
                String::new()
            };
            let stdout = if let Some(mut stdout_handle) = cli_master.stdout.take() {
                let mut buf = String::new();
                use std::io::Read;
                stdout_handle.read_to_string(&mut buf).ok();
                buf
            } else {
                String::new()
            };
            log::error!("CLI master stdout: {}", stdout);
            log::error!("CLI master stderr: {}", stderr);
            return Err(anyhow!(
                "CLI master exited prematurely with status {status}, stderr: {stderr}"
            ));
        }
        None => {
            log::info!("✅ CLI master is running");
        }
    }

    // Spawn TUI process (will be slave on vcom1)
    log::info!("🧪 Step 3: Spawn TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 4: Verify TUI loaded");
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
    log::info!("🧪 Step 5: Navigate to vcom1");
    navigate_to_vcom1(&mut tui_session, &mut tui_cap).await?;

    // Configure as Slave mode
    log::info!("🧪 Step 6: Configure TUI as Slave (mode: {register_mode})");
    configure_tui_slave(
        &mut tui_session,
        &mut tui_cap,
        register_mode,
        register_length,
    )
    .await?;

    // Enable the port
    log::info!("🧪 Step 7: Enable the port");
    enable_port(&mut tui_session, &mut tui_cap).await?;

    // Wait for communication to happen (polls should occur automatically)
    log::info!("🧪 Step 8: Wait for master-slave communication...");
    // Wait longer to allow multiple polls to happen
    for i in 0..5 {
        thread::sleep(Duration::from_secs(2));
        log::info!("  Waiting... ({}/5)", i + 1);
    }

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

/// Navigate to vcom1 port in TUI
async fn navigate_to_vcom1<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    log::info!("📍 Finding vcom1 in port list...");

    // Go to top first
    let go_to_top = vec![CursorAction::PressArrow {
        direction: aoba::ci::ArrowKey::Up,
        count: 50,
    }];
    execute_cursor_actions(session, cap, &go_to_top, "go_to_top").await?;

    let screen = cap.capture(session, "after_going_to_top")?;

    if !screen.contains("/tmp/vcom1") {
        return Err(anyhow!("vcom1 not found in port list"));
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("/tmp/vcom1") {
            vcom1_line = Some(idx);
        }
        if line.contains("> /tmp/") || line.contains("> /dev/") {
            cursor_line = Some(idx);
        }
    }

    let vcom1_idx = vcom1_line.ok_or_else(|| anyhow!("Could not find vcom1 line index"))?;
    let curr_idx = cursor_line.ok_or_else(|| anyhow!("Could not find cursor line"))?;

    if vcom1_idx != curr_idx {
        let delta = vcom1_idx.abs_diff(curr_idx);
        let direction = if vcom1_idx > curr_idx {
            aoba::ci::ArrowKey::Down
        } else {
            aoba::ci::ArrowKey::Up
        };

        let actions = vec![CursorAction::PressArrow {
            direction,
            count: delta,
        }];
        execute_cursor_actions(session, cap, &actions, "nav_to_vcom1").await?;
    }

    // Press Enter to enter vcom1 details
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"/tmp/vcom1")?,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;

    log::info!("✓ Successfully entered vcom1 details");
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
            direction: aoba::ci::ArrowKey::Down,
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
            direction: aoba::ci::ArrowKey::Down,
            count: 1,
        },
        CursorAction::PressEnter,
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Right,
            count: 1,
        },
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_slave_mode").await?;

    // Navigate to Register Mode and set it
    log::info!("Setting register mode to: {register_mode}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
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
            direction: aoba::ci::ArrowKey::Right,
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
            direction: aoba::ci::ArrowKey::Down,
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
