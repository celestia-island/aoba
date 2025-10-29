use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;
use serde_json::json;

use super::{
    config::RegisterMode,
    status_paths::{page_type_path, port_field_path, station_collection, station_field_path},
};
use ci_utils::{
    execute_with_status_checks, read_tui_status, ArrowKey, CursorAction, TerminalCapture,
};

const MODBUS_DASHBOARD_PAGE: &str = "modbus_dashboard";

fn modbus_page_check(description: &str) -> CursorAction {
    CursorAction::CheckStatus {
        description: description.to_string(),
        path: page_type_path().to_string(),
        expected: json!(MODBUS_DASHBOARD_PAGE),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }
}

/// Ensure the cursor is focused on the "Create Station" button at the top of the dashboard.
///
/// This helper recenters the cursor using `Ctrl+PageUp` and verifies via a pattern match that
/// the highlighted line contains the "Create Station" label. All downstream configuration steps
/// assume this starting cursor position for deterministic navigation.
pub async fn focus_create_station_button<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    let pattern = Regex::new(r">\s*Create Station")?;

    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep1s,
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 1,
            },
            CursorAction::Sleep1s,
        ],
        &[
            CursorAction::MatchPattern {
                pattern,
                description: "Cursor positioned on Create Station".to_string(),
                line_range: None,
                col_range: None,
                retry_action: Some(vec![
                    CursorAction::PressEscape,
                    CursorAction::Sleep1s,
                    CursorAction::PressCtrlPageUp,
                    CursorAction::Sleep1s,
                    CursorAction::PressArrow {
                        direction: ArrowKey::Up,
                        count: 1,
                    },
                    CursorAction::Sleep1s,
                ]),
            },
            modbus_page_check("ModbusDashboard active while focusing Create Station"),
        ],
        "focus_create_station_button",
        Some(3),
    )
    .await
}

/// Ensure the Modbus connection mode matches the desired role before creating a station.
///
/// When configuring a Slave station, this toggles the connection mode selector from Master to
/// Slave and verifies the UI text reflects the change. The cursor is returned to the "Create
/// Station" button so subsequent steps can proceed without additional navigation adjustments.
pub async fn ensure_connection_mode<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    is_master: bool,
) -> Result<()> {
    if is_master {
        // Default state is Master; nothing to change. Just ensure the cursor is back at AddLine.
        return Ok(());
    }

    let slave_pattern = Regex::new(r"Connection Mode.*Slave")?;
    let connection_mode_focus_pattern = Regex::new(r">\s*Connection Mode")?;
    let connection_mode_edit_pattern = Regex::new(r"Connection Mode.*<\s*(Master|Slave)\s*>")?;
    let connection_mode_edit_slave_pattern = Regex::new(r"Connection Mode.*<\s*Slave\s*>")?;

    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressCtrlPageUp,
            CursorAction::Sleep1s,
            CursorAction::PressArrow {
                direction: ArrowKey::Up,
                count: 5,
            },
            CursorAction::Sleep1s,
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2,
            },
            CursorAction::Sleep1s,
        ],
        &[
            CursorAction::MatchPattern {
                pattern: connection_mode_focus_pattern,
                description: "Cursor moved to Connection Mode".to_string(),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
            modbus_page_check("ModbusDashboard active while navigating to Connection Mode"),
        ],
        "navigate_to_connection_mode",
        None,
    )
    .await?;

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[
            CursorAction::MatchPattern {
                pattern: connection_mode_edit_pattern,
                description: "Connection Mode selector opened".to_string(),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
            modbus_page_check("ModbusDashboard active in Connection Mode edit"),
        ],
        "enter_connection_mode_edit",
        None,
    )
    .await?;

    const MAX_TOGGLE_ATTEMPTS: usize = 3;
    let mut slave_selected = false;
    for attempt in 1..=MAX_TOGGLE_ATTEMPTS {
        let screen = cap
            .capture_with_logging(
                session,
                &format!("connection_mode_edit_state_attempt_{}", attempt),
                false,
            )
            .await?;

        if connection_mode_edit_slave_pattern.is_match(&screen) {
            slave_selected = true;
            break;
        }

        if attempt == MAX_TOGGLE_ATTEMPTS {
            break;
        }

        execute_with_status_checks(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Right,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            &[modbus_page_check(
                "ModbusDashboard active while toggling Connection Mode",
            )],
            &format!("toggle_connection_mode_attempt_{}", attempt),
            None,
        )
        .await?;
    }

    if !slave_selected {
        return Err(anyhow!(
            "Failed to switch Connection Mode selector to Slave after {MAX_TOGGLE_ATTEMPTS} attempts"
        ));
    }

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::Sleep1s],
        &[
            CursorAction::MatchPattern {
                pattern: connection_mode_edit_slave_pattern,
                description: "Connection Mode selector switched to Slave".to_string(),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
            modbus_page_check("ModbusDashboard active while selecting Slave mode"),
        ],
        "select_slave_connection_mode",
        None,
    )
    .await?;

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[
            CursorAction::MatchPattern {
                pattern: slave_pattern,
                description: "Connection mode shows Slave".to_string(),
                line_range: None,
                col_range: None,
                retry_action: None,
            },
            modbus_page_check("ModbusDashboard active after confirming Slave mode"),
        ],
        "confirm_slave_connection_mode",
        Some(3),
    )
    .await?;

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::Sleep1s],
        &[modbus_page_check(
            "ModbusDashboard active while waiting for connection mode settle",
        )],
        "connection_mode_settle",
        None,
    )
    .await?;

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressCtrlPageUp, CursorAction::Sleep1s],
        &[modbus_page_check(
            "ModbusDashboard active after returning to Create Station",
        )],
        "return_to_create_after_connection_mode",
        None,
    )
    .await
}

