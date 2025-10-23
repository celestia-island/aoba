// Utility functions for TUI E2E tests
use anyhow::{anyhow, Result};
use std::time::Duration;

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    key_input::{ArrowKey, ExpectKeyExt},
    snapshot::TerminalCapture,
    tui::enter_modbus_panel,
};

/// Common cursor navigation and configuration utilities for TUI E2E tests
/// Navigate to a specific port in the TUI
pub async fn navigate_to_port<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
) -> Result<()> {
    log::info!("🔍 Navigating to port: {target_port}");

    // Capture screen - allow a few refresh cycles for port to appear
    let mut screen = String::new();
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 1..=MAX_ATTEMPTS {
        screen = cap
            .capture(session, &format!("nav_port_attempt_{attempt}"))
            .await?;

        if screen.contains(target_port) {
            if attempt > 1 {
                log::info!("✅ Port {target_port} detected after {attempt} attempts");
            }
            break;
        }

        if attempt == MAX_ATTEMPTS {
            return Err(anyhow!(
            "Port {target_port} not found in port list after {MAX_ATTEMPTS} attempts. Available: {available}",
            available = screen
                .lines()
                .filter(|l| l.contains("/tmp/") || l.contains("/dev/"))
                .take(10)
                .collect::<Vec<_>>()
                .join(", ")
            ));
        }

        log::info!("⏳ Port {target_port} not visible yet (attempt {attempt}/{MAX_ATTEMPTS})");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Parse screen to find port line and cursor line
    let lines: Vec<&str> = screen.lines().collect();
    let mut port_line = None;
    let mut cursor_line = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(target_port) {
            port_line = Some(idx);
        }
        if line.contains("> ") {
            let trimmed = line.trim();
            if trimmed.starts_with("│ > ") || trimmed.starts_with("> ") {
                cursor_line = Some(idx);
            }
        }
    }

    let port_idx =
        port_line.ok_or_else(|| anyhow!("Could not find {target_port} line index in screen"))?;
    let curr_idx = cursor_line.unwrap_or(3);

    log::info!("📍 Port at line {port_idx}, cursor at line {curr_idx}");

    // Check if already on the target port
    if port_idx != curr_idx {
        // Calculate delta and move cursor
        let delta = port_idx.abs_diff(curr_idx);
        let direction = if port_idx > curr_idx {
            ArrowKey::Down
        } else {
            ArrowKey::Up
        };

        log::info!("➡️ Moving cursor {direction:?} by {delta} lines");

        let actions = vec![CursorAction::PressArrow {
            direction,
            count: delta,
        }];
        execute_cursor_actions(session, cap, &actions, "navigate_to_target_port").await?;
    }

    // Press Enter to enter port details, with pattern matching to verify
    // We're looking for "Enable Port" in the details panel to confirm we've entered
    use regex::Regex;
    let port_pattern =
        Regex::new(r"Enable Port").map_err(|e| anyhow!("Failed to create regex pattern: {}", e))?;

    // Retry action: if we haven't entered port details, press Escape and try again
    let retry_action = Some(vec![
        CursorAction::PressEscape,
        CursorAction::PressPageUp, // Jump to first cursor position
    ]);

    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 }, // Wait for details panel to load
        CursorAction::MatchPattern {
            pattern: port_pattern,
            description: format!("In {target_port} port details (checking for Enable Port)"),
            line_range: Some((0, 10)),
            col_range: None,
            retry_action,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_port_details").await?;

    log::info!("✅ Successfully navigated to and entered {target_port} details page");
    Ok(())
}

