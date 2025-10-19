use anyhow::{anyhow, Result};
use regex::Regex;

use expectrl::Expect;

/// Check the top-right status indicator in the title bar
/// Returns Ok(status) where status is one of: "NotStarted", "Starting", "Running", "Modified", "Saving", "Syncing", "Applied"
/// This function handles the transient nature of "Applied" status (only shows for 3 seconds)
pub fn check_status_indicator(screen: &str) -> Result<String> {
    // The status indicator is in the top-right corner of the first line
    // Format: "AOBA > /tmp/vcom1 > ModBus Master/Slave Set                Running ● "
    // or:     "AOBA > /tmp/vcom1 > ModBus Master/Slave Set                Applied ✔ "

    let first_line = screen.lines().next().unwrap_or("");

    // Check for each status in priority order (most specific first)
    if Regex::new(r"Applied\s*✔")?.is_match(first_line) {
        return Ok("Applied".to_string());
    }
    if Regex::new(r"Saving\s*[⠏⠛⠹⠼⠶⠧]")?.is_match(first_line) {
        return Ok("Saving".to_string());
    }
    if Regex::new(r"Syncing\s*[⠏⠛⠹⠼⠶⠧]")?.is_match(first_line) {
        return Ok("Syncing".to_string());
    }
    if Regex::new(r"Starting\s*[⠏⠛⠹⠼⠶⠧]")?.is_match(first_line) {
        return Ok("Starting".to_string());
    }
    if Regex::new(r"Running\s*●")?.is_match(first_line) {
        return Ok("Running".to_string());
    }
    if Regex::new(r"Modified\s*○")?.is_match(first_line) {
        return Ok("Modified".to_string());
    }
    if Regex::new(r"Not Started\s*×")?.is_match(first_line) {
        return Ok("NotStarted".to_string());
    }

    Err(anyhow!(
        "No status indicator found in title bar: {}",
        first_line
    ))
}

/// Verify the port is enabled by checking the status indicator
/// This function is more flexible than just looking for "Running" or "Applied"
/// It handles the timing issue where "Applied ✔" only shows for 3 seconds,
/// then transitions to "Running ●" or "Modified ○"
pub async fn verify_port_enabled<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
    capture_name: &str,
) -> Result<String> {
    const MAX_ATTEMPTS: usize = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    for attempt in 1..=MAX_ATTEMPTS {
        let screen = cap
            .capture(session, &format!("{}_{}", capture_name, attempt))
            .await?;

        match check_status_indicator(&screen) {
            Ok(status) => match status.as_str() {
                "Applied" => {
                    log::info!(
                        "✅ Port enabled - status: Applied ✔ (will transition to Running ● in 3s)"
                    );
                    return Ok(status);
                }
                "Running" => {
                    log::info!("✅ Port enabled - status: Running ●");
                    return Ok(status);
                }
                "Modified" => {
                    log::info!(
                        "✅ Port enabled - status: Modified ○ (running with unsaved changes)"
                    );
                    return Ok(status);
                }
                "Saving" | "Syncing" => {
                    log::info!(
                        "⏳ Port status: {} (transitioning...), attempt {}/{}",
                        status,
                        attempt,
                        MAX_ATTEMPTS
                    );
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                        continue;
                    }
                    return Ok(status);
                }
                "Starting" => {
                    log::info!("⏳ Port starting, attempt {}/{}", attempt, MAX_ATTEMPTS);
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                        continue;
                    }
                    return Err(anyhow!(
                        "Port still starting after {} attempts",
                        MAX_ATTEMPTS
                    ));
                }
                "NotStarted" => {
                    log::warn!(
                        "⚠️ Port not started yet, attempt {}/{}",
                        attempt,
                        MAX_ATTEMPTS
                    );
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                        continue;
                    }
                    return Err(anyhow!("Port not started after {} attempts", MAX_ATTEMPTS));
                }
                _ => {
                    log::warn!(
                        "⚠️ Unknown status: {}, attempt {}/{}",
                        status,
                        attempt,
                        MAX_ATTEMPTS
                    );
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                        continue;
                    }
                    return Err(anyhow!("Unknown status: {}", status));
                }
            },
            Err(e) => {
                log::warn!(
                    "⚠️ Failed to check status indicator: {}, attempt {}/{}",
                    e,
                    attempt,
                    MAX_ATTEMPTS
                );
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                return Err(e);
            }
        }
    }

    Err(anyhow!(
        "Failed to verify port enabled after {} attempts",
        MAX_ATTEMPTS
    ))
}

