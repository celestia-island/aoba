//! Common utilities and state constructors for TUI UI E2E tests
//!
//! This module provides common state constructors that are shared across
//! different test modules, representing the common setup steps before
//! running specific tests.

use aoba::tui::status::types::{
    modbus::{ModbusConnectionMode, ModbusRegisterItem, RegisterMode},
    port::{PortConfig, PortData, PortState},
};
use aoba::tui::status::Status;

/// Create a basic single station master configuration state
/// This represents the common setup where a single station is configured as master
pub fn create_single_station_master_base_state() -> Status {
    let mut status = Status::default();

    // Set page to ModbusDashboard with master mode cursor
    status.page = aoba::tui::status::Page::ModbusDashboard {
        selected_port: 0,
        view_offset: 0,
        cursor: aoba::tui::status::types::cursor::ModbusDashboardCursor::StationId { index: 0 },
    };

    // Add a port with master mode
    let mut port_data = PortData::default();
    port_data.port_name = "/tmp/vcom1".to_string();
    port_data.port_type = "virtual".to_string();
    port_data.state = PortState::OccupiedByThis;
    port_data.config = PortConfig::Modbus {
        mode: ModbusConnectionMode::Master,
        stations: vec![],
    };
    port_data.status_indicator = aoba::tui::status::types::port::PortStatusIndicator::Running;

    status.ports.order.push("/tmp/vcom1".to_string());
    status.ports.map.insert("/tmp/vcom1".to_string(), port_data);

    status
}

/// Create a basic single station slave configuration state
/// This represents the common setup where a single station is configured as slave
pub fn create_single_station_slave_base_state() -> Status {
    let mut status = Status::default();

    // Set page to ModbusDashboard with slave mode cursor
    status.page = aoba::tui::status::Page::ModbusDashboard {
        selected_port: 0,
        view_offset: 0,
        cursor: aoba::tui::status::types::cursor::ModbusDashboardCursor::StationId { index: 0 },
    };

    // Add a port with slave mode
    let mut port_data = PortData::default();
    port_data.port_name = "/tmp/vcom1".to_string();
    port_data.port_type = "virtual".to_string();
    port_data.state = PortState::OccupiedByThis;
    port_data.config = PortConfig::Modbus {
        mode: ModbusConnectionMode::Slave {
            current_request_at_station_index: 0,
        },
        stations: vec![],
    };
    port_data.status_indicator = aoba::tui::status::types::port::PortStatusIndicator::Running;

    status.ports.order.push("/tmp/vcom1".to_string());
    status.ports.map.insert("/tmp/vcom1".to_string(), port_data);

    status
}

/// Create a multi-station master configuration base state
/// This represents the common setup for multi-station master tests
pub fn create_multi_station_master_base_state() -> Status {
    let mut status = Status::default();

    // Set page to ModbusDashboard
    status.page = aoba::tui::status::Page::ModbusDashboard {
        selected_port: 0,
        view_offset: 0,
        cursor: aoba::tui::status::types::cursor::ModbusDashboardCursor::StationId { index: 0 },
    };

    // Add a port with master mode and empty stations (will be populated in specific tests)
    let mut port_data = PortData::default();
    port_data.port_name = "/tmp/vcom1".to_string();
    port_data.port_type = "virtual".to_string();
    port_data.state = PortState::OccupiedByThis;
    port_data.config = PortConfig::Modbus {
        mode: ModbusConnectionMode::Master,
        stations: vec![],
    };
    port_data.status_indicator = aoba::tui::status::types::port::PortStatusIndicator::Running;

    status.ports.order.push("/tmp/vcom1".to_string());
    status.ports.map.insert("/tmp/vcom1".to_string(), port_data);

    status
}

/// Create a multi-station slave configuration base state
/// This represents the common setup for multi-station slave tests
pub fn create_multi_station_slave_base_state() -> Status {
    let mut status = Status::default();

    // Set page to ModbusDashboard
    status.page = aoba::tui::status::Page::ModbusDashboard {
        selected_port: 0,
        view_offset: 0,
        cursor: aoba::tui::status::types::cursor::ModbusDashboardCursor::StationId { index: 0 },
    };

    // Add a port with slave mode and empty stations (will be populated in specific tests)
    let mut port_data = PortData::default();
    port_data.port_name = "/tmp/vcom1".to_string();
    port_data.port_type = "virtual".to_string();
    port_data.state = PortState::OccupiedByThis;
    port_data.config = PortConfig::Modbus {
        mode: ModbusConnectionMode::Slave {
            current_request_at_station_index: 0,
        },
        stations: vec![],
    };
    port_data.status_indicator = aoba::tui::status::types::port::PortStatusIndicator::Running;

    status.ports.order.push("/tmp/vcom1".to_string());
    status.ports.map.insert("/tmp/vcom1".to_string(), port_data);

    status
}

/// Helper to create a ModbusRegisterItem with specific configuration
pub fn create_register_item(
    station_id: u8,
    register_mode: RegisterMode,
    address: u16,
    length: u16,
) -> ModbusRegisterItem {
    ModbusRegisterItem {
        station_id,
        register_mode,
        register_address: address,
        register_length: length,
        last_values: vec![],
        req_success: 0,
        req_total: 0,
        next_poll_at: std::time::Instant::now(),
        last_request_time: None,
        last_response_time: None,
        pending_requests: vec![],
    }
}
