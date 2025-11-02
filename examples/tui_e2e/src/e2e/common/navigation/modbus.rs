use anyhow::Result;

use expectrl::Expect;

use super::super::retry::execute_transaction_with_retry;
use aoba_ci_utils::*;

/// Navigate from ConfigPanel to the Modbus dashboard for a specific port.
///
/// This helper is the second phase after [`super::setup::setup_tui_test`]: it enters
/// the configuration panel, selects the requested port, and confirms that the
/// Modbus dashboard is active. All navigation steps use the shared transaction
/// retry helpers so failures capture a snapshot before returning an error.
pub async fn navigate_to_modbus_panel<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
) -> Result<()> {
    log::info!("🗺️  Navigating to port {port1} and entering Modbus panel...");

    execute_transaction_with_retry(
        session,
        cap,
        "entry_to_config_panel",
        &[CursorAction::PressEnter, CursorAction::Sleep3s],
        |_| {
            if let Ok(status) = read_tui_status() {
                match status.page {
                    TuiPage::ConfigPanel => Ok(true),
                    TuiPage::Entry => Ok(false),
                    _ => Ok(false),
                }
            } else {
                Ok(false)
            }
        },
        Some(&[]),
        &[],
    )
    .await?;

    navigate_to_vcom(session, cap, port1).await?;

    execute_transaction_with_retry(
        session,
        cap,
        "enter_modbus_panel",
        &[CursorAction::PressEnter, CursorAction::Sleep3s],
        |screen| {
            if screen.contains("Station") || screen.contains("Create") {
                Ok(true)
            } else if screen.contains("Serial") {
                Ok(false)
            } else {
                Ok(false)
            }
        },
        Some(&[CursorAction::PressEscape, CursorAction::Sleep1s]),
        &[CursorAction::Sleep1s],
    )
    .await?;

    wait_for_tui_page("ModbusDashboard", 10, None).await?;

    // Exit any edit mode and reset cursor to top of panel
    execute_cursor_actions(
        session,
        cap,
        &[
            CursorAction::PressEscape,   // Exit edit mode if active
            CursorAction::Sleep1s,
            CursorAction::PressCtrlPageUp,  // Reset to top
            CursorAction::Sleep1s,
        ],
        "reset_cursor_to_top",
    )
    .await?;

    log::info!("✅ Successfully entered Modbus panel");
    Ok(())
}
