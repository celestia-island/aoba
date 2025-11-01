use anyhow::{anyhow, Result};
use expectrl::Expect;
use regex::Regex;

use super::focus::focus_create_station_button;
use super::modbus_page_check;
use aoba_ci_utils::{
    execute_with_status_checks, ArrowKey, CursorAction, ExpectSession, ScreenAssertion,
    ScreenPatternSpec, TerminalCapture,
};

const MAX_TOGGLE_ATTEMPTS: usize = 3;

/// Ensure the Modbus connection mode matches the desired role before creating a station.
pub async fn ensure_connection_mode<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    is_master: bool,
) -> Result<()> {
    let connection_mode_focus_pattern = Regex::new(r">\s*Connection Mode")?;
    let master_display_pattern = Regex::new(r"(?s)Connection Mode.*Master")?;
    let connection_mode_edit_pattern = Regex::new(r"(?s)Connection Mode.*<")?;
    let connection_mode_edit_slave_pattern = Regex::new(r"(?s)Connection Mode.*Slave")?;
    let connection_mode_slave_pattern = Regex::new(r"(?s)Connection Mode.*Slave")?;

    focus_create_station_button(session, cap).await?;

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
        &[modbus_page_check(
            "ModbusDashboard active while focusing Connection Mode",
        )],
        &[ScreenAssertion::pattern(ScreenPatternSpec::new(
            connection_mode_focus_pattern,
            "Cursor moved to Connection Mode",
        ))],
        "focus_connection_mode",
        Some(3),
    )
    .await?;

    if is_master {
        execute_with_status_checks(
            session,
            cap,
            &[CursorAction::Sleep1s],
            &[modbus_page_check(
                "ModbusDashboard active while verifying Master mode",
            )],
            &[ScreenAssertion::pattern(ScreenPatternSpec::new(
                master_display_pattern,
                "Connection mode remains Master",
            ))],
            "verify_master_connection_mode",
            Some(3),
        )
        .await?;

        focus_create_station_button(session, cap).await?;
        return Ok(());
    }

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressEnter, CursorAction::Sleep1s],
        &[modbus_page_check(
            "ModbusDashboard active in Connection Mode edit",
        )],
        &[ScreenAssertion::pattern(ScreenPatternSpec::new(
            connection_mode_edit_pattern.clone(),
            "Connection mode selector opened",
        ))],
        "enter_connection_mode_edit",
        Some(3),
    )
    .await?;

    let toggle_directions = [ArrowKey::Right, ArrowKey::Left, ArrowKey::Right];
    let mut last_error: Option<anyhow::Error> = None;

    for (attempt, direction) in toggle_directions.iter().enumerate() {
        let result = execute_with_status_checks(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: *direction,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            &[modbus_page_check(
                "ModbusDashboard active while selecting Slave mode",
            )],
            &[ScreenAssertion::pattern(ScreenPatternSpec::new(
                connection_mode_edit_slave_pattern.clone(),
                "Connection mode selector shows Slave",
            ))],
            &format!("toggle_connection_mode_attempt_{}", attempt + 1),
            Some(3),
        )
        .await;

        match result {
            Ok(()) => {
                execute_with_status_checks(
                    session,
                    cap,
                    &[CursorAction::PressEnter, CursorAction::Sleep1s],
                    &[modbus_page_check(
                        "ModbusDashboard active after confirming Slave mode",
                    )],
                    &[ScreenAssertion::pattern(ScreenPatternSpec::new(
                        connection_mode_slave_pattern.clone(),
                        "Connection mode confirms Slave",
                    ))],
                    "confirm_slave_connection_mode",
                    Some(3),
                )
                .await?;

                focus_create_station_button(session, cap).await?;
                return Ok(());
            }
            Err(err) => {
                log::warn!(
                    "⚠️  Failed to toggle Connection Mode on attempt {} using {:?}: {}",
                    attempt + 1,
                    direction,
                    err
                );
                last_error = Some(err);
            }
        }
    }

    let detail = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "no attempts executed".to_string());

    Err(anyhow!(
        "Failed to set Connection Mode to Slave after {} attempts: {}",
        MAX_TOGGLE_ATTEMPTS,
        detail
    ))
}
