use anyhow::Result;

use ratatui::{prelude::*, text::Line};
use rmodbus::server::context::ModbusContext;
use unicode_width::UnicodeWidthStr;

use crate::{
    protocol::status::{
        read_status,
        types::{self, modbus::RegisterMode},
    },
    tui::ui::components::{
        kv_line::render_kv_line,
        styled_label::{input_spans, switch_spans, TextState},
    },
};

/// Create a register row line that displays 8 registers per line.
/// row_base is the absolute address of the first slot in this row (i.e. multiple of 8).
pub fn render_register_row_line(
    label: &str,
    slave_index: usize,
    row_base: u16,
    item: &crate::protocol::status::types::modbus::ModbusRegisterItem,
    current_selection: types::cursor::ModbusDashboardCursor,
) -> Result<Line<'static>> {
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
    let row_selected = if let types::cursor::ModbusDashboardCursor::Register {
        slave_index: si,
        register_index: ri,
    } = current_selection
    {
        if si == slave_index {
            let sel_addr = item.register_address + (ri as u16);
            sel_addr >= row_base && sel_addr < row_base + 8
        } else {
            false
        }
    } else {
        false
    };

    let row_editing = row_selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);
    let text_state = if row_editing {
        TextState::Editing
    } else if row_selected {
        TextState::Selected
    } else {
        TextState::Normal
    };

    let value_closure = |_: TextState| -> Result<Vec<Span>> {
        let mut spans: Vec<Span> = Vec::new();

        let row_start = row_base;
        const SWITCH_COL_TOTAL_WIDTH: usize = 4;
        const NUMERIC_COL_TOTAL_WIDTH: usize = 6;

        let mut col_widths: [usize; 8] = [0; 8];
        for col_width in col_widths.iter_mut() {
            *col_width = match item.register_mode {
                RegisterMode::Coils | RegisterMode::DiscreteInputs => SWITCH_COL_TOTAL_WIDTH,
                RegisterMode::Holding | RegisterMode::Input => NUMERIC_COL_TOTAL_WIDTH,
            };
        }

        for (slot, _) in col_widths.iter().enumerate() {
            let addr = row_start + slot as u16;
            let item_start = item.register_address;
            let item_end = item_start + item.register_length;

            if slot > 0 {
                spans.push(Span::raw(" "));
            }

            if addr >= item_start && addr < item_end {
                let reg_index = (addr - item_start) as usize;

                let slot_selected = if let types::cursor::ModbusDashboardCursor::Register {
                    slave_index: si,
                    register_index: ri,
                } = current_selection
                {
                    si == slave_index && (item.register_address + ri as u16) == addr
                } else {
                    false
                };

                let editing =
                    slot_selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);
                let state = if editing {
                    TextState::Editing
                } else if slot_selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                let cell_spans = match item.register_mode {
                    RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                        // Read from global storage when available (both Master and Slave modes have storage)
                        let is_on = read_status(|status| {
                            if let types::Page::ModbusDashboard { selected_port, .. } = &status.page
                            {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    if let Some(port_entry) = status.ports.map.get(port_name) {
                                        if let Ok(port_guard) = port_entry.read() {
                                            let types::port::PortConfig::Modbus { mode, .. } =
                                                &port_guard.config;
                                            let storage_opt = match mode {
                                                types::modbus::ModbusConnectionMode::Master {
                                                    storage,
                                                } => Some(storage.clone()),
                                                types::modbus::ModbusConnectionMode::Slave {
                                                    storage,
                                                    ..
                                                } => Some(storage.clone()),
                                            };

                                            if let Some(storage) = storage_opt {
                                                if let Ok(context) = storage.lock() {
                                                    // Use the address as the register index
                                                    let value =
                                                        if item.register_mode == RegisterMode::Coils {
                                                            context.get_coil(addr).unwrap_or(false)
                                                        } else {
                                                            context.get_discrete(addr).unwrap_or(false)
                                                        };
                                                    return Ok(value);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Fallback to placeholder logic
                            Ok(reg_index.is_multiple_of(2))
                        })?;
                        switch_spans(is_on, "ON", "OFF", state)?
                    }
                    RegisterMode::Holding | RegisterMode::Input => {
                        // Read from global storage when available (both Master and Slave modes have storage)
                        let current_value = read_status(|status| {
                            if let types::Page::ModbusDashboard { selected_port, .. } = &status.page
                            {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    if let Some(port_entry) = status.ports.map.get(port_name) {
                                        if let Ok(port_guard) = port_entry.read() {
                                            let types::port::PortConfig::Modbus { mode, .. } =
                                                &port_guard.config;
                                            let storage_opt = match mode {
                                                types::modbus::ModbusConnectionMode::Master {
                                                    storage,
                                                } => Some(storage.clone()),
                                                types::modbus::ModbusConnectionMode::Slave {
                                                    storage,
                                                    ..
                                                } => Some(storage.clone()),
                                            };

                                            if let Some(storage) = storage_opt {
                                                if let Ok(context) = storage.lock() {
                                                    // Use the address as the register index
                                                    let value = if item.register_mode
                                                        == RegisterMode::Holding
                                                    {
                                                        context.get_holding(addr).unwrap_or(0)
                                                    } else {
                                                        context.get_input(addr).unwrap_or(0)
                                                    };
                                                    return Ok(value);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Fallback to placeholder logic
                            Ok((reg_index * 10) as u16)
                        })?;
                        let hex_str = format!("0x{current_value:04X}");
                        input_spans(hex_str.clone(), state)?
                    }
                };

                spans.extend(cell_spans.iter().cloned());

                let cell_text: String = cell_spans.iter().map(|s| s.to_string()).collect();
                let cell_width = UnicodeWidthStr::width(cell_text.as_str());
                let target = col_widths[slot];
                if cell_width < target {
                    spans.push(Span::raw(" ".repeat(target - cell_width)));
                }
            } else {
                let placeholder = "_".repeat(col_widths[slot]);
                spans.push(Span::styled(
                    placeholder,
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        Ok(spans)
    };

    render_kv_line(label, text_state, value_closure)
}
