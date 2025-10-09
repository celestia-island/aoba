use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;

/// Navigate to vcom1 port in TUI (shared helper).
pub async fn navigate_to_vcom<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    let screen = cap.capture(session, "before_navigation")?;
    let vcom_pattern = std::env::var("AOBATEST_PORT1").unwrap_or_else(|_| "/tmp/vcom1".to_string());

    if !screen.contains(&vcom_pattern) {
        return Err(anyhow!("vcom1 ({vcom_pattern}) not found in port list"));
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut vcom1_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(&vcom_pattern) {
            vcom1_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("\u{2502} > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let vcom1_idx = vcom1_line.ok_or_else(|| anyhow!("Could not find vcom1 line index"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    if vcom1_idx != curr_idx {
        let delta = vcom1_idx.abs_diff(curr_idx);
        let direction = if vcom1_idx > curr_idx {
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
        crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_vcom1").await?;
    }

    // Press Enter to enter vcom1 details
    let vcom_pattern_regex = Regex::new(&regex::escape(&vcom_pattern))?;
    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: vcom_pattern_regex,
            description: "In vcom1 port details".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;

    Ok(())
}

/// Enable the serial port in TUI - carefully. Reusable across examples.
pub async fn enable_port_carefully<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
    let screen = cap.capture(session, "before_enable")?;
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

    // Check if cursor is on "Enable Port" line
    let lines: Vec<&str> = screen.lines().collect();
    let mut on_enable_port = false;
    for line in lines {
        let trimmed = line.trim();
        if (trimmed.starts_with("\u{2502} > ") || trimmed.starts_with("> "))
            && line.contains("Enable Port")
        {
            on_enable_port = true;
            break;
        }
    }

    if !on_enable_port {
        let actions = vec![crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 3,
        }];
        crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_enable_port")
            .await?;
    }

    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 1500 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "toggle_enable_port")
        .await?;

    Ok(())
}

/// Enter the Modbus configuration panel from port details page
pub async fn enter_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
) -> Result<()> {
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
