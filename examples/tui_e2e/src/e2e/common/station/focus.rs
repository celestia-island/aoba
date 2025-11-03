use super::modbus_page_check;
use anyhow::Result;
use aoba_ci_utils::{
    execute_with_status_checks, CursorAction, ExpectSession, ScreenAssertion, ScreenPatternSpec,
    TerminalCapture,
};
use expectrl::Expect;
use regex::Regex;

/// Ensure the cursor is focused on the "Create Station" button at the top of the dashboard.
pub async fn focus_create_station_button<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    let pattern = Regex::new(r">\s*Create Station")?;

    execute_with_status_checks(
        session,
        cap,
        &[CursorAction::PressCtrlPageUp, CursorAction::Sleep1s],
        &[modbus_page_check(
            "ModbusDashboard active while focusing Create Station",
        )],
        &[ScreenAssertion::pattern(
            ScreenPatternSpec::new(pattern, "Cursor positioned on Create Station")
                .with_retry_action(Some(vec![
                    CursorAction::PressEscape,
                    CursorAction::Sleep1s,
                    CursorAction::PressCtrlPageUp,
                    CursorAction::Sleep1s,
                ])),
        )],
        "focus_create_station_button",
        Some(3),
    )
    .await
}

/// Move the cursor focus to the specified station section.
pub async fn focus_station<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    _port_name: &str,
    station_index: usize,
    _is_master: bool,
) -> Result<()> {
    let mut actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep1s];
    actions.push(CursorAction::PressPageDown);
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
            // Remove CheckStatus - rely on screenshot verification
            modbus_page_check("ModbusDashboard active while focusing station"),
        ],
        &[],
        &format!("focus_station_{}", station_index + 1),
        Some(3),
    )
    .await
}
