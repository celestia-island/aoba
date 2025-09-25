use anyhow::{anyhow, Result};
use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{
        read_status,
        types::{
            self,
            modbus::{ModbusConnectionMode, RegisterMode},
        },
        with_port_read,
    },
    tui::ui::components::{
        kv_line::render_kv_line,
        styled_label::{input_spans, selector_spans, switch_spans, TextState},
    },
};

/// Derive selection index for modbus panel from current page state
pub fn derive_selection() -> Result<types::cursor::ModbusDashboardCursor> {
    read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: impl ToString,
    selected: bool,
    render_closure: impl Fn() -> Result<Vec<Span<'static>>>,
) -> Result<Line<'static>> {
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

    let text_state = if selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None) {
        TextState::Editing
    } else if selected {
        TextState::Selected
    } else {
        TextState::Normal
    };

    let adapted = |_ts: TextState| -> Result<Vec<Span<'static>>> { render_closure() };
    render_kv_line(label, text_state, adapted)
}

/// Create a register row line that displays 8 registers per line.
/// row_base is the absolute address of the first slot in this row (i.e. multiple of 8).
fn create_register_row_line(
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
                        let is_on = item.values.get(reg_index).copied().unwrap_or(0) != 0;
                        switch_spans(is_on, "ON", "OFF", state)?
                    }
                    RegisterMode::Holding | RegisterMode::Input => {
                        let current_value = item.values.get(reg_index).copied().unwrap_or(0);
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

/// Generate lines for modbus panel with 2:20:remaining layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
pub fn render_kv_lines_with_indicators(_sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line> = Vec::new();

    let port_data = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status
                .ports
                .map
                .get(&format!("COM{}", selected_port + 1))
                .cloned())
        } else {
            Ok(None)
        }
    })?;

    let current_selection = derive_selection()?;

    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

    let addline_renderer = || -> Result<Vec<Span<'static>>> { Ok(vec![]) };
    lines.push(create_line(
        &lang().protocol.modbus.add_master_slave,
        matches!(
            current_selection,
            types::cursor::ModbusDashboardCursor::AddLine
        ),
        addline_renderer,
    )?);

    let sep_len = 64usize;
    let sep_str: String = std::iter::repeat_n('─', sep_len).collect();
    let sep = Span::styled(sep_str.clone(), Style::default().fg(Color::DarkGray));
    let has_any = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            if let Some(port_entry) = status.ports.map.get(&format!("COM{}", selected_port + 1)) {
                if let Ok(port_guard) = port_entry.read() {
                    match &port_guard.config {
                        types::port::PortConfig::Modbus { masters, slaves } => {
                            return Ok(!(masters.is_empty() && slaves.is_empty()));
                        }
                    }
                }
            }
        }
        Ok(false)
    })?;
    if has_any {
        lines.push(Line::from(vec![sep]));
    }

    if let Some(port_entry) = &port_data {
        if let Ok(port_data_guard) = port_entry.read() {
            let types::port::PortConfig::Modbus { masters, slaves } = &port_data_guard.config;
            let mut all_items = masters.clone();
            all_items.extend(slaves.clone());

            for (index, item) in all_items.iter().enumerate() {
                let group_title = format!(
                    "{} {} - ID: {}",
                    item.connection_mode,
                    index + 1,
                    item.station_id
                );
                lines.push(Line::from(vec![Span::styled(
                    group_title,
                    Style::default().add_modifier(Modifier::BOLD),
                )]));

                lines.push(create_line(
                    &lang().protocol.modbus.connection_mode,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::ModbusMode { index: i } if i == index
                    ),
                    || -> Result<Vec<Span<'static>>> {
                        let mut rendered_value_spans: Vec<Span> = Vec::new();
                        if let Some(port) = port_data.as_ref() {
                            let current_mode = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { masters, slaves } =
                                    &port.config;

                                if let Some(item) = masters
                                    .get(index)
                                    .or_else(|| slaves.get(index - masters.len()))
                                {
                                    item.connection_mode as usize
                                } else {
                                    0usize // default to Master
                                }
                            })
                            .ok_or(anyhow!("Failed to read port data for ModbusMode"))?;

                            let selected = matches!(
                                current_selection,
                                types::cursor::ModbusDashboardCursor::ModbusMode { index: i } if i == index
                            );

                            let editing = selected
                                && matches!(&input_raw_buffer, types::ui::InputRawBuffer::Index(_));

                            let selected_index = if editing {
                                if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                                    *i
                                } else {
                                    current_mode
                                }
                            } else {
                                current_mode
                            };

                            let state = if editing {
                                TextState::Editing
                            } else if selected {
                                TextState::Selected
                            } else {
                                TextState::Normal
                            };

                            rendered_value_spans = selector_spans::<ModbusConnectionMode>(selected_index, state)?;
                        }
                        Ok(rendered_value_spans)
                    },
                )?);

                lines.push(create_line(
                    &lang().protocol.modbus.station_id,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::StationId { index: i } if i == index
                    ),
                    || -> Result<Vec<Span<'static>>> {
                        let mut rendered_value_spans: Vec<Span> = Vec::new();
                        if let Some(port) = port_data.as_ref() {
                            let current_value = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { masters, slaves } =
                                    &port.config;
                                if let Some(item) = masters
                                    .get(index)
                                    .or_else(|| slaves.get(index - masters.len()))
                                {
                                    item.station_id.to_string()
                                } else {
                                    "?".to_string()
                                }
                            })
                            .ok_or(anyhow!("Failed to read port data for StationId"))?;

                            let selected = matches!(
                                current_selection,
                                types::cursor::ModbusDashboardCursor::StationId { index: i } if i == index
                            );

                            let editing = selected
                                && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                            let state = if editing {
                                TextState::Editing
                            } else if selected {
                                TextState::Selected
                            } else {
                                TextState::Normal
                            };

                            let hex_display = if current_value.starts_with("0x") {
                                current_value.clone()
                            } else if let Ok(n) = current_value.parse::<u32>() {
                                format!("0x{n:02X}")
                            } else {
                                format!("0x{current_value}")
                            };
                            rendered_value_spans = input_spans(hex_display, state)?;
                        }
                        Ok(rendered_value_spans)
                    },
                )?);

                lines.push(create_line(
                    &lang().protocol.modbus.register_mode,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::RegisterMode { index: i } if i == index
                    ),
                    || -> Result<Vec<Span<'static>>> {
                        let mut rendered_value_spans: Vec<Span> = Vec::new();
                        if let Some(port) = port_data.as_ref() {
                            let current_mode = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { masters, slaves } =
                                    &port.config;
                                if let Some(item) = masters
                                    .get(index)
                                    .or_else(|| slaves.get(index - masters.len()))
                                {
                                    (item.register_mode as u8 - 1u8) as usize
                                } else {
                                    2usize // default to Holding
                                }
                            })
                            .ok_or(anyhow!("Failed to read port data for RegisterMode"))?;

                            let selected = matches!(
                                current_selection,
                                types::cursor::ModbusDashboardCursor::RegisterMode { index: i } if i == index
                            );

                            let editing = selected
                                && matches!(&input_raw_buffer, types::ui::InputRawBuffer::Index(_));

                            let selected_index = if editing {
                                if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                                    *i
                                } else {
                                    current_mode
                                }
                            } else {
                                current_mode
                            };

                            let state = if editing {
                                TextState::Editing
                            } else if selected {
                                TextState::Selected
                            } else {
                                TextState::Normal
                            };

                            rendered_value_spans = selector_spans::<RegisterMode>(selected_index, state)?;
                        }
                        Ok(rendered_value_spans)
                    },
                )?);

                lines.push(create_line(
                    &lang().protocol.modbus.register_start_address,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::RegisterStartAddress { index: i } if i == index
                    ),
                    || -> Result<Vec<Span<'static>>> {
                        let mut rendered_value_spans: Vec<Span> = Vec::new();
                        if let Some(port) = port_data.as_ref() {
                            let current_value = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { masters, slaves } =
                                    &port.config;
                                if let Some(item) = masters
                                    .get(index)
                                    .or_else(|| slaves.get(index - masters.len()))
                                {
                                    item.register_address.to_string()
                                } else {
                                    "0".to_string()
                                }
                            })
                            .ok_or(anyhow!("Failed to read port data for RegisterStartAddress"))?;

                            let selected = matches!(
                                current_selection,
                                types::cursor::ModbusDashboardCursor::RegisterStartAddress { index: i } if i == index
                            );

                            let editing = selected
                                && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                            let state = if editing {
                                TextState::Editing
                            } else if selected {
                                TextState::Selected
                            } else {
                                TextState::Normal
                            };

                            let hex_display = if current_value.starts_with("0x") {
                                current_value.clone()
                            } else if let Ok(n) = current_value.parse::<u32>() {
                                format!("0x{n:04X}")
                            } else {
                                format!("0x{current_value}")
                            };
                            rendered_value_spans = input_spans(hex_display, state)?;
                        }
                        Ok(rendered_value_spans)
                    },
                )?);

                lines.push(create_line(
                    &lang().protocol.modbus.register_length,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::RegisterLength { index: i } if i == index
                    ),
                    || -> Result<Vec<Span<'static>>> {
                        let mut rendered_value_spans: Vec<Span> = Vec::new();
                        if let Some(port) = port_data.as_ref() {
                            let current_value = with_port_read(port, |port| {
                                let types::port::PortConfig::Modbus { masters, slaves } =
                                    &port.config;
                                if let Some(item) = masters
                                    .get(index)
                                    .or_else(|| slaves.get(index - masters.len()))
                                {
                                    item.register_length.to_string()
                                } else {
                                    "1".to_string()
                                }
                            })
                            .ok_or(anyhow!("Failed to read port data for RegisterLength"))?;

                            let selected = matches!(
                                current_selection,
                                types::cursor::ModbusDashboardCursor::RegisterLength { index: i } if i == index
                            );

                            let editing = selected
                                && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                            let state = if editing {
                                TextState::Editing
                            } else if selected {
                                TextState::Selected
                            } else {
                                TextState::Normal
                            };

                            let hex_display = if current_value.starts_with("0x") {
                                current_value.clone()
                            } else if let Ok(n) = current_value.parse::<u32>() {
                                format!("0x{n:04X}")
                            } else {
                                format!("0x{current_value}")
                            };
                            rendered_value_spans = input_spans(hex_display, state)?;
                        }
                        Ok(rendered_value_spans)
                    },
                )?);

                if item.register_length > 0 {
                    let item_start = item.register_address;
                    let item_end = item_start + item.register_length;

                    let first_row = (item_start / 8) * 8;
                    let last_row = item_end.div_ceil(8) * 8;

                    let mut row = first_row;
                    while row < last_row {
                        let label = format!("  0x{row:04X}");
                        if let Ok(line) =
                            create_register_row_line(&label, index, row, item, current_selection)
                        {
                            lines.push(line);
                        }
                        row += 8;
                    }
                }

                if index < all_items.len() - 1 {
                    let sep = Span::styled(sep_str.clone(), Style::default().fg(Color::DarkGray));
                    lines.push(Line::from(vec![sep]));
                }
            }
        }
    }

    Ok(lines)
}

/// Generate status lines for modbus panel display
pub fn generate_modbus_status_lines() -> Result<Vec<Line<'static>>> {
    let sel_index = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    render_kv_lines_with_indicators(sel_index)
}
