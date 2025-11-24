use anyhow::Result;
use chrono::{DateTime, Local};

use crate::tui::{status::port::PortStatusIndicator, utils::bus::CoreToUi};

/// Check and update temporary statuses (AppliedSuccess) that should auto-transition
/// after a certain time period. Returns true if any status was updated.
pub(crate) fn check_and_update_temporary_statuses(
    core_tx: Option<&flume::Sender<CoreToUi>>,
) -> Result<bool> {
    check_and_update_temporary_statuses_with_now(core_tx, Local::now())
}

/// Internal variant that accepts a deterministic `now` for testing.
pub(crate) fn check_and_update_temporary_statuses_with_now(
    core_tx: Option<&flume::Sender<CoreToUi>>,
    now: DateTime<Local>,
) -> Result<bool> {
    let mut ports_to_update: Vec<(String, PortStatusIndicator)> = Vec::new();

    crate::tui::status::read_status(|status| {
        for (port_name, port) in &status.ports.map {
            let should_update = match &port.status_indicator {
                PortStatusIndicator::AppliedSuccess { timestamp } => {
                    let elapsed = now.signed_duration_since(*timestamp);
                    elapsed.num_seconds() >= 3
                }
                _ => false,
            };

            if should_update {
                let next_status = if port.state.is_occupied_by_this() {
                    PortStatusIndicator::Running
                } else {
                    PortStatusIndicator::NotStarted
                };

                ports_to_update.push((port_name.clone(), next_status));
            }
        }
        Ok(())
    })?;

    let updated = !ports_to_update.is_empty();

    if updated {
        // CRITICAL: Update status FIRST and release the write lock BEFORE sending messages
        // to avoid deadlock when UI thread tries to acquire read lock
        crate::tui::status::write_status(move |status| {
            for (port_name, next_status) in &ports_to_update {
                if let Some(port) = status.ports.map.get_mut(port_name) {
                    let old_status = format!("{:?}", port.status_indicator);
                    port.status_indicator = next_status.clone();

                }
            }
            Ok(())
        })?;
        // Write lock is now released here ^^^

        // NOW send refresh message (after releasing the write lock)
        if let Some(tx) = core_tx {
            if let Err(err) = tx.send(CoreToUi::Refreshed) {
                log::warn!("Failed to send Refreshed after status auto-transition: {err}");
            }
        }
    }

    Ok(updated)
}

pub(crate) fn log_state_snapshot() -> Result<()> {
    use crate::tui::status::port::PortState;
    use serde_json::json;

    crate::tui::status::read_status(|status| {
        let page_name = match &status.page {
            crate::tui::status::Page::Entry { .. } => "Entry",
            crate::tui::status::Page::ConfigPanel { .. } => "ConfigPanel",
            crate::tui::status::Page::ModbusDashboard { .. } => "ModbusDashboard",
            crate::tui::status::Page::LogPanel { .. } => "LogPanel",
            crate::tui::status::Page::About { .. } => "About",
        };

        let cursor_info = match &status.page {
            crate::tui::status::Page::Entry { cursor, .. } => {
                if let Some(c) = cursor {
                    format!("{c:?}")
                } else {
                    "None".to_string()
                }
            }
            crate::tui::status::Page::ConfigPanel { cursor, .. } => format!("{cursor:?}"),
            crate::tui::status::Page::ModbusDashboard { cursor, .. } => format!("{cursor:?}"),
            _ => "N/A".to_string(),
        };

        let mut port_states = vec![];
        for port_name in &status.ports.order {
            if let Some(port_arc) = status.ports.map.get(port_name) {
                let port = port_arc;
                let state_str = match &port.state {
                    PortState::Free => "Free",
                    PortState::OccupiedByThis => "OccupiedByThis",
                    PortState::OccupiedByOther => "OccupiedByOther",
                };
                port_states.push(
                    json!({ "name": port_name, "state": state_str, "type": &port.port_type }),
                );
            }
        }

        let config_edit = json!({
            "active": status.temporarily.config_edit.active,
            "port": status.temporarily.config_edit.port,
            "field_index": status.temporarily.config_edit.field_index,
            "field_key": status.temporarily.config_edit.field_key,
            "buffer": status.temporarily.config_edit.buffer,
            "cursor_pos": status.temporarily.config_edit.cursor_pos,
        });

        let snapshot = json!({
            "page": page_name,
            "cursor": cursor_info,
            "ports": port_states,
            "config_edit": config_edit,
            "error": status.temporarily.error.as_ref().map(|e| &e.message),
        });

        log::info!("STATE_DUMP: {snapshot}");
        Ok(())
    })
}