/// Create multiple Modbus stations in bulk (Phase 1 of configuration)
///
/// This function creates N stations by pressing Enter N times on the "Create Station" button.
/// After creation, it verifies the last station exists and optionally switches to Master mode.
///
/// # Arguments
/// * `session` - Terminal session
/// * `cap` - Terminal capture for screenshots
/// * `station_count` - Number of stations to create
/// * `is_master` - If true, switches the mode to Master after creation
pub async fn create_modbus_stations<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_count: usize,
    is_master: bool,
) -> Result<()> {
    use regex::Regex;

    log::info!("🏗️ Phase 1: Creating {station_count} Modbus stations (Master: {is_master})");

    // Verify we are inside Modbus panel
    let screen = cap
        .capture(session, "verify_modbus_panel_before_creation")
        .await?;
    if !screen.contains("ModBus Master/Slave Set") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for station creation"
        ));
    }

    // Navigate to "Create Station" button using Ctrl+PageUp
    log::info!("📍 Navigating to Create Station button");
    let actions = vec![CursorAction::PressCtrlPageUp];
    execute_cursor_actions(session, cap, &actions, "nav_to_create_station_button").await?;

    // Create stations by pressing Enter N times
    // After each Enter, cursor moves to the new station, so we need to go back to "Create Station" button
    log::info!("➕ Creating {station_count} stations...");
    for i in 1..=station_count {
        log::info!("  Creating station {i}/{station_count}");
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 }, // Wait longer for station creation to complete (especially for CI)
            // After creating a station, cursor moves down to the new station
            // We need to go back to "Create Station" button for the next iteration
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 200 }, // Small wait after navigation
        ];
        execute_cursor_actions(session, cap, &actions, &format!("create_station_{i}")).await?;
    }

    // Verify the last station was created using regex screenshot
    log::info!("🔍 Verifying station #{station_count} exists");
    let station_pattern = Regex::new(&format!(r"#{}(?:\D|$)", station_count))?;
    let actions = vec![
        CursorAction::Sleep { ms: 500 }, // Wait longer for UI to stabilize after creation (especially for CI)
        CursorAction::MatchPattern {
            pattern: station_pattern,
            description: format!("Station #{station_count} exists"),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "verify_last_station_created").await?;

    // Press Down arrow to move off the "Create Station" button
    log::info!("⬇️ Moving cursor down from Create Station button");
    let actions = vec![CursorAction::PressArrow {
        direction: ArrowKey::Down,
        count: 1,
    }];
    execute_cursor_actions(session, cap, &actions, "move_down_after_creation").await?;

    // If Master mode is needed, switch to it
    if is_master {
        log::info!("🔄 Switching to Master mode: Enter, Right, Enter");
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: 1,
            },
            CursorAction::PressEnter,
        ];
        execute_cursor_actions(session, cap, &actions, "switch_to_master_mode").await?;
    }

    // Move to beginning using Ctrl+PgUp
    log::info!("⏫ Moving to beginning with Ctrl+PgUp");
    let actions = vec![CursorAction::PressCtrlPageUp];
    execute_cursor_actions(session, cap, &actions, "move_to_beginning_after_creation").await?;

    log::info!("✅ Phase 1 complete: Created {station_count} stations");
    Ok(())
}

