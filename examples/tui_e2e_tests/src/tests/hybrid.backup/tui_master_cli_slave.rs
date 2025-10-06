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
    ];

    execute_cursor_actions(session, cap, &actions, "verify_vcom1_visible").await?;

    // Capture current screen to determine which port we're on
    let screen = cap.capture(session, "check_current_port")?;
    log::info!("📸 Current screen:\n{}", screen);

    // Find which line vcom1 is on and which line has the cursor
    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line_index = None;
    let mut current_selection_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("/dev/vcom1") {
            vcom1_line_index = Some(idx);
            log::info!("Found vcom1 at line {}: {}", idx, line);
        }
        // Look for the selection indicator (> at start or highlighted)
        if line.trim_start().starts_with('>') || line.contains("│ > ") {
            current_selection_line = Some(idx);
            log::info!("Current selection at line {}: {}", idx, line);
        }
    }

    // Navigate to vcom1 based on relative position
    let nav_actions = if let (Some(vcom1_idx), Some(curr_idx)) = (vcom1_line_index, current_selection_line) {
        let delta = (vcom1_idx as i32) - (curr_idx as i32);
        log::info!("Need to move {} lines (vcom1 at {}, cursor at {})", delta, vcom1_idx, curr_idx);
        
        if delta > 0 {
            // Need to move down
            vec![
                CursorAction::PressArrow {
                    direction: aoba::ci::ArrowKey::Down,
                    count: delta.abs() as usize,
                },
            ]
        } else if delta < 0 {
            // Need to move up
            vec![
                CursorAction::PressArrow {
                    direction: aoba::ci::ArrowKey::Up,
                    count: delta.abs() as usize,
                },
            ]
        } else {
            // Already on vcom1, no movement needed
            log::info!("Already on vcom1");
            vec![]
        }
    } else {
        // Fallback: if we can't determine positions precisely, use a heuristic
        // Try moving up to potentially get to the first item, then search
        log::warn!("Could not determine exact positions, using fallback navigation");
        vec![
            // Move up several times to try to get to the top
            CursorAction::PressArrow {
                direction: aoba::ci::ArrowKey::Up,
                count: 5,
            },
            CursorAction::Sleep { ms: 500 },
        ]
    };

    if !nav_actions.is_empty() {
        execute_cursor_actions(session, cap, &nav_actions, "navigate_to_vcom1").await?;
        
        // After navigation, verify we can still see vcom1 and adjust if needed
        let screen_after = cap.capture(session, "check_after_nav")?;
        log::info!("📸 Screen after navigation:\n{}", screen_after);
        
        // Check if vcom1 is on the current line
        let lines_after: Vec<&str> = screen_after.lines().collect();
        let mut on_vcom1 = false;
        
        for line in &lines_after {
            if (line.trim_start().starts_with('>') || line.contains("│ > ")) && line.contains("/dev/vcom1") {
                on_vcom1 = true;
                log::info!("✓ Successfully navigated to vcom1");
                break;
            }
        }
        
        // If not on vcom1 yet, try moving down to find it
        if !on_vcom1 {
            log::info!("Not on vcom1 yet, searching...");
            for _ in 0..5 {
                let screen_search = cap.capture(session, "search_vcom1")?;
                let search_lines: Vec<&str> = screen_search.lines().collect();
                
                let mut found = false;
                for line in &search_lines {
                    if (line.trim_start().starts_with('>') || line.contains("│ > ")) && line.contains("/dev/vcom1") {
                        found = true;
                        log::info!("✓ Found vcom1 on current line");
                        break;
                    }
                }
                
                if found {
                    break;
                }
                
                // Move down one and try again
                execute_cursor_actions(session, cap, &vec![
                    CursorAction::PressArrow {
                        direction: aoba::ci::ArrowKey::Down,
                        count: 1,
                    },
                ], "search_down").await?;
            }
        }
    }

    // Enter the port details
    let enter_actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 },
        // Verify we're in the port details view for vcom1
        CursorAction::MatchPattern {
            pattern: Regex::new(r"/dev/vcom1")?,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 2)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &enter_actions, "enter_vcom1").await?;
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
