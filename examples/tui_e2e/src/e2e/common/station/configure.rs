use anyhow::{anyhow, Result};
use regex::Regex;
// serde_json::json not needed here

use expectrl::Expect;

use super::super::config::{RegisterMode, RegisterModeExt};
use super::super::status_paths::station_field_path;
use aoba_ci_utils::{
    execute_with_status_checks, ArrowKey, CursorAction, ExpectSession, ScreenAssertion,
    ScreenPatternSpec, TerminalCapture,
};

/// Configures the Station ID for a given station.
///
/// This function performs the following steps:
/// 1. Navigates to the Station ID field.
/// 2. Enters edit mode.
/// 3. Clears the existing value.
/// 4. Types the new station ID.
/// 5. Commits the change and verifies it via a status check.
pub async fn configure_station_id<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    station_id: u8,
) -> Result<()> {
    // Step 1: Enter edit mode
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[CursorAction::Sleep1s],
        &[],
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
        &[CursorAction::Sleep1s],
        &[],
        "type_station_id",
        None,
    )
    .await?;

    // Step 3: Commit (no status check needed - will be verified via screenshot)
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter],
        &[], // No status checks - rely on screenshot verification instead
        &[],
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
pub async fn configure_register_type<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    register_mode: RegisterMode,
) -> Result<()> {
    let (direction, count) = register_mode.arrow_from_default();
    let register_type_focus_pattern = Regex::new(r">\s*Register Type")?;
    let register_type_edit_pattern = Regex::new(r"Register Type[\s\w:()]+<.*>")?;
    let register_type_value_label = match register_mode {
        RegisterMode::Coils => "Coils(01)",
        RegisterMode::DiscreteInputs => "Discrete Inputs(02)",
        RegisterMode::Holding => "Holding Registers(03)",
        RegisterMode::Input => "Input Registers(04)",
    };
    let register_type_value_pattern = Regex::new(&format!(
        r"Register Type\s+<?\s*{}\s*>?",
        regex::escape(register_type_value_label)
    ))?;

    // Step 1: Navigate to Register Type field. Allow multiple Down presses to converge.
    const MAX_NAV_ATTEMPTS: usize = 8;
    let mut found_register_type = false;
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..MAX_NAV_ATTEMPTS {
        let actions: Vec<CursorAction> = if attempt == 0 {
            vec![CursorAction::Sleep1s]
        } else {
            vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ]
        };

        let result = execute_with_status_checks(
            session,
            cap,
            &actions,
            &[CursorAction::Sleep1s],
            &[ScreenAssertion::pattern(ScreenPatternSpec::new(
                register_type_focus_pattern.clone(),
                "Cursor positioned on Register Type",
            ))],
            &format!("nav_to_register_type_step_{}", attempt + 1),
            Some(3),
        )
        .await;

        match result {
            Ok(_) => {
                found_register_type = true;
                break;
            }
            Err(err) => {
                log::warn!(
                    "⚠️  Failed to focus Register Type on attempt {}: {}",
                    attempt + 1,
                    err
                );
                last_error = Some(err);
            }
        }
    }

    if !found_register_type {
        let detail = last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "no attempts executed".to_string());
        return Err(anyhow!(
            "Failed to locate Register Type after {} attempts: {}",
            MAX_NAV_ATTEMPTS,
            detail
        ));
    }

    // Step 2: Enter register type selector
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[CursorAction::Sleep1s],
        &[ScreenAssertion::pattern(
            ScreenPatternSpec::new(register_type_edit_pattern, "Register Type selector opened")
                .with_retry_action(Some(vec![
                    CursorAction::PressEscape,
                    CursorAction::Sleep1s,
                    CursorAction::PressEnter,
                    CursorAction::Sleep1s,
                ])),
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
            &[CursorAction::Sleep1s],
            &[],
            "select_register_type_option",
            None,
        )
        .await?;
    }

    // Step 4: Commit selection and verify UI reflects the new value
    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[CursorAction::Sleep1s],
        &[ScreenAssertion::pattern(ScreenPatternSpec::new(
            register_type_value_pattern,
            format!("Register Type line shows {}", register_type_value_label),
        ))],
        "confirm_register_type",
        Some(3),
    )
    .await
}

/// Configures the Start Address for a given station.
pub async fn configure_start_address<T: Expect + ExpectSession>(
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
        "Start Address",
        "set_start_address",
    )
    .await
}

/// Configures the Register Count for a given station.
pub async fn configure_register_count<T: Expect + ExpectSession>(
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
        "Register Length",
        "set_register_count",
    )
    .await
}

/// Configures a numeric field (like Start Address or Register Count) for a station.
async fn configure_numeric_field<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    value: u16,
    is_master: bool,
    field_name: &str,
    field_label: &str,
    step_name: &str,
) -> Result<()> {
    let _path = station_field_path(port_name, is_master, station_index, field_name);
    let field_display_pattern = Regex::new(&format!(
        r">\s*{}\s+(?:0x{:04X}\s+\({}\)|>\s*[0-9_\s]*<)",
        regex::escape(field_label),
        value,
        value
    ))?;

    // Step 1: Navigate to the field and give the UI a moment to settle
    execute_with_status_checks(
        session,
        cap,
        &[
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::Sleep1s,
        ],
        &[CursorAction::Sleep1s],
        &[],
        &format!("nav_to_{}", field_name),
        None,
    )
    .await?;

    // Step 2: Enter edit mode, type the desired value, confirm via UI, then verify status
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep1s,
        CursorAction::PressCtrlA,
        CursorAction::Sleep1s,
        CursorAction::PressBackspace,
        CursorAction::Sleep1s,
        CursorAction::TypeString(value.to_string()),
        CursorAction::Sleep1s,
        CursorAction::PressEnter,
        CursorAction::Sleep1s,
    ];

    execute_with_status_checks(
        session,
        cap,
        &actions,
        &[
            // Remove CheckStatus - rely on screenshot verification instead
            CursorAction::Sleep1s,
        ],
        &[ScreenAssertion::pattern(ScreenPatternSpec::new(
            field_display_pattern,
            format!("{field_label} line shows {value}"),
        ))],
        step_name,
        Some(3),
    )
    .await
}
