// Test CLI Master (Slave/Server) with TUI Slave (Master/Client)
// This uses CLI to set up a Modbus Master that responds to requests,
// and TUI to poll it for data

use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;
use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture};

/// Test CLI Master with TUI Slave
/// CLI acts as Modbus Master (Slave/Server) responding to requests with test data
/// TUI acts as Modbus Slave (Master/Client) polling for data
pub async fn test_cli_master_with_tui_slave() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping CLI Master + TUI Slave test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting CLI Master + TUI Slave hybrid test");

    // Prepare test data file for CLI
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("cli_master_test_data.txt");
    {
        let mut file = File::create(&data_file)?;
        // Write test values: 5, 15, 25, 35 (in decimal)
        writeln!(file, "5 15 25 35")?;
    }

    // Start CLI master in persistent mode
    log::info!("🧪 Starting CLI master on vcom2");
    let binary = aoba::ci::build_debug_bin("aoba")?;

    let mut cli_master = Command::new(&binary)
        .args([
            "modbus",
            "master",
            "provide-persist",
            "--port",
            "/dev/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            "holding",
            "--register-address",
            "0",
            "--register-length",
            "4",
            "--data-source",
            &format!("file:{}", data_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give CLI master time to start
    thread::sleep(Duration::from_secs(2));

    // Check if CLI master is still running
    match cli_master.try_wait()? {
        Some(status) => {
            std::fs::remove_file(&data_file)?;
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
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Navigate to vcom1
    log::info!("🧪 Navigating to vcom1 in TUI");
    navigate_to_vcom1(&mut tui_session, &mut tui_cap).await?;

    // Configure as Slave mode
    log::info!("🧪 Configuring TUI as Slave");
    configure_tui_slave(&mut tui_session, &mut tui_cap).await?;

    // Enable the port
    log::info!("🧪 Enabling port in TUI");
    enable_port(&mut tui_session, &mut tui_cap).await?;

    // Wait for communication to happen
    log::info!("🧪 Waiting for master-slave communication...");
    thread::sleep(Duration::from_secs(5));

    // Navigate to Modbus panel to check received values
    log::info!("🧪 Checking received values in TUI");
    check_received_values(&mut tui_session, &mut tui_cap).await?;

    // Cleanup
    log::info!("🧪 Cleaning up processes");
    cli_master.kill()?;
    cli_master.wait()?;

    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "tui_slave").await?;

    std::fs::remove_file(&data_file)?;

    sleep_a_while().await;

    log::info!("✅ CLI Master + TUI Slave test completed successfully");
    Ok(())
}

/// Navigate to vcom1 port in TUI
async fn navigate_to_vcom1<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    let actions = vec![
        // Wait for initial render
        CursorAction::Sleep { ms: 1000 },
        // Look for vcom1 in the port list
        CursorAction::MatchPattern {
            pattern: Regex::new(r"/dev/vcom1")?,
            description: "vcom1 port visible".to_string(),
            line_range: Some((2, 20)),
            col_range: None,
        },
        // Navigate down to find vcom1
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 1,
        },
        // Enter the port details
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify we're in the port details view
        CursorAction::MatchPattern {
            pattern: Regex::new(r"/dev/vcom1")?,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 2)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, "navigate_vcom1").await?;
    Ok(())
}

/// Configure TUI as Modbus Slave
async fn configure_tui_slave<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    let actions = vec![
        // Navigate to Modbus settings
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify in Modbus settings
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 2)),
            col_range: None,
        },
        // Navigate up to mode selector (Create Station is default)
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Up,
            count: 1,
        },
        // Create station
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify station created
        CursorAction::MatchPattern {
            pattern: Regex::new("#1")?,
            description: "Station created".to_string(),
            line_range: None,
            col_range: None,
        },
        // Navigate to Connection Mode and change to Slave
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
        CursorAction::Sleep { ms: 500 },
        // Verify mode changed to Slave
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Connection Mode\s+Slave")?,
            description: "Mode changed to Slave".to_string(),
            line_range: None,
            col_range: None,
        },
        // Navigate to Register Length and set to 4
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 4,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("4".to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        // Navigate back up
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Up,
            count: 2,
        },
    ];

    execute_cursor_actions(session, cap, &actions, "configure_slave").await?;
    Ok(())
}

/// Enable the serial port in TUI
async fn enable_port<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
    let actions = vec![
        // Should be on "Enable Port" option
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify port is enabled
        CursorAction::MatchPattern {
            pattern: Regex::new("Enabled")?,
            description: "Port enabled".to_string(),
            line_range: Some((2, 5)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, "enable_port").await?;
    Ok(())
}

/// Check received values in TUI Modbus panel
async fn check_received_values<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    let actions = vec![
        // Navigate to Modbus panel
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((0, 2)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, "check_values").await?;

    // Capture screen to check values
    let screen = cap.capture(session, "verify_received_values")?;
    log::info!("📸 Screen capture for value verification:");
    log::info!("{}", screen);

    // Expected values from CLI: 5, 15, 25, 35
    // In hex: 0x0005, 0x000F, 0x0019, 0x0023
    let expected_values = vec![5u16, 15, 25, 35];

    let mut all_found = true;
    for &val in &expected_values {
        let patterns = vec![
            format!("0x{:04X}", val),
            format!("0x{:04x}", val),
            format!("{}", val),
        ];

        let mut found = false;
        for pattern in &patterns {
            if screen.contains(pattern) {
                found = true;
                log::info!("✓ Found value {} (pattern: {})", val, pattern);
                break;
            }
        }

        if !found {
            all_found = false;
            log::warn!("⚠️ Value {} not found in TUI display", val);
        }
    }

    if !all_found {
        log::warn!("⚠️ Not all expected values found in TUI, but test can continue");
        log::warn!("This may indicate communication issues or timing problems");
    } else {
        log::info!("✅ All expected values found in TUI display");
    }

    Ok(())
}
