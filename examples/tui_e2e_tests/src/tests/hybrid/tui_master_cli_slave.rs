// Test TUI Master (Slave/Server) with CLI Slave (Master/Client)
// This uses TUI to set up a Modbus Master that responds to requests,
// and CLI to poll it for data

use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture};

/// Test TUI Master with CLI Slave
/// TUI acts as Modbus Master (Slave/Server) responding to requests
/// CLI acts as Modbus Slave (Master/Client) polling for data
pub async fn test_tui_master_with_cli_slave() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master + CLI Slave test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave hybrid test");

    // Spawn TUI process (will be master on vcom1)
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {}", err))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Navigate to vcom1
    log::info!("🧪 Navigating to vcom1 in TUI");
    navigate_to_vcom1(&mut tui_session, &mut tui_cap).await?;

    // Configure as Master mode with test data
    log::info!("🧪 Configuring TUI as Master with test values");
    configure_tui_master(&mut tui_session, &mut tui_cap).await?;

    // Enable the port
    log::info!("🧪 Enabling port in TUI");
    enable_port(&mut tui_session, &mut tui_cap).await?;

    // Give TUI time to fully initialize the port
    sleep_a_while().await;
    thread::sleep(Duration::from_secs(2));

    // Now use CLI to poll the TUI master
    log::info!("🧪 Starting CLI slave to poll TUI master");
    let cli_result = run_cli_slave_poll().await?;

    // Verify the CLI got the expected values
    verify_cli_output(&cli_result)?;

    // Cleanup: quit TUI
    log::info!("🧪 Cleaning up TUI process");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "tui_master").await?;

    sleep_a_while().await;

    log::info!("✅ TUI Master + CLI Slave test completed successfully");
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
        // Navigate down to find vcom1 (it should be near the top)
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

/// Configure TUI as Modbus Master with test values
async fn configure_tui_master<T: Expect>(
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
        // Navigate to Register Length and set to 4
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 5,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("4".to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        // Navigate to register values
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
            count: 1,
        },
    ];

    // Set 4 register values: 0, 10, 20, 30 (entered as hex: 0, A, 14, 1E)
    let register_values = vec![0u16, 10, 20, 30];
    let actions = actions
        .into_iter()
        .chain(register_values.iter().flat_map(|&val| {
            vec![
                CursorAction::PressEnter,
                CursorAction::TypeString(format!("{:X}", val)),
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: aoba::ci::ArrowKey::Right,
                    count: 1,
                },
            ]
        }))
        .chain(vec![
            // Exit register editing
            CursorAction::PressEscape,
            // Navigate back up
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Up,
                count: 2,
            },
        ])
        .collect::<Vec<_>>();

    execute_cursor_actions(session, cap, &actions, "configure_master").await?;
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

/// Run CLI slave poll command
async fn run_cli_slave_poll() -> Result<String> {
    let binary = aoba::ci::build_debug_bin("aoba")?;

    log::info!("🧪 Executing CLI command: modbus slave poll");

    let output = Command::new(&binary)
        .args([
            "modbus",
            "slave",
            "poll",
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
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::info!("🧪 CLI exit status: {}", output.status);
    log::info!("🧪 CLI stdout: {}", stdout);
    if !stderr.is_empty() {
        log::info!("🧪 CLI stderr: {}", stderr);
    }

    if !output.status.success() {
        return Err(anyhow!(
            "CLI command failed with status {}: {}",
            output.status,
            stderr
        ));
    }

    Ok(stdout)
}

/// Verify CLI output contains expected register values
fn verify_cli_output(output: &str) -> Result<()> {
    log::info!("🧪 Verifying CLI output contains expected values");

    // Expected values in decimal: 0, 10, 20, 30
    // These might appear in hex (0x0000, 0x000A, 0x0014, 0x001E) or decimal
    let expected_values = vec![0u16, 10, 20, 30];

    let mut all_found = true;
    for &val in &expected_values {
        // Check for various formats
        let patterns = vec![
            format!("0x{:04X}", val),  // 0x000A
            format!("0x{:04x}", val),  // 0x000a
            format!("{}", val),        // 10
        ];

        let mut found = false;
        for pattern in &patterns {
            if output.contains(pattern) {
                found = true;
                log::info!("✓ Found value {} (pattern: {})", val, pattern);
                break;
            }
        }

        if !found {
            all_found = false;
            log::error!("✗ Value {} not found in CLI output", val);
        }
    }

    if !all_found {
        return Err(anyhow!(
            "CLI output does not contain all expected register values"
        ));
    }

    log::info!("✅ All expected values verified in CLI output");
    Ok(())
}