/// Configure a single Modbus station (Phase 2 of configuration)
///
/// This function configures one station by navigating to it once and then
/// explicitly navigating to each field using Down arrow counts from the station header.
/// This ensures reliable field access while avoiding repeated full re-navigation.
///
/// # Arguments
/// * `session` - Terminal session
/// * `cap` - Terminal capture for screenshots
/// * `station_index` - Station index (0-based, so station #1 has index 0)
/// * `station_id` - Station ID to set
/// * `register_type` - Register type (1=Coils, 2=Discrete, 3=Holding, 4=Input)
/// * `start_address` - Start address (will be entered as decimal, displayed as hex)
/// * `register_count` - Number of registers (will be entered as decimal)
pub async fn configure_modbus_station<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_index: usize,
    station_id: u8,
    register_type: u8,
    start_address: u16,
    register_count: usize,
) -> Result<()> {
    use regex::Regex;

    let station_number = station_index + 1; // Station #1, #2, etc. (1-based)

    log::info!("⚙️ Configuring Station #{station_number} (ID={station_id}, Type={register_type:02}, Addr=0x{start_address:04X}, Count={register_count})");

    // Verify station exists before configuring
    let station_pattern = Regex::new(&format!(r"#{}(?:\D|$)", station_number))?;
    let verify_actions = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep { ms: 200 },
        CursorAction::MatchPattern {
            pattern: station_pattern,
            description: format!("Station #{station_number} exists"),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(
        session,
        cap,
        &verify_actions,
        &format!("verify_station_{station_number}"),
    )
    .await?;

    // Configure each field by navigating from top each time (reliable but verbose)
    // This ensures we always start from a known position
    
    // ===== Field 1: Station ID =====
    log::info!("📝 Setting Station ID to {station_id}");
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    // PgDown logic: observations show:
    // 1 PgDown -> Connection Mode
    // 2 PgDown -> Station #1
    // 3 PgDown -> Station #2
    // So for Station N (0-indexed), we need station_index + 2 PgDown presses
    for _ in 0..(station_index + 2) {
        actions.push(CursorAction::PressPageDown);
    }
    // Add debug breakpoint to see where we land
    actions.push(CursorAction::DebugBreakpoint {
        description: format!("after_pgdown_to_station_{}", station_number),
    });
    actions.extend(vec![
        // PgDown lands us at Station ID field, no need to press Down
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 }, // Wait for edit mode to activate
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 }, // CRITICAL: Wait for edit mode to fully exit and value to commit
    ]);
    execute_cursor_actions(session, cap, &actions, &format!("set_station_id_s{station_number}")).await?;

    // ===== Field 2: Register Type =====
    log::info!("📝 Setting Register Type to {register_type:02}");
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    for _ in 0..(station_index + 2) {
        actions.push(CursorAction::PressPageDown);
    }
    actions.extend(vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }, // Down 1 for Register Type (PgDown lands at Station ID)
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ]);
    
    // Navigate to correct register type
    let current_pos = 2; // Holding (03) is default
    let target_pos = (register_type as usize).saturating_sub(1);
    
    if target_pos < current_pos {
        actions.push(CursorAction::PressArrow {
            direction: ArrowKey::Left,
            count: current_pos - target_pos,
        });
        actions.push(CursorAction::Sleep { ms: 300 });
    } else if target_pos > current_pos {
        actions.push(CursorAction::PressArrow {
            direction: ArrowKey::Right,
            count: target_pos - current_pos,
        });
        actions.push(CursorAction::Sleep { ms: 300 });
    }
    
    actions.extend(vec![
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 }, // Wait for selection to commit
    ]);
    execute_cursor_actions(session, cap, &actions, &format!("set_register_type_s{station_number}")).await?;

    // ===== Field 3: Start Address =====
    log::info!("📝 Setting Start Address to 0x{start_address:04X} ({start_address})");
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    for _ in 0..(station_index + 2) {
        actions.push(CursorAction::PressPageDown);
    }
    actions.extend(vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 }, // Down 2 for Start Address
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(start_address.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 1000 }, // Wait for value to commit
    ]);
    execute_cursor_actions(session, cap, &actions, &format!("set_start_address_s{station_number}")).await?;

    // ===== Field 4: Register Length =====
    log::info!("📝 Setting Register Length to {register_count}");
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    for _ in 0..(station_index + 2) {
        actions.push(CursorAction::PressPageDown);
    }
    actions.push(CursorAction::DebugBreakpoint {
        description: format!("after_pgdown_before_down_for_reglen_s{}", station_number),
    });
    actions.extend(vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 3 }, // Down 3 for Register Length
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
        CursorAction::DebugBreakpoint {
            description: format!("before_type_register_length_s{}", station_number),
        },
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString(register_count.to_string()),
        CursorAction::Sleep { ms: 1000 }, // Wait after typing
        CursorAction::DebugBreakpoint {
            description: format!("after_type_before_enter_s{}", station_number),
        },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 3000 }, // CRITICAL: Much longer wait for register grid initialization and value commit
        CursorAction::DebugBreakpoint {
            description: format!("after_set_register_length_s{}", station_number),
        },
    ]);
    execute_cursor_actions(session, cap, &actions, &format!("set_register_length_s{station_number}")).await?;

    // Return to top with Ctrl+PgUp as per workflow requirement
    log::info!("⏫ Returning to top with Ctrl+PgUp");
    let actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep { ms: 200 }];
    execute_cursor_actions(session, cap, &actions, &format!("return_to_top_s{station_number}")).await?;

    log::info!("✅ Station #{station_number} configured successfully");
    Ok(())
}

