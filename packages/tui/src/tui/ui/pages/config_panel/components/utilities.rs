use anyhow::Result;

use crate::tui::status::{read_status, types};

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    match read_status(|status| Ok(status.page.clone()))? {
        crate::tui::status::Page::ConfigPanel { cursor, .. } => Ok(cursor),
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
    }
}

/// Helper: whether a port is occupied by this instance
pub fn is_port_occupied_by_this(port_data: Option<&types::port::PortData>) -> bool {
    if let Some(port) = port_data {
        return port.state.is_occupied_by_this();
    }
    false
}

/// Get serial parameter value by cursor type
pub fn get_serial_param_value_by_cursor(
    port_data: Option<&types::port::PortData>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> String {
    if let Some(port) = port_data {
        match cursor_type {
            types::cursor::ConfigPanelCursor::BaudRate => {
                return port.serial_config.baud.to_string()
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                return port.serial_config.data_bits.to_string()
            }
            types::cursor::ConfigPanelCursor::Parity => {
                return format!("{:?}", port.serial_config.parity)
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                return port.serial_config.stop_bits.to_string()
            }
            _ => return "??".to_string(),
        }
    }

    "??".to_string()
}
