use anyhow::{anyhow, Result};
use ratatui::{prelude::*, style::Modifier, text::Line};

use super::table::render_register_row_line;
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
        styled_label::{input_spans, selector_spans, TextState},
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

/// Generate lines for modbus panel with 2:20:remaining layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
pub fn render_kv_lines_with_indicators(_sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line> = Vec::new();

    // Separator configuration for this file
    const SEP_LEN: usize = 64usize;

    // Helper: return a full Line containing a separator of given length
    fn separator_line(len: usize) -> Line<'static> {
        let sep_str: String = std::iter::repeat_n('─', len).collect();
        let sep = Span::styled(sep_str, Style::default().fg(Color::DarkGray));
        Line::from(vec![sep])
    }

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

    // Add global mode selector using proper selector_spans
    let global_mode_renderer = || -> Result<Vec<Span<'static>>> {
        let mut rendered_value_spans: Vec<Span> = Vec::new();
        if let Some(port) = port_data.as_ref() {
            let (current_mode, mode_obj) = with_port_read(port, |port| {
                let types::port::PortConfig::Modbus { mode, stations: _ } = &port.config;
                (mode.to_index(), mode.clone())
            })
            .ok_or(anyhow!("Failed to read port data for ModbusMode"))?;

            let selected = matches!(
                current_selection,
                types::cursor::ModbusDashboardCursor::ModbusMode
            );

            let editing =
                selected && matches!(&input_raw_buffer, types::ui::InputRawBuffer::Index(_));

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

            let variants = ModbusConnectionMode::all_variants();
            let current_text = format!("{}", mode_obj);

            match state {
                TextState::Normal => {
                    rendered_value_spans = vec![Span::styled(
                        current_text,
                        Style::default().fg(Color::White),
                    )];
                }
                TextState::Selected => {
                    rendered_value_spans = vec![Span::styled(
                        current_text,
                        Style::default().fg(Color::Yellow),
                    )];
                }
                TextState::Editing => {
                    let mut spans = Vec::new();
                    spans.push(Span::raw("< "));
                    // Use localized Display implementation instead of hardcoded strings
                    let selected_variant = variants.get(selected_index).unwrap_or(&variants[0]);
                    let localized_text = format!("{}", selected_variant);
                    spans.push(Span::styled(
                        localized_text,
                        Style::default().fg(Color::Yellow),
                    ));
                    spans.push(Span::raw(" >"));
                    rendered_value_spans = spans;
                }
            }
        }
        Ok(rendered_value_spans)
    };

    lines.push(create_line(
        &lang().protocol.modbus.connection_mode,
        matches!(
            current_selection,
            types::cursor::ModbusDashboardCursor::ModbusMode
        ),
        global_mode_renderer,
    )?);

    // Separator before stations will be added only when we actually have stations

    // reuse sep_len from above and helper for separator
    let has_any = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            if let Some(port_entry) = status.ports.map.get(&format!("COM{}", selected_port + 1)) {
                if let Ok(port_guard) = port_entry.read() {
                    match &port_guard.config {
                        types::port::PortConfig::Modbus { mode: _, stations } => {
                            return Ok(!stations.is_empty());
                        }
                    }
                }
            }
        }
        Ok(false)
    })?;
    if has_any {
        lines.push(separator_line(SEP_LEN));
    }

    if let Some(port_entry) = &port_data {
        if let Ok(port_data_guard) = port_entry.read() {
            let types::port::PortConfig::Modbus { mode: _, stations } = &port_data_guard.config;
            let all_items = stations.clone();

            for (index, item) in all_items.iter().enumerate() {
                let group_title = format!("#{} - ID: {}", index + 1, item.station_id);
                lines.push(Line::from(vec![Span::styled(
                    group_title,
                    Style::default().add_modifier(Modifier::BOLD),
                )]));

                // Remove individual connection mode selector since we now have global mode

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
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                if let Some(item) = stations.get(index) {
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
                            } else if let Ok(n) = current_value.parse::<u8>() {
                                format!("0x{n:02X} ({n})")
                            } else {
                                format!("0x{current_value} (?)")
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
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                if let Some(item) = stations.get(index) {
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
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                if let Some(item) = stations.get(index) {
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
                            } else if let Ok(n) = current_value.parse::<u16>() {
                                format!("0x{n:04X} ({n})")
                            } else {
                                format!("0x{current_value} (?)")
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
                                let types::port::PortConfig::Modbus { mode: _, stations } =
                                    &port.config;
                                if let Some(item) = stations.get(index) {
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
                            } else if let Ok(n) = current_value.parse::<u16>() {
                                format!("0x{n:04X} ({n})")
                            } else {
                                format!("0x{current_value} (?)")
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
                            render_register_row_line(&label, index, row, item, current_selection)
                        {
                            lines.push(line);
                        }
                        row += 8;
                    }
                }

                if index < all_items.len() - 1 {
                    lines.push(separator_line(SEP_LEN));
                }
            }
        }
    }

    Ok(lines)
}

/// Generate status lines for modbus panel display
pub fn render_modbus_status_lines() -> Result<Vec<Line<'static>>> {
    let sel_index = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    render_kv_lines_with_indicators(sel_index)
}
