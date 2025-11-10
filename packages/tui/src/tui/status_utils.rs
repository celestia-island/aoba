use anyhow::Result;
use chrono::Local;

use crate::tui::{status::port::PortStatusIndicator, utils::bus::CoreToUi};

/// Check and update temporary statuses (AppliedSuccess, StartupFailed) that should auto-transition
/// after a certain time period. Returns true if any status was updated.
pub(crate) fn check_and_update_temporary_statuses(
    core_tx: Option<&flume::Sender<CoreToUi>>,
) -> Result<bool> {
    let now = Local::now();
    let mut ports_to_update: Vec<(String, PortStatusIndicator)> = Vec::new();

    crate::tui::status::read_status(|status| {
        for (port_name, port) in &status.ports.map {
            let should_update = match &port.status_indicator {
                PortStatusIndicator::AppliedSuccess { timestamp } => {
                    let elapsed = now.signed_duration_since(*timestamp);
                    elapsed.num_seconds() >= 3
                }
                PortStatusIndicator::StartupFailed { timestamp, .. } => {
                    let elapsed = now.signed_duration_since(*timestamp);
                    elapsed.num_seconds() >= 10
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
                    log::debug!(
                        "Auto-transitioned {port_name} status from {old_status} to {:?}",
                        port.status_indicator
                    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::status::{
        port::{PortData, PortState, PortStatusIndicator},
        Status,
    };
    use chrono::Local;
    use parking_lot::RwLock;
    use std::sync::Arc;

    fn setup_test_env() {
        let status = Arc::new(RwLock::new(Status::default()));
        // Try to initialize, but ignore error if already initialized
        let _ = crate::tui::status::init_status(status.clone());
    }

    fn cleanup_test_port(port_name: &str) {
        let _ = crate::tui::status::write_status(|status| {
            status.ports.map.remove(port_name);
            status.ports.order.retain(|p| p != port_name);
            Ok(())
        });
    }

    #[test]
    fn test_applied_success_auto_transition() {
        setup_test_env();
        let test_port = "/tmp/test_port_1";
        cleanup_test_port(test_port);

        // Add a test port with AppliedSuccess status from 5 seconds ago
        let old_timestamp = Local::now() - chrono::Duration::seconds(5);
        crate::tui::status::write_status(|status| {
            let port = PortData {
                port_name: test_port.to_string(),
                state: PortState::OccupiedByThis,
                status_indicator: PortStatusIndicator::AppliedSuccess {
                    timestamp: old_timestamp,
                },
                config_modified: false,
                ..Default::default()
            };

            status.ports.map.insert(test_port.to_string(), port);
            status.ports.order.push(test_port.to_string());
            Ok(())
        })
        .unwrap();

        // Check and update
        let updated = check_and_update_temporary_statuses(None).unwrap();
        assert!(updated, "Status should have been updated");

        // Verify transition to Running
        crate::tui::status::read_status(|status| {
            let port = status.ports.map.get(test_port).unwrap();
            assert!(
                matches!(port.status_indicator, PortStatusIndicator::Running),
                "Status should transition to Running"
            );
            Ok(())
        })
        .unwrap();

        cleanup_test_port(test_port);
    }

    #[test]
    fn test_applied_success_with_changes_transition() {
        setup_test_env();
        let test_port = "/tmp/test_port_2";
        cleanup_test_port(test_port);

        // Add a test port with AppliedSuccess status and config_modified = true
        let old_timestamp = Local::now() - chrono::Duration::seconds(5);
        crate::tui::status::write_status(|status| {
            let port = PortData {
                port_name: test_port.to_string(),
                state: PortState::OccupiedByThis,
                status_indicator: PortStatusIndicator::AppliedSuccess {
                    timestamp: old_timestamp,
                },
                config_modified: true, // Port has unsaved changes
                ..Default::default()
            };

            status.ports.map.insert(test_port.to_string(), port);
            status.ports.order.push(test_port.to_string());
            Ok(())
        })
        .unwrap();

        // Check and update
        let updated = check_and_update_temporary_statuses(None).unwrap();
        assert!(updated, "Status should have been updated");

        // Verify transition to Running (no longer RunningWithChanges)
        crate::tui::status::read_status(|status| {
            let port = status.ports.map.get(test_port).unwrap();
            assert!(
                matches!(port.status_indicator, PortStatusIndicator::Running),
                "Status should transition to Running"
            );
            Ok(())
        })
        .unwrap();

        cleanup_test_port(test_port);
    }

    #[test]
    fn test_startup_failed_auto_transition() {
        setup_test_env();
        let test_port = "/tmp/test_port_3";
        cleanup_test_port(test_port);

        // Add a test port with StartupFailed status from 15 seconds ago
        let old_timestamp = Local::now() - chrono::Duration::seconds(15);
        crate::tui::status::write_status(|status| {
            let port = PortData {
                port_name: test_port.to_string(),
                state: PortState::Free,
                status_indicator: PortStatusIndicator::StartupFailed {
                    error_message: "Test error".to_string(),
                    timestamp: old_timestamp,
                },
                config_modified: false,
                ..Default::default()
            };

            status.ports.map.insert(test_port.to_string(), port);
            status.ports.order.push(test_port.to_string());
            Ok(())
        })
        .unwrap();

        // Check and update
        let updated = check_and_update_temporary_statuses(None).unwrap();
        assert!(updated, "Status should have been updated");

        // Verify transition to NotStarted
        crate::tui::status::read_status(|status| {
            let port = status.ports.map.get(test_port).unwrap();
            assert!(
                matches!(port.status_indicator, PortStatusIndicator::NotStarted),
                "Status should transition to NotStarted"
            );
            Ok(())
        })
        .unwrap();

        cleanup_test_port(test_port);
    }

    #[test]
    fn test_no_transition_if_not_expired() {
        setup_test_env();
        let test_port = "/tmp/test_port_4";
        cleanup_test_port(test_port);

        // Add a test port with AppliedSuccess status from 1 second ago (not expired)
        let recent_timestamp = Local::now() - chrono::Duration::seconds(1);
        crate::tui::status::write_status(|status| {
            let port = PortData {
                port_name: test_port.to_string(),
                state: PortState::OccupiedByThis,
                status_indicator: PortStatusIndicator::AppliedSuccess {
                    timestamp: recent_timestamp,
                },
                config_modified: false,
                ..Default::default()
            };

            status.ports.map.insert(test_port.to_string(), port);
            status.ports.order.push(test_port.to_string());
            Ok(())
        })
        .unwrap();

        // Check and update
        let updated = check_and_update_temporary_statuses(None).unwrap();
        assert!(!updated, "Status should not have been updated");

        // Verify status is still AppliedSuccess
        crate::tui::status::read_status(|status| {
            let port = status.ports.map.get(test_port).unwrap();
            assert!(
                matches!(
                    port.status_indicator,
                    PortStatusIndicator::AppliedSuccess { .. }
                ),
                "Status should still be AppliedSuccess"
            );
            Ok(())
        })
        .unwrap();

        cleanup_test_port(test_port);
    }
}
