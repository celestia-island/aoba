use anyhow::{anyhow, Result};
use regex::Regex;

use expectrl::Expect;

/// Navigate to port1 in TUI (shared helper).
pub async fn navigate_to_vcom<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    let ports = crate::ports::vcom_matchers();
    let port_name = &ports.port1_name;

    // Capture screen until the target port shows up. TUI scans ports lazily on start,
    // so the very first frame might not include `/tmp/vcom1` yet. Allow a few refresh
    // cycles before giving up to make the helper more resilient on slower environments.
    let mut screen = String::new();
    const MAX_ATTEMPTS: usize = 6;
    for attempt in 1..=MAX_ATTEMPTS {
        let desc = format!("before_navigation_attempt_{attempt}");
        screen = cap.capture(session, &desc).await?;

        if screen.contains(port_name) {
            if attempt > 1 {
                log::info!("navigate_to_vcom: port {port_name} detected after {attempt} attempts");
            }
            break;
        }

        if attempt == MAX_ATTEMPTS {
            return Err(anyhow!(
                "Port ({port_name}) not found in port list after {MAX_ATTEMPTS} attempts"
            ));
        }

        log::info!(
            "navigate_to_vcom: port {port_name} not visible yet (attempt {attempt}/{MAX_ATTEMPTS}); waiting for next frame"
        );
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut port_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(port_name) {
            port_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("\u{2502} > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let port_idx = port_line.ok_or_else(|| anyhow!("Could not find {port_name} line index"))?;
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
    let port_pattern_regex = Regex::new(&regex::escape(port_name))?;
    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: port_pattern_regex,
            description: format!("In {port_name} port details"),
            line_range: Some((0, 3)),
            col_range: None,
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
    }];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "align_enable_port_verify")
        .await?;

    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 1500 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "toggle_enable_port")
        .await?;

    // Verify that the UI now shows the port as enabled to catch navigation drift early.
    let screen = cap
        .capture(session, "verify_port_toggle")
        .await
        .map_err(|err| anyhow!("Failed to capture screen after enabling port: {err}"))?;
    if !screen.contains("Enable Port") || !screen.contains("Enabled") {
        return Err(anyhow!(
            "Port toggle did not reflect as enabled; latest screen:\n{screen}"
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

    // From port details page, navigate down to "Business Configuration" and enter
    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Down,
            count: 2,
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "enter_modbus_panel")
        .await?;
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
    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 10,
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Down,
            count: 6,
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_first_register")
        .await?;

    for (i, &val) in new_values.iter().enumerate() {
        let dec_val = format!("{val}");
        let actions = vec![
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::TypeString(dec_val),
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        ];
        crate::auto_cursor::execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("update_reg_{i}"),
        )
        .await?;

        if i < new_values.len() - 1 {
            let actions = vec![crate::auto_cursor::CursorAction::PressArrow {
                direction: crate::key_input::ArrowKey::Right,
                count: 1,
            }];
            crate::auto_cursor::execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("nav_to_reg_{}", i + 1),
            )
            .await?;
        }
    }

    Ok(())
}
