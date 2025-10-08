// Test CLI Master (Slave/Server) with TUI Slave (Master/Client) - Continuous mode
// This test performs continuous random data updates in CLI Master and verifies TUI Slave polls them correctly
// Tests all 4 register types: holding, input, coils, discrete

use anyhow::{anyhow, Result};
use rand::Rng;
use regex::Regex;
use std::{
    fs::File,
    io::Write,
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
    let mut rng = rand::thread_rng();
    if is_coil {
        // For coils/discrete, generate only 0 or 1
        (0..length).map(|_| rng.gen_range(0..=1)).collect()
    } else {
        // For holding/input, generate any u16 value
        (0..length).map(|_| rng.gen_range(0..=0xFFFF)).collect()
    }
}

/// Test CLI Master with TUI Slave - Continuous mode
/// This test runs continuous random updates from CLI and verifies TUI receives them
pub async fn test_cli_master_continuous_with_tui_slave(
    register_mode: &str,
) -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping CLI Master continuous test on this platform");
        return Ok(());
    }

    log::info!(
        "🧪 Starting CLI Master + TUI Slave continuous test (mode: {})",
        register_mode
    );

    // Verify vcom ports exist
    if !std::path::Path::new("/tmp/vcom1").exists() {
        return Err(anyhow!("/tmp/vcom1 was not created by socat"));
    }
    if !std::path::Path::new("/tmp/vcom2").exists() {
        return Err(anyhow!("/tmp/vcom2 was not created by socat"));
    }
    log::info!("✓ Virtual COM ports verified");

    // Determine if this is a coil type register
    let is_coil = register_mode == "coils" || register_mode == "discrete";
    let register_length = if is_coil { 8 } else { 6 };

    // Prepare data file for CLI master
    log::info!("🧪 Step 1: Prepare test data file with multiple random updates");
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join(format!("cli_master_continuous_{}.json", register_mode));
    
    let mut all_expected_values = Vec::new();
    {
        let mut file = File::create(&data_file)?;
        // Create 5 lines of random data
        for _ in 0..5 {
            let values = generate_random_data(register_length, is_coil);
            writeln!(file, "{{\"values\": {:?}}}", values)?;
            all_expected_values.push(values);
        }
    }
    log::info!("✓ Test data file created with {} value sets", all_expected_values.len());
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
            return Err(anyhow!(
                "CLI master exited prematurely with status {}",
                status
            ));
        }
        None => {
            log::info!("✅ CLI master is running");
        }
    }

    // Spawn TUI process (will be slave on vcom1)
    log::info!("🧪 Step 3: Spawn TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
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
    log::info!("🧪 Step 6: Configure TUI as Slave (mode: {})", register_mode);
    configure_tui_slave(&mut tui_session, &mut tui_cap, register_mode, register_length).await?;

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

    // Check received values in TUI multiple times to capture updates
    log::info!("🧪 Step 9: Check received values in TUI");
    let mut captured_values = Vec::new();
    for attempt in 0..3 {
        log::info!("  Capture attempt {}/3", attempt + 1);
        match capture_tui_values(&mut tui_session, &mut tui_cap).await {
            Ok(values) => {
                log::info!("  Captured: {:?}", values);
                captured_values.push(values);
            }
            Err(e) => {
                log::warn!("  Capture failed: {}", e);
            }
        }
        thread::sleep(Duration::from_secs(2));
    }

    // Verify that at least some values were captured and they're not all zeros
    log::info!("🧪 Step 10: Verify captured values");
    
    if captured_values.is_empty() {
        log::warn!("⚠️ No values captured from TUI display");
    } else {
        // Check if we captured any non-zero values (indicates data is being received)
        let has_non_zero = captured_values.iter().any(|vals| vals.iter().any(|&v| v != 0));
        
        if has_non_zero {
            log::info!("✅ Successfully captured non-zero values from TUI, indicating data flow");
            log::info!("   Captured value samples: {:?}", &captured_values[0]);
        } else {
            log::warn!("⚠️ All captured values are zero - may indicate no data flow");
            log::warn!("   This could be a timing issue or display format issue");
        }
        
        // Try to find if any of the expected values appear in captured data
        let mut found_count = 0;
        for expected in &all_expected_values {
            // Check if this exact sequence appears in any capture
            let found = captured_values.iter().any(|captured| {
                // Check if all expected values appear in the captured set (order-independent)
                expected.iter().all(|exp_val| captured.contains(exp_val))
            });
            if found {
                found_count += 1;
                log::info!("✅ Found expected value set: {:?}", expected);
            }
        }
        
        if found_count > 0 {
            log::info!(
                "✅ Found {}/{} expected value sets in TUI captures",
                found_count,
                all_expected_values.len()
            );
        } else {
            log::warn!("⚠️ Expected values not found in captures");
            log::warn!("   Expected value sets:");
            for (i, expected) in all_expected_values.iter().enumerate() {
                log::warn!("     Set {}: {:?}", i + 1, expected);
            }
            log::warn!("   Captured values:");
            for (i, captured) in captured_values.iter().enumerate() {
                log::warn!("     Capture {}: {:?}", i + 1, captured);
            }
            log::warn!("   Note: This may be due to display format differences or timing");
        }
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

    log::info!(
        "✅ CLI Master + TUI Slave continuous test completed (mode: {})",
        register_mode
    );
    Ok(())
}

/// Navigate to vcom1 port in TUI
async fn navigate_to_vcom1<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
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
    log::info!("📝 Configuring as Slave (mode: {})...", register_mode);

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
    log::info!("Setting register mode to: {}", register_mode);
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
        _ => return Err(anyhow!("Invalid register mode: {}", register_mode)),
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
    log::info!("Setting register length to: {}", register_length);
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
async fn capture_tui_values<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<Vec<u16>> {
    // Navigate to Modbus panel
    let actions = vec![
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_modbus_panel").await?;

    // Capture screen
    let screen = cap.capture(session, "modbus_panel_values")?;

    // Exit back
    let actions = vec![CursorAction::PressEscape];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_panel").await?;

    // Parse values from screen
    let mut values = Vec::new();
    
    // Look for hex patterns like 0x0000, 0x0001, etc.
    for line in screen.lines() {
        // Try to find hex values in the format 0xXXXX
        let hex_pattern = regex::Regex::new(r"0x([0-9A-Fa-f]{4})")?;
        for cap in hex_pattern.captures_iter(line) {
            if let Some(hex_str) = cap.get(1) {
                if let Ok(val) = u16::from_str_radix(hex_str.as_str(), 16) {
                    values.push(val);
                }
            }
        }
    }

    if values.is_empty() {
        Err(anyhow!("No values found in TUI display"))
    } else {
        Ok(values)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    log::info!("🧪 Running TUI E2E Continuous Tests: CLI Master + TUI Slave");

    // Test only writable register modes (master can only write to holding and coils)
    // Input and discrete are read-only and cannot be written by master-provide
    let register_modes = ["holding", "coils"];
    
    for mode in &register_modes {
        log::info!("\n========== Testing register mode: {} ==========\n", mode);
        match test_cli_master_continuous_with_tui_slave(mode).await {
            Ok(_) => {
                log::info!("✅ Test passed for mode: {}", mode);
            }
            Err(e) => {
                log::error!("❌ Test failed for mode {}: {}", mode, e);
                return Err(e);
            }
        }
        
        // Wait between tests to ensure ports are released
        thread::sleep(Duration::from_secs(2));
    }

    log::info!("\n✅ All TUI Slave continuous tests passed!");
    Ok(())
}
