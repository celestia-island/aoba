use anyhow::{anyhow, Result};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
        },
        write_status,
    },
    tui::utils::bus::{Bus, UiToCore},
};

pub fn handle_enter_action(bus: &Bus) -> Result<()> {
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            create_new_modbus_entry()?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::ModbusMode { .. }
        | types::cursor::ModbusDashboardCursor::RegisterMode { .. } => {
            let _sel_index = read_status(|_status| {
                if let types::cursor::ModbusDashboardCursor::ModbusMode { index } = current_cursor {
                    Ok(index)
                } else if let types::cursor::ModbusDashboardCursor::RegisterMode { index } = current_cursor {
                    Ok(index)
                } else {
                    Ok(0)
                }
            })?;
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(0);
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. } => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
                    bytes: Vec::new(),
                    offset: 0,
                };
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::Register {
            slave_index: _,
            register_index: _,
        } => {
            let port_name_opt = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    Ok(status.ports.order.get(*selected_port).cloned())
                } else {
                    Ok(None)
                }
            })?;

            if let Some(_port_name) = port_name_opt {
                // Handle register toggle/edit logic here
                // This is a simplified version - the full logic is quite complex
                bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            }
        }
    }
    Ok(())
}

pub fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;
    write_status(|status| {
        status.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
    Ok(())
}

fn create_new_modbus_entry() -> Result<()> {
    // Simplified implementation - just log the action for now
    log::info!("Creating new modbus entry");
    Ok(())
}