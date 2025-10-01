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
    log::info!(
        "🧪 Setting magic number 0x{:04X} on {} register",
        magic_number,
        session_name
    );

    // Convert magic number to hex string
    let hex_string = format!("{:04X}", magic_number);

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
            description: format!("Magic number 0x{} displayed", hex_string),
            line_range: Some((2, 20)),
            col_range: None,
        },
    ];

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Set magic number 0x{:04X} on {}", magic_number, session_name);
    Ok(())
}

/// Verify that the magic number appears on the slave's display
pub async fn verify_magic_number<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
    magic_number: u16,
) -> Result<()> {
    log::info!(
        "🧪 Verifying magic number 0x{:04X} on {}",
        magic_number,
        session_name
    );

    // Convert magic number to hex string (try multiple formats)
    let hex_upper = format!("{:04X}", magic_number);
    let hex_lower = format!("{:04x}", magic_number);
    let hex_with_prefix = format!("0x{:04X}", magic_number);

    // Wait for communication to occur
    let wait_actions = vec![CursorAction::Sleep { ms: 2000 }];
    execute_cursor_actions(session, cap, &wait_actions, session_name).await?;

    // Try to match any format of the magic number
    let pattern_str = format!("({}|{}|{})", hex_upper, hex_lower, hex_with_prefix);
    let actions = vec![CursorAction::MatchPattern {
        pattern: Regex::new(&pattern_str)?,
        description: format!("Magic number 0x{} visible on slave", hex_upper),
        line_range: Some((2, 20)),
        col_range: None,
    }];

    match execute_cursor_actions(session, cap, &actions, session_name).await {
        Ok(_) => {
            log::info!(
                "✅ SUCCESS: {} correctly displays the magic number 0x{:04X}!",
                session_name,
                magic_number
            );
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "⚠️  {} does not show 0x{:04X} yet - communication may need fixing",
                session_name,
                magic_number
            );
            log::warn!("This is expected on first run - the test will help identify what needs to be fixed");
            log::warn!("Error: {}", e);
            // Return error to fail the test as expected
            Err(anyhow!(
                "Magic number 0x{:04X} not found on {} display",
                magic_number,
                session_name
            ))
        }
    }
}