/// Navigate to port1 in TUI (shared helper).
pub async fn navigate_to_vcom<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
    port1: &str,
) -> Result<()> {
    let ports = crate::ports::vcom_matchers_with_ports(port1, crate::ports::DEFAULT_PORT2);
    log::info!(
        "navigate_to_vcom: port1 aliases = {:?}",
        ports.port1_aliases
    );
    let mut detected_port_name = ports.port1_name.clone();

    // Capture screen until the target port shows up. TUI scans ports lazily on start,
    // so the very first frame might not include `/tmp/vcom1` yet. Allow a few refresh
    // cycles before giving up to make the helper more resilient on slower environments.
    let mut screen = String::new();
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 1..=MAX_ATTEMPTS {
        let desc = format!("before_navigation_attempt_{attempt}");
        screen = cap.capture(session, &desc).await?;

        if let Some(alias) = ports
            .port1_aliases
            .iter()
            .find(|candidate| screen.contains(candidate.as_str()))
        {
            detected_port_name = alias.clone();
            if attempt > 1 {
                log::info!("navigate_to_vcom: port {alias} detected after {attempt} attempts");
            }
            break;
        }

        if attempt == MAX_ATTEMPTS {
            return Err(anyhow!(
                "Port ({}) not found in port list after {MAX_ATTEMPTS} attempts",
                ports.port1_name
            ));
        }

        log::info!(
            "navigate_to_vcom: port {} not visible yet (attempt {attempt}/{MAX_ATTEMPTS}); waiting for next frame",
            ports.port1_name
        );

        crate::helpers::sleep_a_while().await;
    }

    if detected_port_name != ports.port1_name {
        log::info!(
            "navigate_to_vcom: using resolved alias {detected_port_name} for target {}",
            ports.port1_name
        );
    }

    let lines: Vec<&str> = screen.lines().collect();
    let mut port_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if ports
            .port1_aliases
            .iter()
            .any(|candidate| line.contains(candidate))
        {
            port_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("\u{2502} > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let port_idx =
        port_line.ok_or_else(|| anyhow!("Could not find {detected_port_name} line index"))?;
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
    let port_pattern_regex = ports.port1_rx.clone();

    // Retry action: if we entered wrong port, press Escape and try to navigate again
    let retry_action = Some(vec![
        crate::auto_cursor::CursorAction::PressEscape,
        crate::auto_cursor::CursorAction::Sleep { ms: 500 },
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 20, // Go all the way up
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ]);

    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: port_pattern_regex,
            description: format!("In {detected_port_name} port details"),
            line_range: Some((0, 3)),
            col_range: None,
            retry_action,
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
        retry_action: None, // Already aligned, no retry needed
    }];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "align_enable_port_verify")
        .await?;

    log::info!("↩️ Pressing Enter to toggle port enable");
    let actions = vec![
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 2000 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "toggle_enable_port")
        .await?;

    // Give UI extra time to process port enable and re-render
    log::info!("Waiting for UI to update port status");
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await;

    // Verify that the UI now shows the port as enabled to catch navigation drift early.
    // Use a retry loop to wait for UI to update
    log::info!("Verifying port enabled status with retry logic");
    let mut verified = false;
    for attempt in 1..=3 {
        let screen = cap
            .capture(session, &format!("verify_port_toggle_attempt_{attempt}"))
            .await
            .map_err(|err| anyhow!("Failed to capture screen after enabling port: {err}"))?;

        if screen.contains("Enable Port") && screen.contains("Enabled") {
            log::info!("✅ Port enabled status verified on attempt {attempt}");
            verified = true;
            break;
        }

        if attempt < 3 {
            log::info!("Port not shown as enabled yet, waiting (attempt {attempt}/3)");
            crate::helpers::sleep_a_while().await;
        }
    }

    if !verified {
        let final_screen = cap.capture(session, "verify_port_toggle_failed").await?;
        return Err(anyhow!(
            "Port toggle did not reflect as enabled after 3 attempts; latest screen:\n{final_screen}"
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
    if screen.contains("ModBus Master/Slave Set") {
        log::info!("Already in Modbus settings panel; skipping navigation");
        return Ok(());
    }

    // Verify we're in port details page (should see "Enable Port")
    // If not, we might have been kicked back to port list - need to re-enter
    if !screen.contains("Enable Port") {
        log::warn!("⚠️ Not in port details page - attempting to recover");
        log::warn!("Current screen:\n{}", screen);
        return Err(anyhow!(
            "Not in port details page when trying to enter Modbus panel. Expected 'Enable Port' in screen."
        ));
    }

    // From port details page, navigate down to "Business Configuration" and enter
    // First, capture screen to determine cursor position relative to target
    let screen = cap.capture(session, "before_modbus_nav").await?;
    let lines: Vec<&str> = screen.lines().collect();

    // Find "Enter Business Configuration" line
    let target_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| line.contains("Enter Business Configuration").then_some(idx))
        .ok_or_else(|| anyhow!("'Enter Business Configuration' not found in screen"))?;

    // Find cursor position - look for "> " at the beginning of menu items in the details panel
    // The cursor should be on lines containing options like "Enable Port", "Enter Business Configuration", etc.
    let cursor_idx = lines
        .iter()
        .enumerate()
        .find_map(|(idx, line)| {
            // Look for lines that have "> " followed by a known menu item
            // Check for "> " anywhere in the line (after │ or whitespace)
            if line.contains("> Enable Port")
                || line.contains("> Protocol Mode")
                || line.contains("> Enter Business")
                || line.contains("> Enter Log")
                || line.contains("> Baud rate")
                || line.contains("> Data bits")
                || line.contains("> Parity")
                || line.contains("> Stop bits")
            {
                // Exclude port list items (contain "COM" or "/tmp/")
                if !line.contains("COM") && !line.contains("/tmp/") && !line.contains("/dev/") {
                    return Some(idx);
                }
            }
            None
        })
        .unwrap_or(0);

    log::info!(
        "Navigating from line {} to 'Enter Business Configuration' at line {}",
        cursor_idx,
        target_idx
    );

    let delta = target_idx.abs_diff(cursor_idx);
    let direction = if target_idx > cursor_idx {
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
        crate::auto_cursor::CursorAction::PressEnter,
        crate::auto_cursor::CursorAction::Sleep { ms: 2000 }, // Give page time to load and stabilize rendering
        crate::auto_cursor::CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Set")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 10)), // Expanded range to catch the header even if scrolled
            col_range: None,
            retry_action: None, // Retry is handled at a higher level (setup_tui_port)
        },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "enter_modbus_panel")
        .await?;

    // Additional sleep after successful entry to allow UI to fully stabilize
    // This works around a rendering issue where the UI briefly shows incorrect state after page transition
    log::info!("✅ Entered Modbus panel, waiting for UI to fully stabilize...");
    crate::helpers::sleep_a_while().await;

    Ok(())
}

