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
        // Navigate to Modbus panel from ConfigPanel
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5, // Navigate to Modbus option
        },
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("Modbus|Master|Slave")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((2, 20)),
            col_range: None,
        },
        // Add a new modbus entry
        CursorAction::PressEnter, // Enter on "Add Master/Slave"
        CursorAction::Sleep { ms: 300 },
        // Master is default mode, verify it
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1, // Down to Mode field
        },
        CursorAction::Sleep { ms: 200 },
        // Verify Master mode is displayed
        CursorAction::MatchPattern {
            pattern: Regex::new("Master")?,
            description: "Master mode selected".to_string(),
            line_range: Some((2, 20)),
            col_range: None,
        },
    ];

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
        // Navigate to Modbus panel from ConfigPanel
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5, // Navigate to Modbus option
        },
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("Modbus|Master|Slave")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((2, 20)),
            col_range: None,
        },
        // Add a new modbus entry
        CursorAction::PressEnter, // Enter on "Add Master/Slave"
        CursorAction::Sleep { ms: 300 },
        // Navigate to Mode selection and toggle to Slave
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1, // Down to Mode field
        },
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressEnter, // Enter to toggle mode
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1, // Down to select Slave
        },
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressEnter, // Confirm selection
        CursorAction::Sleep { ms: 300 },
        // Verify Slave mode is now displayed
        CursorAction::MatchPattern {
            pattern: Regex::new("Slave")?,
            description: "Slave mode selected".to_string(),
            line_range: Some((2, 20)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Configured {session_name} as Modbus Slave");
    Ok(())
}
