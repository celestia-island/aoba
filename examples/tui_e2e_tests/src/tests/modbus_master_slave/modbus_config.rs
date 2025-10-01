// Modbus configuration logic using auto_cursor framework

use anyhow::Result;
use expectrl::Expect;
use regex::Regex;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{ArrowKey, TerminalCapture};

/// Configure the session as a Modbus Master
pub async fn configure_master_mode<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Configuring {session_name} as Modbus Master");

    let actions = vec![
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("/dev/vcom1 > ModBus Master/Slave Settings")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((0, 0)),
            col_range: None,
        },
        // Add a new modbus entry
        CursorAction::PressEnter, // Enter on "Add Master/Slave"
        // Navigate to `Register Length` and set it to 12
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()),
        CursorAction::PressEnter,
        // Navigate to registers
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    // Set all 12 registers to 0, 11, 22, ..., 110
    let actions = actions
        .into_iter()
        .chain((0..12).flat_map(|i| {
            vec![
                CursorAction::PressEnter,
                CursorAction::TypeString(format!("{}", i * 11)),
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: 1,
                },
            ]
        }))
        .collect::<Vec<_>>();

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Configured {session_name} as Modbus Master");
    Ok(())
}

/// Configure the session as a Modbus Slave
pub async fn configure_slave_mode<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Configuring {session_name} as Modbus Slave");

    let actions = vec![
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("/dev/vcom2 > ModBus Master/Slave Settings")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((0, 0)),
            col_range: None,
        },
        // Add a new modbus entry
        CursorAction::PressEnter, // Enter on "Add Master/Slave"
        // Change Mode to Slave
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1, // Navigate to Mode
        },
        CursorAction::PressEnter,
        CursorAction::PressArrow {
            direction: ArrowKey::Right,
            count: 1, // Select Slave
        },
        CursorAction::PressEnter,
        // Navigate to `Mode` and set it to Slave
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::PressEnter,
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1, // Select Slave
        },
        CursorAction::PressEnter,
        // Navigate to `Register Length` and set it to 12
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()),
        CursorAction::PressEnter,
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Configured {session_name} as Modbus Slave");
    Ok(())
}
