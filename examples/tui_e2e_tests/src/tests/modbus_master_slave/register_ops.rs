// Register operations and verification logic using auto_cursor framework

use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;

use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
use aoba::ci::TerminalCapture;

/// Verify that slave registers match the expected values from master
/// This checks that all 12 registers on the slave side show values: 0, 11, 22, 33, ..., 110
pub async fn verify_slave_registers<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Verifying {session_name} registers match master values");

    // Launch serial port
    let actions = vec![
        // Leave the register editing mode
        CursorAction::PressEnter, // Enter on "Enable Port"
        CursorAction::Sleep { ms: 500 },
        CursorAction::MatchPattern {
            pattern: Regex::new("Enabled")?,
            description: "Port enabled".to_string(),
            line_range: Some((2, 2)),
            col_range: None,
        },
        // Navigate to Modbus Panel
        CursorAction::PressArrow {
            direction: aoba::ci::ArrowKey::Down,
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

    // Capture the screen to check register values
    let screen = cap.capture(session, &format!("{session_name} - register verification"))?;

    log::info!("📸 Captured screen for verification:");
    log::info!("{screen}");

    // Expected register values: 0, 11, 22, 33, 44, 55, 66, 77, 88, 99, 110
    let expected_values = (0..12).map(|i| i * 11).collect::<Vec<u16>>();

    log::info!("🔍 Expected register values: {expected_values:?}");

    // Check each register value
    // The UI typically displays register values in hexadecimal format
    // We'll look for patterns like "0x0000", "0x000B", "0x0016", etc.
    let mut all_matched = true;
    let mut missing_values = Vec::new();

    for (index, &expected) in expected_values.iter().enumerate() {
        let hex_patterns = vec![
            format!("0x{:04X}", expected), // uppercase hex: 0x000B
            format!("0x{:04x}", expected), // lowercase hex: 0x000b
            format!("{:04X}", expected),   // no prefix uppercase: 000B
            format!("{:04x}", expected),   // no prefix lowercase: 000b
            format!("{}", expected),       // decimal: 11
        ];

        let mut found = false;
        for pattern in &hex_patterns {
            if screen.contains(pattern) {
                found = true;
                log::info!("✓ Register {index} (expected {expected}): Found pattern '{pattern}'");
                break;
            }
        }

        if !found {
            all_matched = false;
            missing_values.push((index, expected));
            log::warn!("✗ Register {index} (expected {expected}): NOT FOUND in screen output");
        }
    }

    if !all_matched {
        log::error!("❌ Verification FAILED: Some register values not found on slave");
        log::error!("Missing registers: {missing_values:?}");
        log::error!("This is EXPECTED on first run - master-slave communication needs to be fixed");
        return Err(anyhow!(
            "Slave registers do not match master values. Missing {} register(s): {:?}",
            missing_values.len(),
            missing_values
        ));
    }

    log::info!("✅ All register values verified successfully on {session_name}!");
    Ok(())
}