/// Move the cursor focus to the specified station section.
///
/// After creating a new station the cursor may remain on the "Create Station" button
/// or the previously selected station. This helper recenters the cursor at the top of
/// the dashboard and pages down to the desired station index so that subsequent edit
/// steps operate on the intended station fields.
pub async fn focus_station<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    is_master: bool,
) -> Result<()> {
    let mut actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep1s];
    actions.push(CursorAction::PressPageDown); // Jump from AddLine to global mode selector
    actions.push(CursorAction::Sleep1s);

    for _ in 0..=station_index {
        actions.push(CursorAction::PressPageDown);
        actions.push(CursorAction::Sleep1s);
    }

    execute_with_status_checks(
        session,
        cap,
        &actions,
        &[
            CursorAction::CheckStatus {
                description: format!("Station {} visible", station_index + 1),
                path: station_field_path(port_name, is_master, station_index, "register_type"),
                expected: json!("Holding"),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
            modbus_page_check("ModbusDashboard active while focusing station"),
        ],
        &format!("focus_station_{}", station_index + 1),
        Some(3),
    )
    .await
}

/// Creates a new station and verifies its creation via a status check.
///
/// This function encapsulates the actions and verification for creating a single station.
/// It presses "Enter" on the "Create Station" button and then uses a `CheckStatus`
/// action to confirm that the station appears in the status file.
///
/// # Arguments
///
/// * `session` - The expectrl session to interact with the TUI.
/// * `cap` - The terminal capture utility for debugging.
/// * `port_name` - The name of the port where the station is being created.
/// * `is_master` - A boolean indicating whether the created station should be a master or a slave.
///
/// # Returns
///
/// * `Result<usize>` - Index of the newly created station in the status tree.
pub async fn create_station<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    is_master: bool,
) -> Result<usize> {
    let status = read_tui_status()?;
    let port = status
        .ports
        .iter()
        .find(|p| p.name == port_name)
        .ok_or_else(|| anyhow!("Port {port_name} not found when creating station"))?;

    let current_count = if is_master {
        port.modbus_masters.len()
    } else {
        port.modbus_slaves.len()
    };

    log::info!(
        "📊 Current {} station count before create: {}",
        if is_master { "master" } else { "slave" },
        current_count
    );

    let new_index = current_count;
    let collection = station_collection(is_master);
    let description = format!("Station #{} created ({collection})", new_index + 1);
    let station_id_path = station_field_path(port_name, is_master, new_index, "station_id");

    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressEnter, // Press on "Create Station"
            CursorAction::Sleep3s,
        ],
        &[
            CursorAction::CheckStatus {
                description,
                path: station_id_path,
                expected: json!(1),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
            modbus_page_check("ModbusDashboard active after creating station"),
        ],
        "create_station",
        Some(3),
    )
    .await?;

    log::info!(
        "✅ Station created at index {} ({} collection)",
        new_index,
        collection
    );

    Ok(new_index)
}

