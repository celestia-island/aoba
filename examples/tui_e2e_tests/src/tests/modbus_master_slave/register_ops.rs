// Register operations using auto_cursor framework

use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::{ArrowKey, TerminalCapture};

/// Set a magic number on the master's register
pub async fn set_magic_number<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
    magic_number: u16,
) -> Result<()> {
    log::info!("🧪 Setting magic number 0x{magic_number:04X} on {session_name} register");

    // Convert magic number to hex string
    let hex_string = format!("{magic_number:04X}");

    let actions = vec![
        // Navigate to the register value field
        // Assuming current focus is on Mode row, need to navigate to register area
        CursorAction::Sleep { ms: 200 },
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5, // Navigate to register value field
        },
        CursorAction::Sleep { ms: 200 },
        // Enter edit mode on register value
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
        // Type magic number (hex format)
        CursorAction::TypeString(hex_string.clone()),
        CursorAction::Sleep { ms: 200 },
        // Confirm the value
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        // Verify the value is displayed
        CursorAction::MatchPattern {
            pattern: Regex::new(&hex_string)?,
            description: format!("Magic number 0x{hex_string} displayed"),
            line_range: Some((2, 20)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Set magic number 0x{magic_number:04X} on {session_name}");
    Ok(())
}

/// Verify that the magic number appears on the slave's display
pub async fn verify_magic_number<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
    magic_number: u16,
) -> Result<()> {
    log::info!("🧪 Verifying magic number 0x{magic_number:04X} on {session_name}");

    // Convert magic number to hex string (try multiple formats)
    let hex_upper = format!("{magic_number:04X}");
    let hex_lower = format!("{magic_number:04x}");
    let hex_with_prefix = format!("0x{magic_number:04X}");

    // Wait for communication to occur
    let wait_actions = vec![CursorAction::Sleep { ms: 2000 }];
    execute_cursor_actions(session, cap, &wait_actions, session_name).await?;

    // Try to match any format of the magic number
    let pattern_str = format!("({hex_upper}|{hex_lower}|{hex_with_prefix})");
    let actions = vec![CursorAction::MatchPattern {
        pattern: Regex::new(&pattern_str)?,
        description: format!("Magic number 0x{hex_upper} visible on slave"),
        line_range: Some((2, 20)),
        col_range: None,
    }];

    match execute_cursor_actions(session, cap, &actions, session_name).await {
        Ok(_) => {
            log::info!("✅ SUCCESS: {session_name} correctly displays the magic number 0x{magic_number:04X}!");
            Ok(())
        }
        Err(e) => {
            log::warn!("⚠️  {session_name} does not show 0x{magic_number:04X} yet - communication may need fixing");
            log::warn!("This is expected on first run - the test will help identify what needs to be fixed");
            log::warn!("Error: {e}");
            // Return error to fail the test as expected
            Err(anyhow!(
                "Magic number 0x{magic_number:04X} not found on {session_name} display"
            ))
        }
    }
}