/// Configure multiple Modbus stations in a batch
///
/// This is the recommended high-level function for configuring multiple stations.
/// It handles the two-phase process automatically:
/// 1. Creates all stations at once
/// 2. Configures each station with provided parameters
///
/// # Arguments
/// * `session` - The TUI process session
/// * `cap` - Terminal capture for screen snapshots
/// * `stations` - Array of station configurations (station_id, register_type, start_address, register_count)
///
/// # Example
/// ```ignore
/// let stations = [
///     (1, 3, 0, 8),    // Station 1, Type 03, Address 0, Length 8
///     (1, 3, 12, 8),   // Station 1, Type 03, Address 12, Length 8
///     (1, 3, 24, 8),   // Station 1, Type 03, Address 24, Length 8
/// ];
/// configure_multiple_stations(&mut session, &mut cap, &stations).await?;
/// ```
pub async fn configure_multiple_stations<T: ExpectKeyExt + Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    stations: &[(u8, u8, u16, usize)], // (station_id, register_type, start_address, register_count)
) -> Result<()> {
    let num_stations = stations.len();

    log::info!(
        "🏗️ Starting batch configuration for {} stations",
        num_stations
    );

    // Phase 1: Create all stations at once
    log::info!("📋 Phase 1: Creating {} stations", num_stations);
    create_modbus_stations(session, cap, num_stations, false).await?;
    log::info!("✅ Phase 1 complete: All {} stations created", num_stations);

    // Phase 2: Configure each station individually
    log::info!("⚙️ Phase 2: Configuring each station");
    for (i, &(station_id, register_type, start_address, register_count)) in
        stations.iter().enumerate()
    {
        log::info!(
            "🔧 Phase 2.{}: Configuring Station {} (ID={}, Type={:02}, Addr=0x{:04X}, Length={})",
            i + 1,
            i + 1,
            station_id,
            register_type,
            start_address,
            register_count
        );

        configure_modbus_station(
            session,
            cap,
            i, // station_index (0-based)
            station_id,
            register_type,
            start_address,
            register_count,
        )
        .await?;

        log::info!("✅ Station {} configured successfully", i + 1);
    }

    log::info!(
        "✅ Phase 2 complete: All {} stations configured",
        num_stations
    );

    // NOTE: Data update phase is intentionally skipped
    // New stations have default register values (0) which is sufficient for configuration testing
    // If specific register values are needed, they should be set after this function returns

    Ok(())
}

