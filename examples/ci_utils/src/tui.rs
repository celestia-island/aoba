use anyhow::{anyhow, Result};
use regex::Regex;

use expectrl::Expect;

/// Navigate to port1 in TUI (shared helper).
pub async fn navigate_to_vcom<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    let ports = crate::ports::vcom_matchers();
    log::info!(
        "navigate_to_vcom: port1 aliases = {:?}",
        ports.port1_aliases
    );
    let mut detected_port_name = ports.port1_name.clone();

    // Capture screen until the target port shows up. TUI scans ports lazily on start,
    // so the very first frame might not include `/tmp/vcom1` yet. Allow a few refresh
    // cycles before giving up to make the helper more resilient on slower environments.
    let mut screen = String::new();
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 1..=MAX_ATTEMPTS {
        let desc = format!("before_navigation_attempt_{attempt}");
        screen = cap.capture(session, &desc).await?;

        if let Some(alias) = ports
            .port1_aliases
            .iter()
            .find(|candidate| screen.contains(candidate.as_str()))
        {
            detected_port_name = alias.clone();
            if attempt > 1 {
                log::info!("navigate_to_vcom: port {alias} detected after {attempt} attempts");
            }
            break;
        }

        if attempt == MAX_ATTEMPTS {
            return Err(anyhow!(
                "Port ({}) not found in port list after {MAX_ATTEMPTS} attempts",
                ports.port1_name
            ));
        }

        log::info!(
            "navigate_to_vcom: port {} not visible yet (attempt {attempt}/{MAX_ATTEMPTS}); waiting for next frame",
            ports.port1_name
        );

        crate::helpers::sleep_a_while().await;
    }

    if detected_port_name != ports.port1_name {
        log::info!(
            "navigate_to_vcom: using resolved alias {detected_port_name} for target {}",
            ports.port1_name
        );
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut port_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if ports
            .port1_aliases
            .iter()
            .any(|candidate| line.contains(candidate))
        {
            port_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("\u{2502} > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let port_idx =
        port_line.ok_or_else(|| anyhow!("Could not find {detected_port_name} line index"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    if port_idx != curr_idx {
        let delta = port_idx.abs_diff(curr_idx);
        let direction = if port_idx > curr_idx {
            crate::key_input::ArrowKey::Down
        } else {
            crate::key_input::ArrowKey::Up
        };

        let actions = vec![
            crate::auto_cursor::CursorAction::PressArrow {
                direction,
                count: delta,
            },
            crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        ];
        crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_port").await?;
    }

    // Press Enter to enter port details
    let port_pattern_regex = ports.port1_rx.clone();

    // Retry action: if we entered wrong port, press Escape and try to navigate again
    let retry_action = Some(vec![
        crate::auto_cursor::CursorAction::PressEscape,
        crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 20, // Go all the way up
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ]);

    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: port_pattern_regex,
            description: format!("In {detected_port_name} port details"),
            line_range: Some((0, 3)),
            col_range: None,
            retry_action,
        },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "enter_port").await?;

    Ok(())
}

/// Enable the serial port in TUI - carefully. Reusable across examples.
pub async fn enable_port_carefully<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    log::info!("=== enable_port_carefully: START ===");
    let screen = cap.capture(session, "before_enable").await?;

    if !screen.contains("Enable Port") {
        return Err(anyhow!(
            "Not in port details page - 'Enable Port' not found"
        ));
    }

    // Check if port is already enabled - look for "Enable Port           Enabled"
    let already_enabled = screen
        .lines()
        .any(|line| line.contains("Enable Port") && line.contains("Enabled"));

    if already_enabled {
        log::info!("Port is already enabled, skipping toggle");
        return Ok(());
    }

    // Instead of trying to detect cursor position, always press Up a lot to ensure
    // we're at the top, then Down to "Enable Port" (which should be first item)
    log::info!("Aligning cursor to Enable Port using captured screen context");
    let lines: Vec<&str> = screen.lines().collect();
    let enable_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| line.contains("Enable Port").then_some(idx))
        .ok_or_else(|| anyhow!("Unexpected: unable to locate 'Enable Port' line"))?;

    let cursor_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| {
            if let Some(pos) = line.find("> ") {
                if pos <= 4 {
                    return Some(idx);
                }
            }
            None
        })
        .unwrap_or(enable_idx);

    if enable_idx != cursor_idx {
        let delta = enable_idx.abs_diff(cursor_idx);
        let direction = if enable_idx > cursor_idx {
            crate::key_input::ArrowKey::Down
        } else {
            crate::key_input::ArrowKey::Up
        };
        log::info!(
            "Moving cursor from line {cursor_idx} to {enable_idx} using {direction:?} x{delta}"
        );
        let actions = vec![
            crate::auto_cursor::CursorAction::PressArrow {
                direction,
                count: delta,
            },
            crate::auto_cursor::CursorAction::Sleep { ms: 200 },
        ];
        crate::auto_cursor::execute_cursor_actions(
            session,
            cap,
            &actions,
            "align_enable_port_move",
        )
        .await?;
    } else {
        log::info!("Cursor already on Enable Port");
    }

    let enable_port_selected_regex = Regex::new(r">\s*Enable Port")?;
    let line_start = enable_idx.saturating_sub(1);
    let line_end = (enable_idx + 1).min(lines.len().saturating_sub(1));
    let actions = vec![crate::auto_cursor::CursorAction::MatchPattern {
        pattern: enable_port_selected_regex,
        description: "Enable Port option focused".to_string(),
        line_range: Some((line_start, line_end)),
        col_range: None,
        retry_action: None, // Already aligned, no retry needed
    }];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "align_enable_port_verify")
        .await?;

    log::info!("↩️ Pressing Enter to toggle port enable");
    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 2000 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "toggle_enable_port")
        .await?;

    // Give UI extra time to process port enable and re-render
    log::info!("Waiting for UI to update port status");
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await;

    // Verify that the UI now shows the port as enabled to catch navigation drift early.
    // Use a retry loop to wait for UI to update
    log::info!("Verifying port enabled status with retry logic");
    let mut verified = false;
    for attempt in 1..=5 {
        let screen = cap
            .capture(session, &format!("verify_port_toggle_attempt_{attempt}"))
            .await
            .map_err(|err| anyhow!("Failed to capture screen after enabling port: {err}"))?;

        if screen.contains("Enable Port") && screen.contains("Enabled") {
            log::info!("✅ Port enabled status verified on attempt {attempt}");
            verified = true;
            break;
        }

        if attempt < 5 {
            log::info!("Port not shown as enabled yet, waiting (attempt {attempt}/5)");
            crate::helpers::sleep_a_while().await;
        }
    }

    if !verified {
        let final_screen = cap.capture(session, "verify_port_toggle_failed").await?;
        return Err(anyhow!(
            "Port toggle did not reflect as enabled after 5 attempts; latest screen:\n{final_screen}"
        ));
    }

    Ok(())
}

