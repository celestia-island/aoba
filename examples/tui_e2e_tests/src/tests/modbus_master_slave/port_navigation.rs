// Port navigation logic using auto_cursor framework

use anyhow::Result;
use expectrl::Expect;
use regex::Regex;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{ArrowKey, TerminalCapture};

/// Navigate to vcom1 (first port in list) - just press Enter
pub async fn navigate_to_vcom1<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Navigating to vcom1 (first port) in {session_name}");

    // Give the TUI a moment to fully render before navigating
    let actions = vec![
        // Verify vcom1 appears on screen
        CursorAction::MatchPattern {
            pattern: Regex::new("vcom1")?,
            description: "vcom1 port visible".to_string(),
            line_range: Some((2, 20)), // Search in main content area
            col_range: None,
        },
        // vcom1 should be the first item (cursor already there), just press Enter
        CursorAction::PressEnter,
        // Navigate into ConfigPanel
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("/dev/vcom1 > ModBus Master/Slave Settings")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((0, 0)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Navigated to vcom1 in {session_name}");
    Ok(())
}

/// Navigate to vcom2 (second port in list) - press Down once then Enter
pub async fn navigate_to_vcom2<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Navigating to vcom2 (second port) in {session_name}");

    // Give the TUI a moment to fully render before navigating
    let actions = vec![
        CursorAction::Sleep { ms: 500 },
        // Verify vcom2 appears on screen
        CursorAction::MatchPattern {
            pattern: Regex::new("vcom2")?,
            description: "vcom2 port visible".to_string(),
            line_range: Some((2, 20)), // Search in main content area
            col_range: None,
        },
        // vcom2 should be the second item, press Down once then Enter
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::PressEnter,
        // Navigate into ConfigPanel
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        // Verify we're in Modbus panel
        CursorAction::MatchPattern {
            pattern: Regex::new("/dev/vcom2 > ModBus Master/Slave Settings")?,
            description: "In Modbus panel".to_string(),
            line_range: Some((0, 0)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Navigated to vcom2 in {session_name}");
    Ok(())
}
