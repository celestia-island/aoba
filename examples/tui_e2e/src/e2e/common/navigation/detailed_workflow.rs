/// Comprehensive screenshot-aware workflow for TUI E2E tests
///
/// This module implements detailed screenshot capture at every atomic operation step,
/// as requested in the feedback. Each function captures state after its operation completes.
use anyhow::{anyhow, Result};
use expectrl::Expect;

use super::super::{
    config::{RegisterMode, RegisterModeExt, StationConfig},
    state_helpers::{
        add_master_station, add_slave_station, create_modbus_dashboard_state, enable_port,
    },
    station::{
        configure_register_count, configure_register_type, configure_start_address,
        configure_station_id, create_station, ensure_connection_mode, focus_create_station_button,
        focus_station,
    },
};
use aoba_ci_utils::*;

/// Helper to create state with N stations configured
fn create_state_with_stations(
    port_name: &str,
    configs: &[StationConfig],
    is_master: bool,
) -> TuiStatus {
    let mut state = create_modbus_dashboard_state(port_name);

    for config in configs {
        let register_type = format!("{:?}", config.register_mode());
        if is_master {
            state = add_master_station(
                state,
                config.station_id(),
                &register_type,
                config.start_address(),
                config.register_count() as usize,
            );
        } else {
            state = add_slave_station(
                state,
                config.station_id(),
                &register_type,
                config.start_address(),
                config.register_count() as usize,
            );
        }
    }

    state
}

/// Configure multiple stations with detailed screenshot capture at every step
pub async fn configure_stations_with_screenshots<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    configs: &[StationConfig],
    screenshot_ctx: &ScreenshotContext,
) -> Result<()> {
    if configs.is_empty() {
        return Ok(());
    }

    let is_master = configs[0].is_master();

    // Screenshot: After switching connection mode
    ensure_connection_mode(session, cap, is_master).await?;
    let state = create_modbus_dashboard_state(port_name);
    screenshot_ctx
        .capture_or_verify(
            session,
            cap,
            state,
            &format!(
                "connection_mode_{}",
                if is_master { "master" } else { "slave" }
            ),
        )
        .await?;

    // Create all stations first, capturing after each creation
    let mut station_indices = Vec::new();
    for (idx, _config) in configs.iter().enumerate() {
        let station_index = create_station(session, cap, port_name, is_master).await?;
        station_indices.push(station_index);

        // Screenshot: After creating each station
        let state = create_state_with_stations(port_name, &configs[..=idx], is_master);
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state,
                &format!("after_create_station_{}", idx + 1),
            )
            .await?;
    }

    // Configure each station with screenshots after every field edit
    for (idx, config) in configs.iter().enumerate() {
        let station_index = station_indices[idx];

        // Screenshot: Navigate to station
        focus_station(session, cap, port_name, station_index, is_master).await?;
        let state = create_state_with_stations(port_name, &configs[..=idx], is_master);
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state,
                &format!("navigate_station_{}", idx + 1),
            )
            .await?;

        // Navigate to Station ID field (Down x1)
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "move_to_station_id",
        )
        .await?;

        // Configure Station ID
        configure_station_id(
            session,
            cap,
            port_name,
            station_index,
            config.station_id(),
            is_master,
        )
        .await?;

        // Screenshot: After editing station ID
        let mut state = create_state_with_stations(port_name, configs, is_master);
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_id", idx + 1),
            )
            .await?;

        // Navigate to Register Type field (Down x1)
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "move_to_register_type",
        )
        .await?;

        // Configure Register Type
        configure_register_type(
            session,
            cap,
            port_name,
            station_index,
            config.register_mode(),
            is_master,
        )
        .await?;

        // Screenshot: After editing register type
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_reg_type", idx + 1),
            )
            .await?;

        // Navigate to Start Address field (Down x1)
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "move_to_start_address",
        )
        .await?;

        // Configure Start Address
        configure_start_address(
            session,
            cap,
            port_name,
            station_index,
            config.start_address(),
            is_master,
        )
        .await?;

        // Screenshot: After editing start address
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_start_addr", idx + 1),
            )
            .await?;

        // Navigate to Register Count field (Down x1)
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "move_to_register_count",
        )
        .await?;

        // Configure Register Count
        configure_register_count(
            session,
            cap,
            port_name,
            station_index,
            config.register_count(),
            is_master,
        )
        .await?;

        // Screenshot: After editing register count
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_reg_count", idx + 1),
            )
            .await?;

        // Return to top for next station
        focus_create_station_button(session, cap).await?;
    }

    // Screenshot: After saving configuration
    execute_cursor_actions(
        session,
        cap,
        &[CursorAction::PressCtrlS, CursorAction::Sleep3s],
        "save_configuration",
    )
    .await?;

    let state = create_state_with_stations(port_name, configs, is_master);
    screenshot_ctx
        .capture_or_verify(session, cap, state.clone(), "after_save")
        .await?;

    // Screenshot: After port enabled
    wait_for_port_enabled(port_name, 20, Some(500)).await?;
    let state = enable_port(state);
    screenshot_ctx
        .capture_or_verify(session, cap, state, "port_enabled")
        .await?;

    Ok(())
}
