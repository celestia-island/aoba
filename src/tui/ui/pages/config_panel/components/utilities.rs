use anyhow::Result;
use std::sync::{Arc, RwLock};

use crate::protocol::status::{read_status, types, with_port_read};

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    match read_status(|status| Ok(status.page.clone()))? {
        types::Page::ConfigPanel { cursor, .. } => Ok(cursor),
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
    }
}

/// Helper: whether a port is occupied by this instance
pub fn is_port_occupied_by_this(port_data: Option<&Arc<RwLock<types::port::PortData>>>) -> bool {
    if let Some(port) = port_data {
        if let Some(v) = with_port_read(port, |port| {
            matches!(&port.state, types::port::PortState::OccupiedByThis { .. })
        }) {
            return v;
        }
    }
    false
}

/// Get serial parameter value by cursor type
pub fn get_serial_param_value_by_cursor(
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> String {
    if let Some(port) = port_data {
        if let Some(s) = with_port_read(port, |port| {
            if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &port.state {
                match cursor_type {
                    types::cursor::ConfigPanelCursor::BaudRate => {
                        return runtime.current_cfg.baud.to_string()
                    }
                    types::cursor::ConfigPanelCursor::DataBits { .. } => {
                        return runtime.current_cfg.data_bits.to_string()
                    }
                    types::cursor::ConfigPanelCursor::Parity => {
                        return format!("{:?}", runtime.current_cfg.parity)
                    }
                    types::cursor::ConfigPanelCursor::StopBits => {
                        return runtime.current_cfg.stop_bits.to_string()
                    }
                    _ => return "??".to_string(),
                }
            }
            "??".to_string()
        }) {
            return s;
        } else {
            log::warn!("get_serial_param_value_by_cursor: failed to acquire read lock");
        }
    }

    "??".to_string()
}