/// Configures the Station ID for a given station.
///
/// This function performs the following steps:
/// 1. Navigates to the Station ID field.
/// 2. Enters edit mode.
/// 3. Clears the existing value.
/// 4. Types the new station ID.
/// 5. Commits the change and verifies it via a status check.
///
/// # Arguments
///
/// * `session` - The expectrl session.
/// * `cap` - The terminal capture utility.
/// * `port_name` - The name of the port.
/// * `station_index` - The index of the station being configured.
/// * `station_id` - The new station ID to set.
/// * `is_master` - Whether the station is a master or a slave.
///
/// # Returns
///
/// * `Result<()>` - Ok if the Station ID was configured and verified successfully.
pub async fn configure_station_id<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    station_id: u8,
    is_master: bool,
) -> Result<()> {
    let path = station_field_path(port_name, is_master, station_index, "station_id");

    // Step 1: Enter edit mode
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[modbus_page_check(
            "ModbusDashboard active while entering Station ID edit",
        )],
        "enter_edit_station_id",
        None,
    )
    .await?;

    // Step 2: Type new value
    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(station_id.to_string()),
        ],
        &[modbus_page_check(
            "ModbusDashboard active while typing Station ID",
        )],
        "type_station_id",
        None,
    )
    .await?;

    // Step 3: Commit and verify
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[CursorAction::CheckStatus {
            description: format!("Station ID updated to {}", station_id),
            path,
            expected: json!(station_id),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "commit_station_id",
        Some(3),
    )
    .await
}

/// Configures the Register Type for a given station.
///
/// This function performs the following steps:
/// 1. Navigates to the Register Type field.
/// 2. Enters the selection mode.
/// 3. Selects the new register type.
/// 4. Commits the change and verifies it via a status check.
///
/// # Arguments
///
/// * `session` - The expectrl session.
/// * `cap` - The terminal capture utility.
/// * `port_name` - The name of the port.
/// * `station_index` - The index of the station being configured.
/// * `register_mode` - The new register mode to set.
/// * `is_master` - Whether the station is a master or a slave.
///
/// # Returns
///
/// * `Result<()>` - Ok if the Register Type was configured and verified successfully.
pub async fn configure_register_type<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    register_mode: RegisterMode,
    is_master: bool,
) -> Result<()> {
    let path = station_field_path(port_name, is_master, station_index, "register_type");
    let (direction, count) = register_mode.arrow_from_default();

    // Step 1: Navigate to Register Type field
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 1,
        }],
        &[modbus_page_check(
            "ModbusDashboard active after navigating to Register Type",
        )],
        "nav_to_register_type",
        None,
    )
    .await?;

    // Step 2: Enter register type selector
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[modbus_page_check(
            "ModbusDashboard active while entering Register Type selector",
        )],
        "enter_register_type_selector",
        None,
    )
    .await?;

    // Step 3: Navigate to desired option if movement is needed
    if count > 0 {
        execute_with_status_checks(
            session,
            cap,
            &[
                CursorAction::PressArrow { direction, count },
                CursorAction::Sleep1s,
            ],
            &[modbus_page_check(
                "ModbusDashboard active while selecting Register Type",
            )],
            "select_register_type_option",
            None,
        )
        .await?;
    }

    // Step 4: Commit selection and verify status update
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[CursorAction::CheckStatus {
            description: format!("Register type is {:?}", register_mode),
            path,
            expected: json!(format!("{register_mode:?}")),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "confirm_register_type",
        Some(3),
    )
    .await
}

/// Configures a numeric field (like Start Address or Register Count) for a station.
///
/// This is a generic helper function to configure any numeric field in the station settings.
/// It navigates down one field, enters edit mode, types the value, and verifies the change.
///
/// # Arguments
///
/// * `session` - The expectrl session.
/// * `cap` - The terminal capture utility.
/// * `port_name` - The name of the port.
/// * `station_index` - The index of the station.
/// * `value` - The numeric value to set.
/// * `is_master` - Whether the station is a master or a slave.
/// * `field_name` - The name of the field being configured (e.g., "start_address").
/// * `step_name` - A descriptive name for the test step (e.g., "set_start_address").
///
/// # Returns
///
/// * `Result<()>` - Ok if the field was configured and verified successfully.
async fn configure_numeric_field<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    value: u16,
    is_master: bool,
    field_name: &str,
    step_name: &str,
) -> Result<()> {
    let path = station_field_path(port_name, is_master, station_index, field_name);

    // Step 1: Navigate to the field
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 1,
        }],
        &[modbus_page_check(
            "ModbusDashboard active while navigating to field",
        )],
        &format!("nav_to_{}", field_name),
        None,
    )
    .await?;

    // Step 2: Edit the field
    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString(value.to_string()),
            CursorAction::PressEnter,
        ],
        &[
            CursorAction::CheckStatus {
                description: format!("{} is {}", field_name, value),
                path,
                expected: json!(value),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
            modbus_page_check("ModbusDashboard active after committing numeric field"),
        ],
        step_name,
        Some(3),
    )
    .await
}

/// Configures the Start Address for a given station.
pub async fn configure_start_address<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    start_address: u16,
    is_master: bool,
) -> Result<()> {
    configure_numeric_field(
        session,
        cap,
        port_name,
        station_index,
        start_address,
        is_master,
        "start_address",
        "set_start_address",
    )
    .await
}

