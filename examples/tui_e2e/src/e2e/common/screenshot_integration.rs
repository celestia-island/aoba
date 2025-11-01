//! Screenshot integration helpers for TUI E2E tests
//!
//! This module provides helpers to integrate screenshot capture/verification
//! at key points in the test workflow.

use anyhow::Result;
use aoba_ci_utils::*;

use super::config::RegisterModeExt;
use super::state_helpers::{
    add_master_station, add_slave_station, create_modbus_dashboard_state, enable_port,
};

/// Helper function to apply station configuration to a state
fn apply_station_to_state(
    state: TuiStatus,
    port_name: &str,
    station_id: u8,
    register_mode: aoba::protocol::status::types::modbus::RegisterMode,
    start_address: u16,
    register_count: usize,
    is_master: bool,
) -> TuiStatus {
    let mut state = state;
    let register_type = format!("{:?}", register_mode);
    
    if is_master {
        state = add_master_station(
            state,
            station_id,
            &register_type,
            start_address,
            register_count,
        );
    } else {
        state = add_slave_station(
            state,
            station_id,
            &register_type,
            start_address,
            register_count,
        );
    }
    
    state
}

/// Capture or verify screenshot after navigating to Modbus dashboard
pub async fn screenshot_after_modbus_panel<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    screenshot_ctx: Option<&ScreenshotContext>,
) -> Result<()> {
    if let Some(ctx) = screenshot_ctx {
        let state = create_modbus_dashboard_state(port_name);
        ctx.capture_or_verify(session, cap, state, "modbus_dashboard").await?;
    }
    Ok(())
}

/// Capture or verify screenshot after configuring a station
pub async fn screenshot_after_station_config<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_id: u8,
    register_mode: aoba::protocol::status::types::modbus::RegisterMode,
    start_address: u16,
    register_count: usize,
    is_master: bool,
    screenshot_ctx: Option<&ScreenshotContext>,
) -> Result<()> {
    if let Some(ctx) = screenshot_ctx {
        let state = create_modbus_dashboard_state(port_name);
        let state = apply_station_to_state(
            state,
            port_name,
            station_id,
            register_mode,
            start_address,
            register_count,
            is_master,
        );
        
        ctx.capture_or_verify(session, cap, state, "station_configured").await?;
    }
    Ok(())
}

/// Capture or verify screenshot after enabling port
pub async fn screenshot_after_port_enabled<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_id: u8,
    register_mode: aoba::protocol::status::types::modbus::RegisterMode,
    start_address: u16,
    register_count: usize,
    is_master: bool,
    screenshot_ctx: Option<&ScreenshotContext>,
) -> Result<()> {
    if let Some(ctx) = screenshot_ctx {
        let state = create_modbus_dashboard_state(port_name);
        let state = apply_station_to_state(
            state,
            port_name,
            station_id,
            register_mode,
            start_address,
            register_count,
            is_master,
        );
        let state = enable_port(state);
        
        ctx.capture_or_verify(session, cap, state, "port_enabled").await?;
    }
    Ok(())
}