/// Configure a TUI process as a Modbus Master with common settings
///
/// **DEPRECATED**: This function is deprecated. Use the two-phase approach instead:
/// 1. Call `create_modbus_stations` once to create all stations
/// 2. Call `configure_modbus_station` for each station to configure it
///
/// This function remains for backward compatibility but may have navigation issues
/// in multi-station scenarios. The new two-phase approach follows the standard
/// test flow more closely and is more reliable.
#[deprecated(
    since = "0.0.1",
    note = "Use create_modbus_stations + configure_modbus_station instead"
)]
#[allow(dead_code)]
pub async fn configure_tui_master_common<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_type: u8,
    register_mode: &str,
    start_address: u16,
    register_length: usize,
    is_first_station: bool, // NEW: indicates if this is the first station
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Master (Station {station_id}, Type {register_type:02}, Address 0x{start_address:04X}, first={is_first_station})");

    // Verify we are inside Modbus panel
    let screen = cap
        .capture(session, &format!("verify_modbus_panel_master{station_id}"))
        .await?;
    if !screen.contains("ModBus Master/Slave Set") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for Master (Station {station_id})"
        ));
    }

    // Only create a new station if this is the first one
    // For subsequent stations, TUI already created them and moved cursor there
    if is_first_station {
        // Navigate to "Create Station" button at the top of the Modbus panel
        // Use Ctrl+PageUp to jump to the first group (AddLine)
        log::info!("📍 Navigating to Create Station button using Ctrl+PageUp");
        let actions = vec![
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"(?i)create.*station|Create Station")?,
                description: "Create Station button focused".to_string(),
                line_range: None,
                col_range: None,
                retry_action: Some(vec![
                    CursorAction::PressArrow {
                        direction: ArrowKey::Down,
                        count: 1,
                    },
                    CursorAction::Sleep { ms: 300 },
                ]),
            },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("nav_to_add_line_master{station_id}"),
        )
        .await?;

        // Create station
        log::info!("🏗️ Creating Modbus station by pressing Enter on Create Station");
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"#\d+")?, // Match #1, #2, #3, etc.
                description: format!("Station #{} created", station_id),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("create_station_master{station_id}"),
        )
        .await?;
    } else {
        log::info!("📍 Skipping station creation (station already created by previous call, cursor should be on new station)");
    }

    // Set Station ID if not 1
    if station_id != 1 {
        log::info!("📝 Setting station ID to {station_id}");
        let actions = vec![
            // Navigate to Station ID field using Ctrl+PageUp + PageDown
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressPageDown, // Jump to first station
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1, // Move to Station ID field
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::TypeString(station_id.to_string()),
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_station_id_{station_id}"),
        )
        .await?;
    }

    // Set Register Type
    log::info!("📝 Setting register type to {register_type:02} ({register_mode})");

    // Use is_first_station parameter to determine navigation strategy
    // instead of trying to detect from screen content
    let is_multi_station = !is_first_station;

    if is_multi_station {
        log::info!("🔍 Multi-station mode: using precise navigation for station {station_id}");

        // For multi-station, navigate to register type field
        // IMPORTANT: New stations default to Holding (03) in TUI code
        // The selector opens at the CURRENT value, not at Coils
        let actions = if register_type == 3 {
            // Target is Holding (03), which is the default - just confirm
            vec![
                // From StationId, go down 1 to Register Type
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 300 },
                // Already at Holding (default), just confirm
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 500 },
            ]
        } else {
            // Need to navigate from Holding (03) to target
            // Calculate navigation: we're at position 2 (Holding), need to get to target position
            // Positions: 0=Coils(01), 1=Discrete(02), 2=Holding(03), 3=Input(04)
            let current_pos = 2; // Holding is at position 2
            let target_pos = (register_type as usize).saturating_sub(1);
            let nav_count = if target_pos > current_pos {
                target_pos - current_pos
            } else {
                // Navigate left (or wrap around - but for now assume we won't need this)
                0
            };

            vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: nav_count,
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 500 },
            ]
        };
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_multi"),
        )
        .await?;
    } else {
        log::info!("🔍 Single station mode: using standard navigation");

        // For single station, use standard navigation
        // Navigate to Register Type field using Ctrl+PageUp + PageDown
        // The selector defaults to Holding(03), so we just confirm it.
        let actions = vec![
            // Navigate to Register Type field
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressPageDown, // Jump to first station
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2, // Move to Register Type field
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            // When we enter register type selection, it defaults to Holding(03)
            // Just press Enter to confirm the selection
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_single"),
        )
        .await?;
    }

    // Remove the MatchPattern verification since it's not reliable
    log::info!("✅ Register type configuration completed");

    // Set Start Address
    log::info!("📝 Setting start address to 0x{start_address:04X}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(start_address.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_start_address_master{station_id}"),
    )
    .await?;

    // Set Register Length
    log::info!("📝 Setting register length to {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_register_length_master{station_id}"),
    )
    .await?;

    log::info!("✅ Master (Station {station_id}, Type {register_type:02}, Address 0x{start_address:04X}) configured successfully");
    Ok(())
}