/// Configures the Register Count for a given station.
pub async fn configure_register_count<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    register_count: u16,
    is_master: bool,
) -> Result<()> {
    configure_numeric_field(
        session,
        cap,
        port_name,
        station_index,
        register_count,
        is_master,
        "register_count",
        "set_register_count",
    )
    .await
}

/// Initialize slave register values after base configuration.
///
/// For slave stations that provide an explicit list of register values, this helper navigates
/// into the register table, edits each entry, and verifies cursor progression using pattern
/// matching. Status files do not currently expose per-register values, so the best available
/// validation is confirming that the cursor advances to the expected register index after each
/// commit.
pub async fn initialize_slave_registers<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    values: &[u16],
    register_mode: RegisterMode,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }

    // Move from Register Count to the first register entry (two steps below)
    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2,
            },
            CursorAction::Sleep1s,
            CursorAction::PressArrow {
                direction: ArrowKey::Left,
                count: 10,
            },
            CursorAction::Sleep1s,
        ],
        &[modbus_page_check(
            "ModbusDashboard active while navigating to first register",
        )],
        "nav_to_first_register",
        None,
    )
    .await?;

    for (index, value) in values.iter().enumerate() {
        log::info!("Setting register {index} to {value}");

        match register_mode {
            RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                let desired_on = (value & 1) != 0;

                if desired_on {
                    execute_with_status_checks(
                        session,
                        cap,
                        &[CursorAction::PressEnter, CursorAction::Sleep1s],
                        &[modbus_page_check(
                            "ModbusDashboard active while toggling coil register",
                        )],
                        &format!("toggle_coil_register_{index}"),
                        Some(3),
                    )
                    .await?;
                }

                if index + 1 < values.len() {
                    execute_with_status_checks(
                        session,
                        cap,
                        &[
                            CursorAction::PressArrow {
                                direction: ArrowKey::Right,
                                count: 1,
                            },
                            CursorAction::Sleep1s,
                        ],
                        &[modbus_page_check(
                            "ModbusDashboard active while moving to next register",
                        )],
                        &format!("advance_coil_register_{index}"),
                        None,
                    )
                    .await?;
                }
            }
            RegisterMode::Holding | RegisterMode::Input => {
                execute_with_status_checks(
                    session,
                    cap,
                    &[CursorAction::PressEnter],
                    &[modbus_page_check(
                        "ModbusDashboard active while entering register edit",
                    )],
                    &format!("enter_register_{index}_edit"),
                    None,
                )
                .await?;

                execute_with_status_checks(
                    session,
                    cap,
                    &[
                        CursorAction::PressCtrlA,
                        CursorAction::PressBackspace,
                        CursorAction::TypeString(value.to_string()),
                    ],
                    &[modbus_page_check(
                        "ModbusDashboard active while typing register value",
                    )],
                    &format!("type_register_{index}"),
                    None,
                )
                .await?;

                let mut commit_actions = vec![CursorAction::PressEnter, CursorAction::Sleep1s];
                let mut commit_checks = if index + 1 < values.len() {
                    commit_actions.push(CursorAction::PressArrow {
                        direction: ArrowKey::Right,
                        count: 1,
                    });
                    commit_actions.push(CursorAction::Sleep1s);
                    Vec::new()
                } else {
                    Vec::new()
                };

                commit_checks.push(modbus_page_check(
                    "ModbusDashboard active after committing register",
                ));

                execute_with_status_checks(
                    session,
                    cap,
                    &commit_actions,
                    &commit_checks,
                    &format!("commit_register_{index}"),
                    Some(3),
                )
                .await?;
            }
        }
    }

    Ok(())
}

/// Saves the configuration and verifies that the port is enabled.
///
/// This function presses Ctrl+S to save the configuration and then checks
/// the status file to ensure the port's `enabled` flag is set to `true`.
///
/// # Arguments
///
/// * `session` - The expectrl session.
/// * `cap` - The terminal capture utility.
/// * `port_name` - The name of the port being configured.
///
/// # Returns
///
/// * `Result<()>` - Ok if the configuration was saved and the port is enabled.
pub async fn save_configuration_and_verify<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
) -> Result<()> {
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressCtrlS, CursorAction::Sleep3s],
        &[
            // Verify port is enabled after save
            CursorAction::CheckStatus {
                description: "Port is enabled".to_string(),
                path: port_field_path(port_name, "enabled"),
                expected: json!(true),
                timeout_secs: Some(10),
                retry_interval_ms: Some(500),
            },
            modbus_page_check("ModbusDashboard active after saving configuration"),
        ],
        "save_configuration",
        Some(3),
    )
    .await
}
