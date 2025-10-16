// Utility functions for TUI E2E tests
use anyhow::{anyhow, Result};
use std::time::Duration;

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    key_input::{ArrowKey, ExpectKeyExt},
    snapshot::TerminalCapture,
    tui::{enable_port_carefully, enter_modbus_panel},
};

/// Common cursor navigation and configuration utilities for TUI E2E tests
/// Navigate to a specific port in the TUI
pub async fn navigate_to_port<T: Expect>(
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
            "Port {target_port} not found in port list after {MAX_ATTEMPTS} attempts. Available: {available}",
            available = screen
                .lines()
                .filter(|l| l.contains("/tmp/") || l.contains("/dev/"))
                .take(10)
                .collect::<Vec<_>>()
                .join(", ")
            ));
        }

        log::info!("⏳ Port {target_port} not visible yet (attempt {attempt}/{MAX_ATTEMPTS})");
        tokio::time::sleep(Duration::from_secs(1)).await;
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

    let port_idx =
        port_line.ok_or_else(|| anyhow!("Could not find {target_port} line index in screen"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    log::info!("📍 Port at line {port_idx}, cursor at line {curr_idx}");

    // Check if already on the target port
    if port_idx == curr_idx {
        log::info!("✅ Cursor already on {target_port}, pressing Enter");
        session.send_enter()?;
        tokio::time::sleep(Duration::from_secs(1)).await;
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
    tokio::time::sleep(Duration::from_secs(1)).await;

    log::info!("✅ Successfully navigated to and selected {target_port}");
    Ok(())
}

/// Configure a TUI process as a Modbus Master with common settings
pub async fn configure_tui_master_common<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_type: u8,
    register_mode: &str,
    register_length: usize,
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Master (Station {station_id}, Type {register_type:02})");

    // Verify we are inside Modbus panel
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
    log::info!("📝 Setting register type to {register_type:02} ({register_mode})");

    // First, capture the current screen to understand the layout
    let screen = cap
        .capture(
            session,
            &format!("before_set_register_type_{register_type}"),
        )
        .await?;

    // Check if we're in a multi-station configuration
    let is_multi_station = screen.lines().filter(|l| l.contains("#")).count() > 1;

    if is_multi_station {
        log::info!("🔍 Multi-station configuration detected, using precise navigation");

        // For multi-station, navigate more carefully to the register type field
        // First, ensure we're at the top of the current station section
        let actions = vec![
            // Navigate to top of the current station section
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 20,
            },
            CursorAction::Sleep { ms: 300 },
            // Navigate down to the current station's Register Type field
            // In multi-station mode, each station has its own Register Type field
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 3,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            // Navigate through register types to reach target type
            // Register types: 01=Coils, 02=Discrete Inputs, 03=Holding, 04=Input
            // We need to navigate from current position to target type
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: register_type as usize, // Direct mapping: 1=Coils, 2=Discrete, 3=Holding, 4=Input
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
            // Verify the register type was set correctly
            CursorAction::MatchPattern {
                pattern: regex::Regex::new(&format!("Register Type.*{register_type:02}"))?,
                description: format!("Register type set to {register_type:02}"),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_multi"),
        )
        .await?;
    } else {
        log::info!("🔍 Single station configuration, using standard navigation");

        // For single station, use standard navigation
        // The issue: when we enter register type selection, it defaults to Holding(03)
        // But we want Holding(03). We don't need to navigate, just press Enter to confirm.
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
            // When we enter register type selection, it defaults to Holding(03)
            // Just press Enter to confirm the selection
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
            // Verify the register type was set correctly
            CursorAction::MatchPattern {
                pattern: regex::Regex::new(&format!("Register Type.*{register_type:02}"))?,
                description: format!("Register type set to {register_type:02}"),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_single"),
        )
        .await?;
    }

    // Set Register Length
    log::info!("📝 Setting register length to {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(register_length.to_string()),
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

/// Configure a TUI process as a Modbus Slave with common settings
pub async fn configure_tui_slave_common<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_type: u8,
    register_mode: &str,
    register_length: usize,
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Slave (Station {station_id}, Type {register_type:02})");

    // Verify we are inside Modbus panel
    let screen = cap
        .capture(session, &format!("verify_modbus_panel_slave{station_id}"))
        .await?;
    if !screen.contains("ModBus Master/Slave Settings") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for Slave (Station {station_id})"
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
        &format!("create_station_slave{station_id}"),
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
    log::info!("📝 Setting register type to {register_type:02} ({register_mode})");

    // First, capture the current screen to understand the layout
    let screen = cap
        .capture(
            session,
            &format!("before_set_register_type_{register_type}"),
        )
        .await?;

    // Check if we're in a multi-station configuration
    let is_multi_station = screen.lines().filter(|l| l.contains("#")).count() > 1;

    if is_multi_station {
        log::info!("🔍 Multi-station configuration detected, using precise navigation");

        // For multi-station, navigate more carefully to the register type field
        let actions = vec![
            // Navigate to top of the current station section
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 15,
            },
            CursorAction::Sleep { ms: 300 },
            // Navigate down to Register Type field (usually 3rd field in station)
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 3,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            // Debug: capture the register type selection screen
            CursorAction::DebugBreakpoint {
                description: "register_type_selection_screen".to_string(),
            },
            // Navigate through register types to reach target type
            // Register types: 01=Coils, 02=Discrete Inputs, 03=Holding, 04=Input
            // Try different navigation strategies
            CursorAction::PressArrow {
                direction: ArrowKey::Left,
                count: 5, // Go all the way left first
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: register_type as usize, // Then go right to target type
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_multi"),
        )
        .await?;
    } else {
        log::info!("🔍 Single station configuration, using standard navigation");

        // For single station, use standard navigation
        // The issue: when we enter register type selection, it defaults to Coils(01)
        // But we want Holding(03). We need to navigate from 01 to 03.
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
            // When we enter register type selection, it defaults to Coils(01)
            // We need to navigate to Holding(03) - press Right twice to go from 01 to 03
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: 2, // From Coils(01) to Discrete Inputs(02) to Holding(03)
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
            // Verify the register type was set correctly
            CursorAction::MatchPattern {
                pattern: regex::Regex::new(&format!("Register Type.*{register_type:02}"))?,
                description: format!("Register type set to {register_type:02}"),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_single"),
        )
        .await?;
    }

    // Set Register Length
    log::info!("📝 Setting register length to {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_register_length_slave{station_id}"),
    )
    .await?;

    log::info!("✅ Slave (Station {station_id}, Type {register_type:02}) configured successfully");
    Ok(())
}

/// Common setup for TUI port configuration
pub async fn setup_tui_port<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
) -> Result<()> {
    // Navigate to the target port
    log::info!("📍 Navigating to port {target_port}");
    navigate_to_port(session, cap, target_port).await?;

    // Enable the port
    log::info!("🔌 Enabling port");
    enable_port_carefully(session, cap).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Enter Modbus panel
    log::info!("⚙️ Entering Modbus configuration panel");
    enter_modbus_panel(session, cap).await?;

    Ok(())
}

/// Test a station with retry logic - common polling function
pub async fn test_station_with_retries(
    port: &str,
    station_id: u8,
    register_mode: &str,
    expected_data: &[u16],
    max_retries: usize,
    retry_interval_ms: u64,
) -> Result<bool> {
    use std::process::{Command, Stdio};

    let binary = ci_utils::terminal::build_debug_bin("aoba")?;

    for attempt in 1..=max_retries {
        log::info!(
            "  Attempt {attempt}/{max_retries}: Polling {port} for Station {station_id} ({register_mode})"
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
        if attempt < max_retries {
            tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
        }
    }

    log::error!("  ❌ FAILED: All {max_retries} attempts failed for Station {station_id}");
    Ok(false)
}