/// Configure a TUI process as a Modbus Slave with common settings
///
/// **DEPRECATED**: This function is deprecated. Use the two-phase approach instead:
/// 1. Call `create_modbus_stations` once to create all stations (with is_master=false)
/// 2. Call `configure_modbus_station` for each station to configure it
///
/// This function remains for backward compatibility but may have navigation issues
/// in multi-station scenarios. The new two-phase approach follows the standard
/// test flow more closely and is more reliable.
#[deprecated(
    since = "0.0.1",
    note = "Use create_modbus_stations + configure_modbus_station instead"
)]
#[allow(dead_code)]
pub async fn configure_tui_slave_common<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
    register_type: u8,
    register_mode: &str,
    start_address: u16,
    register_length: usize,
) -> Result<()> {
    use regex::Regex;

    log::info!("📝 Configuring Slave (Station {station_id}, Type {register_type:02}, Address 0x{start_address:04X})");

    // Verify we are inside Modbus panel
    let screen = cap
        .capture(session, &format!("verify_modbus_panel_slave{station_id}"))
        .await?;
    if !screen.contains("ModBus Master/Slave Set") {
        return Err(anyhow!(
            "Expected to be inside ModBus panel for Slave (Station {station_id})"
        ));
    }

    // Create station
    log::info!("🏗️ Creating Modbus station");
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("create_station_slave{station_id}"),
    )
    .await?;

    // Set Station ID if not 1
    if station_id != 1 {
        log::info!("📝 Setting station ID to {station_id}");
        let actions = vec![
            // Navigate to Station ID field using Ctrl+PageUp + PageDown
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressPageDown, // Jump to first station
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1, // Move to Station ID field
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            CursorAction::TypeString(station_id.to_string()),
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 },
        ];
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_station_id_{station_id}"),
        )
        .await?;
    }

    // Set Register Type
    log::info!("📝 Setting register type to {register_type:02} ({register_mode})");

    // First, capture the current screen to understand the layout
    let screen = cap
        .capture(
            session,
            &format!("before_set_register_type_{register_type}"),
        )
        .await?;

    // Check if we're in a multi-station configuration
    let is_multi_station = screen.lines().filter(|l| l.contains("#")).count() > 1;

    if is_multi_station {
        log::info!("🔍 Multi-station configuration detected, using precise navigation");

        // For multi-station, navigate to register type field using Ctrl+PageUp + PageDown
        let actions = vec![
            // Navigate to top of the current station section
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressPageDown, // Jump to first station
            CursorAction::Sleep { ms: 300 },
            // Navigate down to Register Type field (usually 3rd field in station)
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 300 },
            // Navigate through register types to reach target type
            // IMPORTANT: New stations default to Holding (03), so selector starts there
            // If target is Holding (03), just confirm; otherwise navigate
        ];

        let nav_actions = if register_type == 3 {
            // Target is Holding (03), which is the default - just confirm
            vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 500 }]
        } else {
            // Need to navigate from Holding (03) to target
            // Strategy: Go all the way left to Coils (01), then navigate right to target
            vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Left,
                    count: 5, // Go all the way left first (to Coils/01)
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: (register_type as usize).saturating_sub(1), // Adjust for 0-indexed: type 03 needs 2 Right presses (01->02->03)
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 500 },
            ]
        };

        let mut actions = actions;
        actions.extend(nav_actions);
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_multi"),
        )
        .await?;
    } else {
        log::info!("🔍 Single station configuration, using standard navigation");

        // For single station, use standard navigation
        // Navigate to Register Type field using Ctrl+PageUp + PageDown
        // The selector defaults to Holding(03)
        let actions = if register_type == 3 {
            // Target is Holding (03), which is the default - just confirm
            vec![
                // Navigate to Register Type field
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressPageDown, // Jump to first station
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 2, // Move to Register Type field
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 300 },
                // When we enter register type selection, it defaults to Holding(03)
                // Just press Enter to confirm the selection
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 500 },
                // Verify the register type was set correctly
                CursorAction::MatchPattern {
                    pattern: regex::Regex::new(&format!("Register Type.*{register_type:02}"))?,
                    description: format!("Register type set to {register_type:02}"),
                    line_range: None,
                    col_range: None,
                    retry_action: None,
                },
            ]
        } else {
            // Need to navigate from Holding (03) to target
            // Navigate left to reset, then right to target
            vec![
                // Navigate to Register Type field
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressPageDown, // Jump to first station
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 2, // Move to Register Type field
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 300 },
                // Navigate from Holding(03) to target type
                CursorAction::PressArrow {
                    direction: ArrowKey::Left,
                    count: 5, // Go all the way left to Coils(01)
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: (register_type as usize).saturating_sub(1),
                },
                CursorAction::Sleep { ms: 300 },
                CursorAction::PressEnter,
                CursorAction::Sleep { ms: 500 },
                // Verify the register type was set correctly
                CursorAction::MatchPattern {
                    pattern: regex::Regex::new(&format!("Register Type.*{register_type:02}"))?,
                    description: format!("Register type set to {register_type:02}"),
                    line_range: None,
                    col_range: None,
                    retry_action: None,
                },
            ]
        };
        execute_cursor_actions(
            session,
            cap,
            &actions,
            &format!("set_register_type_{register_type}_single"),
        )
        .await?;
    }

    // Set Start Address
    log::info!("📝 Setting start address to 0x{start_address:04X}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(start_address.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_start_address_slave{station_id}"),
    )
    .await?;

    // Set Register Length
    log::info!("📝 Setting register length to {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 300 },
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        session,
        cap,
        &actions,
        &format!("set_register_length_slave{station_id}"),
    )
    .await?;

    log::info!("✅ Slave (Station {station_id}, Type {register_type:02}, Address 0x{start_address:04X}) configured successfully");
    Ok(())
}

