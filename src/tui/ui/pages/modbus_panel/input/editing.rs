use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
        },
        write_status,
    },
    tui::{
        ui::components::input_span_handler::handle_input_span,
        utils::bus::{Bus, UiToCore},
    },
};

pub fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

            match &input_raw_buffer {
                types::ui::InputRawBuffer::Index(selected_index) => {
                    commit_selector_edit(current_cursor, *selected_index)?;
                }
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    let value = String::from_utf8_lossy(bytes).to_string();
                    commit_text_edit(current_cursor, value)?;
                }
                _ => {}
            }

            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;

            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        _ => {
            handle_input_span(
                key,
                bus,
                None,
                None,
                |_| true,
                |_| Ok(()),
            )?;
            Ok(())
        }
    }
}

fn commit_selector_edit(cursor: types::cursor::ModbusDashboardCursor, selected_index: usize) -> Result<()> {
    // Handle selector edits (connection mode, register mode)
    match cursor {
        types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
            // Apply connection mode changes
            log::info!("Committing connection mode edit for index {}: {}", index, selected_index);
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            // Apply register mode changes
            log::info!("Committing register mode edit for index {}: {}", index, selected_index);
        }
        _ => {}
    }
    Ok(())
}

fn commit_text_edit(cursor: types::cursor::ModbusDashboardCursor, value: String) -> Result<()> {
    // Handle text edits (station ID, register address, register length)
    match cursor {
        types::cursor::ModbusDashboardCursor::StationId { index } => {
            log::info!("Committing station ID edit for index {}: {}", index, value);
        }
        types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
            log::info!("Committing register start address edit for index {}: {}", index, value);
        }
        types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
            log::info!("Committing register length edit for index {}: {}", index, value);
        }
        _ => {}
    }
    Ok(())
}