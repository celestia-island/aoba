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
        // Create the station
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 2000 },
        // Ensure the station has created
        CursorAction::MatchPattern {
            pattern: Regex::new("#1")?,
            description: "Modbus entry created".to_string(),
            line_range: None,
            col_range: None,
        },
        // Navigate to `Register Length` and set it to 12
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 5,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        // Verify Register Length was set to 12
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Register Length\s+0x000C")?,
            description: "Register Length set to 12 (0x000C)".to_string(),
            line_range: None,
            col_range: None,
        },
        // Navigate to registers
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
    ];
    // Set all 12 registers to 0, 10, 20, ..., 110 (in decimal)
    // UI expects hex input, so we need to enter hex values: 0, A, 14, 1E, 28, 32, 3C, 46, 50, 5A, 64, 6E
    let actions = actions
        .into_iter()
        .chain((0..12).flat_map(|i| {
            let decimal_value = i * 10;
            let hex_string = format!("{:X}", decimal_value); // Format as uppercase hex without 0x prefix
            vec![
                CursorAction::PressEnter,
                CursorAction::TypeString(hex_string),
                CursorAction::PressEnter,
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: 1,
                },
            ]
        }))
        .collect::<Vec<_>>();

    // Verify register values row by row (4 registers per row for 80-column terminals)
    // Row 1: registers 0-3 (decimal values: 0, 10, 20, 30 -> hex input: 0, A, 14, 1E)
    let actions = actions
        .into_iter()
        .chain(vec![
            CursorAction::Sleep { ms: 500 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"0x0000.*0x000A.*0x0014.*0x001E")?,
                description: "Row 1: registers 0-3 values verified".to_string(),
                line_range: None,
                col_range: None,
            },
        ])
        .collect::<Vec<_>>();
    // Row 2: registers 4-7 (decimal values: 40, 50, 60, 70 -> hex: 28, 32, 3C, 46)
    let actions = actions
        .into_iter()
        .chain(vec![CursorAction::MatchPattern {
            pattern: Regex::new(r"0x0004.*0x0028.*0x0032.*0x003C")?,
            description: "Row 2: registers 4-7 values verified".to_string(),
            line_range: None,
            col_range: None,
        }])
        .collect::<Vec<_>>();
    // Row 3: registers 8-11 (decimal values: 80, 90, 100, 110 -> hex: 50, 5A, 64, 6E)
    let actions = actions
        .into_iter()
        .chain(vec![CursorAction::MatchPattern {
            pattern: Regex::new(r"0x0008.*0x0050.*0x005A.*0x0064")?,
            description: "Row 3: registers 8-11 values verified".to_string(),
            line_range: None,
            col_range: None,
        }])
        .collect::<Vec<_>>();

    // Leave the register editing mode
    let actions = actions
        .into_iter()
        .chain(vec![
            CursorAction::PressEscape,
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 2,
            },
        ])
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
        // Create the station
        CursorAction::PressArrow {
            direction: ArrowKey::Up,
            count: 1,
        },
        CursorAction::PressEnter, // Ensure the station has created
        CursorAction::MatchPattern {
            pattern: Regex::new("#1")?,
            description: "Modbus entry created".to_string(),
            line_range: None,
            col_range: None,
        },
        // Wait for screen to render
        CursorAction::Sleep { ms: 500 },
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
        // Verify mode changed to Slave
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Connection Mode\s+Slave")?,
            description: "Mode changed to Slave".to_string(),
            line_range: None,
            col_range: None,
        },
        CursorAction::Sleep { ms: 2000 },
        // Navigate to `Register Length` and set it to 12
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString("12".to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        // Verify Register Length was set to 12
        CursorAction::MatchPattern {
            pattern: Regex::new(r"Register Length\s+0x000C")?,
            description: "Register Length set to 12 (0x000C)".to_string(),
            line_range: None,
            col_range: None,
        },
    ];

    // Leave the register editing mode
    let actions = actions
        .into_iter()
        .chain(vec![
            CursorAction::PressEscape,
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 2,
            },
        ])
        .collect::<Vec<_>>();

    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✓ Configured {session_name} as Modbus Slave");
    Ok(())
}