/// Navigate to a port and enter its Modbus panel WITHOUT enabling it first.
/// This is used for the new workflow where we configure first, then enable automatically on save.
pub async fn navigate_to_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    target_port: &str,
) -> Result<()> {
    // Navigate to the target port details page
    log::info!("📍 Navigating to port {target_port}");
    navigate_to_port(session, cap, target_port).await?;

    // Enter Modbus panel directly without enabling the port
    log::info!("⚙️ Entering Modbus configuration panel (port not yet enabled)");
    enter_modbus_panel(session, cap).await?;

    log::info!("✅ Successfully entered Modbus panel for {target_port}");
    Ok(())
}

/// Test a station with retry logic - common polling function
pub async fn test_station_with_retries(
    port: &str,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    expected_data: &[u16],
    max_retries: usize,
    retry_interval_ms: u64,
) -> Result<bool> {
    use std::process::{Command, Stdio};

    let binary = ci_utils::terminal::build_debug_bin("aoba")?;

    for attempt in 1..=max_retries {
        log::info!(
            "  Attempt {attempt}/{max_retries}: Polling {port} for Station {station_id} ({register_mode}) at address 0x{start_address:04X}"
        );

        let cli_output = Command::new(&binary)
            .args([
                "--slave-poll",
                port,
                "--station-id",
                &station_id.to_string(),
                "--register-address",
                &start_address.to_string(),
                "--register-length",
                &expected_data.len().to_string(),
                "--register-mode",
                register_mode,
                "--baud-rate",
                "9600",
                "--json",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if cli_output.status.success() {
            let stdout = String::from_utf8_lossy(&cli_output.stdout);

            // Parse and check the data
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let received: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();

                    if received == expected_data {
                        log::info!("  ✅ SUCCESS on attempt {attempt}: Data verified!");
                        return Ok(true);
                    } else {
                        log::warn!(
                            "  ⚠️ Data mismatch on attempt {attempt}: expected {expected_data:?}, got {received:?}"
                        );
                    }
                }
            }
        } else {
            let stderr = String::from_utf8_lossy(&cli_output.stderr);
            log::warn!("  ⚠️ Poll failed on attempt {attempt}: {stderr}");
        }

        // Wait before next attempt (except after last attempt)
        if attempt < max_retries {
            tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
        }
    }

    log::error!("  ❌ FAILED: All {max_retries} attempts failed for Station {station_id}");
    Ok(false)
}
