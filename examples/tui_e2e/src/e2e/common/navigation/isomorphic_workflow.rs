/// Isomorphic screenshot workflow - same code works in both generation and verification modes
///
/// Key principle: During screenshot generation, keyboard actions are skipped and only
/// state prediction occurs. During verification, keyboard actions execute and states
/// are verified. This "isomorphic" pattern allows the same workflow to describe both
/// processes.
use anyhow::Result;
use expectrl::Expect;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use super::super::{
    config::StationConfig,
    state_helpers::{
        add_master_station, add_slave_station, create_modbus_dashboard_state, enable_port,
        update_register_value,
    },
    station::{
        configure_register_count, configure_register_type, configure_start_address,
        configure_station_id, create_station, ensure_connection_mode, focus_create_station_button,
        focus_station,
    },
};
use aoba_ci_utils::*;

use crate::e2e::common::config::RegisterMode;

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

/// Isomorphic workflow: Configure stations with screenshots
///
/// In generation mode: Only predicts states and generates screenshots
/// In normal mode: Executes keyboard actions and verifies screenshots
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
    let is_generation_mode = screenshot_ctx.mode() == ExecutionMode::GenerateScreenshots;

    // Step 1: Switch connection mode (Master/Slave)
    if !is_generation_mode {
        ensure_connection_mode(session, cap, is_master).await?;
    }
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

    // Step 2: Create all stations
    let mut station_indices = Vec::new();
    for (idx, _config) in configs.iter().enumerate() {
        if !is_generation_mode {
            let station_index = create_station(session, cap, port_name, is_master).await?;
            station_indices.push(station_index);
        } else {
            // In generation mode, predict station indices
            station_indices.push(idx);
        }

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

    // Step 3: Configure each station
    for (idx, config) in configs.iter().enumerate() {
        let station_index = station_indices[idx];

        // Navigate to station
        if !is_generation_mode {
            focus_station(session, cap, port_name, station_index, is_master).await?;
        }
        let state = create_state_with_stations(port_name, &configs[..=idx], is_master);
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state,
                &format!("navigate_station_{}", idx + 1),
            )
            .await?;

        // Navigate to Station ID field
        if !is_generation_mode {
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
        }

        // Configure Station ID
        if !is_generation_mode {
            configure_station_id(
                session,
                cap,
                port_name,
                station_index,
                config.station_id(),
                is_master,
            )
            .await?;
        }
        let mut state = create_state_with_stations(port_name, configs, is_master);
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_id", idx + 1),
            )
            .await?;

        // Navigate to Register Type field
        if !is_generation_mode {
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
        }

        // Configure Register Type
        if !is_generation_mode {
            configure_register_type(
                session,
                cap,
                port_name,
                station_index,
                config.register_mode(),
                is_master,
            )
            .await?;
        }
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_reg_type", idx + 1),
            )
            .await?;

        // Navigate to Start Address field
        if !is_generation_mode {
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
        }

        // Configure Start Address
        if !is_generation_mode {
            configure_start_address(
                session,
                cap,
                port_name,
                station_index,
                config.start_address(),
                is_master,
            )
            .await?;
        }
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_start_addr", idx + 1),
            )
            .await?;

        // Navigate to Register Count field
        if !is_generation_mode {
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
        }

        // Configure Register Count
        if !is_generation_mode {
            configure_register_count(
                session,
                cap,
                port_name,
                station_index,
                config.register_count(),
                is_master,
            )
            .await?;
        }
        screenshot_ctx
            .capture_or_verify(
                session,
                cap,
                state.clone(),
                &format!("edit_station_{}_reg_count", idx + 1),
            )
            .await?;

        // Step 3.5: Navigate to and edit registers with placeholder values
        // After configuring register count, cursor automatically moves to register grid
        if !is_generation_mode {
            execute_cursor_actions(
                session,
                cap,
                &[CursorAction::Sleep1s],
                "wait_for_register_grid",
            )
            .await?;
        }

        // Edit 10 registers per station for single-station tests
        // (For multi-station tests, we'll handle more registers)
        let num_registers_to_edit = std::cmp::min(10, config.register_count() as usize);

        // For Holding/Input register types, create random number array
        // Use deterministic seed for reproducibility: based on port name and station index
        let random_values: Vec<u16> = if matches!(config.register_mode(), RegisterMode::Holding | RegisterMode::Input) {
            let seed_str = format!("{}_{}_{}", port_name, idx, config.register_mode() as u8);
            let seed = seed_str.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let mut rng = StdRng::seed_from_u64(seed);
            
            (0..num_registers_to_edit)
                .map(|_| rng.gen_range(1000..60000))  // Generate random values avoiding 0x0000
                .collect()
        } else {
            vec![]
        };

        for reg_idx in 0..num_registers_to_edit {
            let (value, placeholder) = match config.register_mode() {
                RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                    (0x0000, PlaceholderValue::Boolean(false))
                }
                RegisterMode::Holding | RegisterMode::Input => {
                    let random_val = random_values[reg_idx];
                    // Use Hex placeholder for Holding/Input types since TUI displays in hex format
                    (random_val, PlaceholderValue::Hex(random_val))
                }
            };

            // Register placeholder immediately so numbering tracks MatchScreenCapture order
            register_placeholder_values(&[placeholder]);

            // Navigate to register (cursor should already be on first register after count config)
            if reg_idx > 0 && !is_generation_mode {
                execute_cursor_actions(
                    session,
                    cap,
                    &[
                        CursorAction::PressArrow {
                            direction: ArrowKey::Right,
                            count: 1,
                        },
                        CursorAction::Sleep1s,
                    ],
                    &format!("move_to_register_{}", reg_idx),
                )
                .await?;
            }

            // Get the value for this register
            // Edit register value (only in normal mode)
            if !is_generation_mode {
                execute_cursor_actions(
                    session,
                    cap,
                    &[
                        CursorAction::PressEnter,
                        CursorAction::Sleep1s,
                        CursorAction::TypeString(format!("{:04x}", value)),
                        CursorAction::PressEnter,
                        CursorAction::Sleep1s,
                    ],
                    &format!("edit_register_{}", reg_idx),
                )
                .await?;
            }

            // Update state with register value
            state = update_register_value(state.clone(), station_index, reg_idx, value, is_master);

            // Capture screenshot (placeholder system will handle the replacement automatically)
            // Note: Placeholders were already registered above, so numbering is sequential
            screenshot_ctx
                .capture_or_verify(
                    session,
                    cap,
                    state.clone(),
                    &format!("edit_station_{}_register_{}", idx + 1, reg_idx),
                )
                .await?;
        }

        // Return to top for next station
        if !is_generation_mode {
            focus_create_station_button(session, cap).await?;
        }
    }

    // Step 4: Save configuration
    if !is_generation_mode {
        execute_cursor_actions(
            session,
            cap,
            &[CursorAction::PressCtrlS, CursorAction::Sleep3s],
            "save_configuration",
        )
        .await?;
    }
    let state = create_state_with_stations(port_name, configs, is_master);
    screenshot_ctx
        .capture_or_verify(session, cap, state.clone(), "after_save")
        .await?;

    // Step 5: Wait for port enabled
    if !is_generation_mode {
        wait_for_port_enabled(port_name, 20, Some(500)).await?;
    }
    let state = enable_port(state);
    screenshot_ctx
        .capture_or_verify(session, cap, state, "port_enabled")
        .await?;

    Ok(())
}
