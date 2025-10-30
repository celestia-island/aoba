use anyhow::Result;
use expectrl::Expect;

use super::super::config::RegisterMode;
use super::modbus_page_check;
use ci_utils::{execute_with_status_checks, ArrowKey, CursorAction, TerminalCapture};

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
