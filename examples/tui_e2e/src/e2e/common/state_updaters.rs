//! Shared state updaters for TUI E2E screenshot tests
//!
//! This module provides reusable state update functions that can be shared
//! across different test modules for consistent state predictions.

use aoba_ci_utils::{E2EPortState, TuiPort, TuiStatus};

/// Add discovered COM ports to Entry state
///
/// This updater adds the virtual serial ports that would be discovered
/// by the TUI on startup, ensuring the Entry page screenshot shows the ports.
pub fn add_discovered_ports(state: &mut TuiStatus, port1: &str, port2: &str) {
    // Add port1
    state.ports.push(TuiPort {
        name: port1.to_string(),
        enabled: false,
        state: E2EPortState::Free,
        modbus_masters: Vec::new(),
        modbus_slaves: Vec::new(),
        log_count: 0,
    });

    // Add port2
    state.ports.push(TuiPort {
        name: port2.to_string(),
        enabled: false,
        state: E2EPortState::Free,
        modbus_masters: Vec::new(),
        modbus_slaves: Vec::new(),
        log_count: 0,
    });
}