/// Enter the Modbus configuration panel from port details page
pub async fn enter_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    // If we're already inside the Modbus panel, skip navigation to avoid bouncing back to the
    // port overview and keep the register table in view for monitoring.
    let screen = cap.capture(session, "check_modbus_panel").await?;
    if screen.contains("ModBus Master/Slave Settings") {
        log::info!("Already in Modbus settings panel; skipping navigation");
        return Ok(());
    }

    // Verify we're in port details page (should see "Enable Port")
    // If not, we might have been kicked back to port list - need to re-enter
    if !screen.contains("Enable Port") {
        log::warn!("⚠️ Not in port details page - attempting to recover");
        log::warn!("Current screen:\n{}", screen);
        return Err(anyhow!(
            "Not in port details page when trying to enter Modbus panel. Expected 'Enable Port' in screen."
        ));
    }

    // From port details page, navigate down to "Business Configuration" and enter
    // First, capture screen to determine cursor position relative to target
    let screen = cap.capture(session, "before_modbus_nav").await?;
    let lines: Vec<&str> = screen.lines().collect();

    // Find "Enter Business Configuration" line
    let target_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| line.contains("Enter Business Configuration").then_some(idx))
        .ok_or_else(|| anyhow!("'Enter Business Configuration' not found in screen"))?;

    // Find cursor position - look for "> " at the beginning of menu items in the details panel
    // The cursor should be on lines containing options like "Enable Port", "Enter Business Configuration", etc.
    let cursor_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| {
            // Look for lines that have "> " followed by a known menu item
            // Check for "> " anywhere in the line (after │ or whitespace)
            if line.contains("> Enable Port")
                || line.contains("> Protocol Mode")
                || line.contains("> Enter Business")
                || line.contains("> Enter Log")
                || line.contains("> Baud rate")
                || line.contains("> Data bits")
                || line.contains("> Parity")
                || line.contains("> Stop bits")
            {
                // Exclude port list items (contain "COM" or "/tmp/")
                if !line.contains("COM") && !line.contains("/tmp/") && !line.contains("/dev/") {
                    return Some(idx);
                }
            }
            None
        })
        .unwrap_or(0);

    log::info!(
        "Navigating from line {} to 'Enter Business Configuration' at line {}",
        cursor_idx,
        target_idx
    );

    let delta = target_idx.abs_diff(cursor_idx);
    let direction = if target_idx > cursor_idx {
        crate::key_input::ArrowKey::Down
    } else {
        crate::key_input::ArrowKey::Up
    };

    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction,
            count: delta,
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 2000 }, // Give page time to load and stabilize rendering
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 10)), // Expanded range to catch the header even if scrolled
            col_range: None,
            retry_action: None, // Retry is handled at a higher level (setup_tui_port)
        },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "enter_modbus_panel")
        .await?;

    // Additional sleep after successful entry to allow UI to fully stabilize
    // This works around a rendering issue where the UI briefly shows incorrect state after page transition
    log::info!("✅ Entered Modbus panel, waiting for UI to fully stabilize...");
    crate::helpers::sleep_a_while().await;

    Ok(())
}

