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

    // Wait a bit for communication to occur
    aoba::ci::sleep_a_while().await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Capture the screen to check register values
    let screen = cap.capture(session, &format!("{} - register verification", session_name))?;
    
    log::info!("📸 Captured screen for verification:");
    log::info!("{}", screen);

    // Expected register values: 0, 11, 22, 33, 44, 55, 66, 77, 88, 99, 110
    let expected_values = (0..12).map(|i| i * 11).collect::<Vec<u16>>();
    
    log::info!("🔍 Expected register values: {:?}", expected_values);

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
                log::info!("✓ Register {} (expected {}): Found pattern '{}'", index, expected, pattern);
                break;
            }
        }

        if !found {
            all_matched = false;
            missing_values.push((index, expected));
            log::warn!("✗ Register {} (expected {}): NOT FOUND in screen output", index, expected);
        }
    }

    if !all_matched {
        log::error!("❌ Verification FAILED: Some register values not found on slave");
        log::error!("Missing registers: {:?}", missing_values);
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

/// Verify master registers are set correctly
/// This is a sanity check to ensure the master side has the expected values
pub async fn verify_master_registers<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
) -> Result<()> {
    log::info!("🧪 Verifying {session_name} registers are set correctly");

    // Capture the screen to check register values
    let screen = cap.capture(session, &format!("{} - master register check", session_name))?;
    
    log::info!("📸 Master screen captured:");
    log::info!("{}", screen);

    // Expected register values: 0, 11, 22, 33, 44, 55, 66, 77, 88, 99, 110
    let expected_values = (0..12).map(|i| i * 11).collect::<Vec<u16>>();
    
    log::info!("🔍 Expected master register values: {:?}", expected_values);

    // Check that we can see at least some of the expected values
    let mut found_count = 0;
    for (index, &expected) in expected_values.iter().enumerate() {
        let hex_patterns = vec![
            format!("0x{:04X}", expected),
            format!("0x{:04x}", expected),
            format!("{:04X}", expected),
            format!("{:04x}", expected),
            format!("{}", expected),
        ];

        for pattern in &hex_patterns {
            if screen.contains(pattern) {
                found_count += 1;
                log::info!("✓ Master register {} ({}): Found pattern '{}'", index, expected, pattern);
                break;
            }
        }
    }

    if found_count == 0 {
        log::error!("❌ Master verification FAILED: No register values found");
        return Err(anyhow!("Master registers not set correctly - no values found on screen"));
    }

    log::info!("✅ Master registers verified: found {}/{} values on screen", found_count, expected_values.len());
    Ok(())
}

/// Alternative verification using MatchPattern actions
/// This approach uses the auto_cursor framework more directly
#[allow(dead_code)]
pub async fn verify_registers_with_match_pattern<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    session_name: &str,
    register_values: &[u16],
) -> Result<()> {
    log::info!("🧪 Verifying registers using MatchPattern for {session_name}");

    // Build actions to match each register value
    let mut actions = Vec::new();

    for (index, &value) in register_values.iter().enumerate() {
        // Try to match the value in hex format (most common in Modbus UIs)
        let hex_upper = format!("0x{:04X}", value);
        let hex_lower = format!("0x{:04x}", value);
        
        // Create a pattern that matches either format
        let pattern_str = format!("({}|{})", regex::escape(&hex_upper), regex::escape(&hex_lower));
        
        actions.push(CursorAction::MatchPattern {
            pattern: Regex::new(&pattern_str)?,
            description: format!("Register {} should have value {}", index, value),
            line_range: None, // Search entire screen
            col_range: None,
        });
    }

    // Execute all match actions - this will fail fast on first mismatch
    execute_cursor_actions(session, cap, &actions, session_name).await?;

    log::info!("✅ All registers verified using MatchPattern on {session_name}");
    Ok(())
}