/// Update TUI registers with new values (shared implementation)
/// NOTE: Assumes you're already IN the Modbus panel and cursor is on or near the target station
pub async fn update_tui_registers<T: Expect>(
    session: &mut T,
    cap: &mut crate::snapshot::TerminalCapture,
    new_values: &[u16],
    _is_coil: bool,
) -> Result<()> {
    // Strategy: First navigate to a known position (top of Modbus panel),
    // then navigate down to find the first station's register grid.
    // This ensures we always start from the same position regardless of where
    // the cursor was after previous operations (like Ctrl+S).

    log::info!("🔍 Resetting to top of Modbus panel...");

    // Navigate up many times to ensure we reach the top
    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 50, // Large count to ensure we reach top
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "nav_to_top").await?;

    // Now search down for the register grid
    log::info!("🔍 Searching down for register grid...");
    let mut found_register = false;
    let mut attempts = 0;
    let max_attempts = 20;

    while !found_register && attempts < max_attempts {
        let screen = cap
            .capture(session, &format!("search_attempt_{}", attempts))
            .await?;

        // Check if current screen shows register values
        // Look for lines with multiple hex values (register display)
        for line in screen.lines() {
            // Register lines contain hex addresses and values like:
            // "    0x0000    0xABCD 0x1234 0x5678 0x9ABC"
            if line.contains("0x00") && line.matches("0x").count() >= 3 {
                found_register = true;
                log::info!("Found register grid at attempt {}", attempts);
                break;
            }
        }

        if !found_register {
            // Navigate down to find registers
            let actions = vec![
                crate::auto_cursor::CursorAction::PressArrow {
                    direction: crate::key_input::ArrowKey::Down,
                    count: 1,
                },
                crate::auto_cursor::CursorAction::Sleep { ms: 100 },
            ];
            crate::auto_cursor::execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("search_down_{}", attempts),
            )
            .await?;

            attempts += 1;
        }
    }

    if !found_register {
        return Err(anyhow!(
            "Could not find register grid after {} attempts from top",
            attempts
        ));
    }

    log::info!("Found register grid, navigating to first register of first row...");

    // We're now somewhere in the register grid. But we need to be careful:
    // The cursor order is: ... RegisterStartAddress -> RegisterLength -> Register0 -> Register1 -> ...
    // If we navigate up from the register display, we might land on RegisterStartAddress or RegisterLength.
    // 
    // Strategy: Navigate up until we find the "Register Length" line, then go down once to reach the first register.
    // This ensures we don't accidentally edit configuration fields.
    
    for attempt in 0..15 {
        let screen = cap
            .capture(session, &format!("search_for_reg_length_{}", attempt))
            .await?;

        // Look for "Register Length" line which is immediately before the first register
        let found_reg_length = screen
            .lines()
            .any(|l| l.contains("Register Length") || l.contains("register length"));

        if found_reg_length {
            log::info!("Found 'Register Length' field at attempt {}, navigating down to first register", attempt);
            // Move down once to get to the first register value
            let actions = vec![
                crate::auto_cursor::CursorAction::PressArrow {
                    direction: crate::key_input::ArrowKey::Down,
                    count: 1,
                },
                crate::auto_cursor::CursorAction::Sleep { ms: 300 },
            ];
            crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "to_first_reg_from_length")
                .await?;
            break;
        }

        // Navigate up to find Register Length
        let actions = vec![crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 1,
        }];
        crate::auto_cursor::execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("search_up_{}", attempt),
        )
        .await?;
    }

    log::info!("At first register row, starting updates...");

    for (i, &val) in new_values.iter().enumerate() {
        // Format as hex since TUI expects hex input for registers
        let hex_val = format!("{val:x}");
        let actions = vec![
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::TypeString(hex_val),
            crate::auto_cursor::CursorAction::PressEnter,
            crate::auto_cursor::CursorAction::Sleep { ms: 50 },
        ];
        crate::auto_cursor::execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("update_reg_{i}"),
        )
        .await?;

        if i < new_values.len() - 1 {
            let next_index = i + 1;
            let registers_per_row = 4;
            let next_col = next_index % registers_per_row;

            let actions = if next_col == 0 {
                vec![
                    crate::auto_cursor::CursorAction::PressArrow {
                        direction: crate::key_input::ArrowKey::Down,
                        count: 1,
                    },
                    crate::auto_cursor::CursorAction::PressArrow {
                        direction: crate::key_input::ArrowKey::Left,
                        count: registers_per_row - 1,
                    },
                ]
            } else {
                vec![crate::auto_cursor::CursorAction::PressArrow {
                    direction: crate::key_input::ArrowKey::Right,
                    count: 1,
                }]
            };

            crate::auto_cursor::execute_cursor_actions(
                session,
                cap,
                &actions,
                &format!("nav_to_reg_{next_index}"),
            )
            .await?;
        }
    }

    // Critical: Wait for all register values to be fully saved to internal storage
    // before any subsequent operations (like navigating to add another station)
    log::info!("⏱️ Waiting for all register values to be committed...");
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await;
    crate::helpers::sleep_a_while().await; // Extra safety margin

    // Navigate back to top of page for next station configuration
    // After editing registers, cursor is at the last register. The next call to
    // configure_tui_master_common expects to be near the top of the Modbus panel (station list).
    // We need to go up but NOT leave the Modbus panel (which would take us to port config).
    // The register grid is ~3-4 rows, so Up 10 should be safe to reach station list without leaving panel.
    log::info!("⬆️ Navigating back to station list within Modbus panel");
    let actions = vec![
        crate::auto_cursor::CursorAction::PressArrow {
            direction: crate::key_input::ArrowKey::Up,
            count: 10, // Moderate count to reach station list without leaving Modbus panel
        },
        crate::auto_cursor::CursorAction::Sleep { ms: 300 },
    ];
    crate::auto_cursor::execute_cursor_actions(session, cap, &actions, "return_to_station_list")
        .await?;

    Ok(())
}