/// Update TUI registers with new values (shared implementation)
/// NOTE: Assumes you're already IN the Modbus panel at the register editing area
pub async fn update_tui_registers<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
    new_values: &[u16],
    _is_coil: bool,
) -> Result<()> {
    // The cursor should already be positioned on the newly created station after configuration.
    // The station list cursor is on the station name. We need to navigate to reach the register grid.
    // Try moving down just 1 time to enter the station details and land on the first field.
    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Down,
            count: 1, // Move down once to enter station and reach first register
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_first_register")
        .await?;

    for (i, &val) in new_values.iter().enumerate() {
        // Format as hex since TUI expects hex input for registers
        let hex_val = format!("{val:x}");
        let actions = vec![
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::TypeString(hex_val),
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::Sleep { ms: 50 },
        ];
        crate::auto_cursor::execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("update_reg_{i}"),
        )
        .await?;

        if i < new_values.len() - 1 {
            let next_index = i + 1;
            let registers_per_row = 4;
            let next_col = next_index % registers_per_row;

            let actions = if next_col == 0 {
                vec![
                    crate::auto_cursor::CursorAction::PressArrow {
                        direction: crate::key_input::ArrowKey::Down,
                        count: 1,
                    },
                    crate::auto_cursor::CursorAction::PressArrow {
                        direction: crate::key_input::ArrowKey::Left,
                        count: registers_per_row - 1,
                    },
                ]
            } else {
                vec![crate::auto_cursor::CursorAction::PressArrow {
                    direction: crate::key_input::ArrowKey::Right,
                    count: 1,
                }]
            };

            crate::auto_cursor::execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("nav_to_reg_{next_index}"),
            )
            .await?;
        }
    }

    // Critical: Wait for all register values to be fully saved to internal storage
    // before any subsequent operations (like navigating to add another station)
    log::info!("⏱️ Waiting for all register values to be committed...");
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await; // Extra safety margin

    Ok(())
}
