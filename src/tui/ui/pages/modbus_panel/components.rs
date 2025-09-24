use anyhow::Result;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read, write_status},
    tui::ui::components::styled_label::{
        input_spans, selector_spans, switch_spans, TextState,
    },
};

use types::modbus::{ModbusConnectionMode, RegisterMode};

// Constants to avoid magic numbers/strings in layout calculation
const LABEL_PADDING_EXTRA: usize = 2; // extra spacing added before label when padding
const TARGET_LABEL_WIDTH: usize = 20; // target label column width for alignment
const INDICATOR_SELECTED: &str = "> ";
const INDICATOR_UNSELECTED: &str = "  ";

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
    label: &str,
    cursor: types::cursor::ModbusDashboardCursor,
    selected: bool,
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
) -> Result<Line<'static>> {
    // Early return empty line for empty labels, consistent with config_panel
    if label.is_empty() {
        return Ok(Line::from(vec![Span::raw("")]));
    }

    // Selection indicator (similar to config_panel layout)
    let indicator = if selected {
        INDICATOR_SELECTED
    } else {
        INDICATOR_UNSELECTED
    };

    // Calculate dynamic padding to achieve target alignment
    let label_width = label.width();
    let padding_needed = if label_width < TARGET_LABEL_WIDTH {
        TARGET_LABEL_WIDTH - label_width
    } else {
        0
    };
    let padded_label = format!("{}{}", label, " ".repeat(padding_needed + LABEL_PADDING_EXTRA));

    let mut rendered_value_spans: Vec<Span> = Vec::new();

    match cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            // Simple text option for "Create Master/Slave"
            rendered_value_spans = vec![Span::styled(
                lang().protocol.modbus.add_master_slave.clone(),
                if selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                },
            )];
        }
        types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
            // Connection mode selector
            if let Some(port) = port_data {
                let current_mode = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(index).or_else(|| slaves.get(index - masters.len())) {
                            Ok(match item.connection_mode {
                                ModbusConnectionMode::Master => 0,
                                ModbusConnectionMode::Slave => 1,
                            })
                        } else {
                            Ok(0) // default to Master
                        }
                    } else {
                        Ok(0)
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::Index(_)))
                })?;

                let selected_index = if editing {
                    read_status(|s| {
                        if let types::ui::InputRawBuffer::Index(i) = s.temporarily.input_raw_buffer {
                            Ok(i)
                        } else {
                            Ok(current_mode)
                        }
                    })?
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

                rendered_value_spans = selector_spans::<ModbusConnectionMode>(selected_index, state)
                    .unwrap_or_else(|_| vec![Span::raw("Master")]);
            }
        }
        types::cursor::ModbusDashboardCursor::StationId { index } => {
            // Station ID input field
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(index).or_else(|| slaves.get(index - masters.len())) {
                            Ok(item.station_id.to_string())
                        } else {
                            Ok("1".to_string())
                        }
                    } else {
                        Ok("1".to_string())
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(!matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::None))
                })?;

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)
                    .unwrap_or_else(|_| vec![Span::raw("1")]);
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            // Register mode selector
            if let Some(port) = port_data {
                let current_mode = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(index).or_else(|| slaves.get(index - masters.len())) {
                            Ok(match item.register_mode {
                                RegisterMode::Coils => 0,
                                RegisterMode::DiscreteInputs => 1,
                                RegisterMode::Holding => 2,
                                RegisterMode::Input => 3,
                            })
                        } else {
                            Ok(2) // default to Holding
                        }
                    } else {
                        Ok(2)
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::Index(_)))
                })?;

                let selected_index = if editing {
                    read_status(|s| {
                        if let types::ui::InputRawBuffer::Index(i) = s.temporarily.input_raw_buffer {
                            Ok(i)
                        } else {
                            Ok(current_mode)
                        }
                    })?
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

                rendered_value_spans = selector_spans::<RegisterMode>(selected_index, state)
                    .unwrap_or_else(|_| vec![Span::raw("Holding Registers")]);
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
            // Register start address input
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(index).or_else(|| slaves.get(index - masters.len())) {
                            Ok(item.register_address.to_string())
                        } else {
                            Ok("0".to_string())
                        }
                    } else {
                        Ok("0".to_string())
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(!matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::None))
                })?;

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)
                    .unwrap_or_else(|_| vec![Span::raw("0")]);
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
            // Register length input
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(index).or_else(|| slaves.get(index - masters.len())) {
                            Ok(item.register_length.to_string())
                        } else {
                            Ok("1".to_string())
                        }
                    } else {
                        Ok("1".to_string())
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(!matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::None))
                })?;

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)
                    .unwrap_or_else(|_| vec![Span::raw("1")]);
            }
        }
        types::cursor::ModbusDashboardCursor::Register { slave_index, register_index } => {
            // Individual register values
            if let Some(port) = port_data {
                let (register_mode, current_value) = with_port_read(port, |port| {
                    if let types::port::PortConfig::Modbus { masters, slaves } = &port.config {
                        if let Some(item) = masters.get(slave_index).or_else(|| slaves.get(slave_index - masters.len())) {
                            let reg_mode = item.register_mode;
                            let value = item.values.get(register_index).copied().unwrap_or(0);
                            Ok((reg_mode, value))
                        } else {
                            Ok((RegisterMode::Holding, 0))
                        }
                    } else {
                        Ok((RegisterMode::Holding, 0))
                    }
                })?;

                let editing = selected && read_status(|s| {
                    Ok(!matches!(s.temporarily.input_raw_buffer, types::ui::InputRawBuffer::None))
                })?;

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                // Use switch_spans for Coils and DiscreteInputs, input_spans for others
                match register_mode {
                    RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                        let is_on = current_value != 0;
                        rendered_value_spans = switch_spans(is_on, "ON", "OFF", state)
                            .unwrap_or_else(|_| vec![Span::raw("OFF")]);
                    }
                    RegisterMode::Holding | RegisterMode::Input => {
                        rendered_value_spans = input_spans(current_value.to_string(), state)
                            .unwrap_or_else(|_| vec![Span::raw(current_value.to_string())]);
                    }
                }
            }
        }
    }

    // Assemble the final line with proper formatting
    let mut spans = vec![
        Span::raw(indicator),
        Span::raw(padded_label),
    ];
    spans.extend(rendered_value_spans);

    Ok(Line::from(spans))
}

/// Generate lines for modbus panel with 2:20:remaining layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
pub fn render_kv_lines_with_indicators(sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get the current port data
    let port_data = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status.ports.map.get(&format!("COM{}", selected_port + 1)).cloned())
        } else {
            Ok(None)
        }
    })?;

    // Get current cursor
    let current_selection = derive_selection()?;

    // Start with "Create Master/Slave" option
    let selected = matches!(current_selection, types::cursor::ModbusDashboardCursor::AddLine);
    if let Ok(line) = create_line(
        &lang().protocol.modbus.add_master_slave,
        types::cursor::ModbusDashboardCursor::AddLine,
        selected,
        port_data.as_ref(),
    ) {
        lines.push(line);
    }

    // Add separator after the first group
    let sep_len = 64usize;
    let sep_str: String = std::iter::repeat('─').take(sep_len).collect();
    let sep = Span::styled(sep_str, Style::default().fg(Color::DarkGray));
    lines.push(Line::from(vec![sep]));

    // Add configuration groups for existing master/slave items
    if let Some(port_entry) = &port_data {
        if let Ok(port_data_guard) = port_entry.read() {
            if let types::port::PortConfig::Modbus { masters, slaves } = &port_data_guard.config {
                let mut all_items = masters.clone();
                all_items.extend(slaves.clone());

                for (index, item) in all_items.iter().enumerate() {
                    // Group title
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

                    // Connection mode
                    let cursor = types::cursor::ModbusDashboardCursor::ModbusMode { index };
                    let selected = current_selection == cursor;
                    if let Ok(line) = create_line(
                        &lang().protocol.modbus.connection_mode,
                        cursor,
                        selected,
                        port_data.as_ref(),
                    ) {
                        lines.push(line);
                    }

                    // Station ID
                    let cursor = types::cursor::ModbusDashboardCursor::StationId { index };
                    let selected = current_selection == cursor;
                    if let Ok(line) = create_line(
                        &lang().protocol.modbus.station_id,
                        cursor,
                        selected,
                        port_data.as_ref(),
                    ) {
                        lines.push(line);
                    }

                    // Register mode
                    let cursor = types::cursor::ModbusDashboardCursor::RegisterMode { index };
                    let selected = current_selection == cursor;
                    if let Ok(line) = create_line(
                        &lang().protocol.modbus.register_mode,
                        cursor,
                        selected,
                        port_data.as_ref(),
                    ) {
                        lines.push(line);
                    }

                    // Register start address
                    let cursor = types::cursor::ModbusDashboardCursor::RegisterStartAddress { index };
                    let selected = current_selection == cursor;
                    if let Ok(line) = create_line(
                        &lang().protocol.modbus.register_start_address,
                        cursor,
                        selected,
                        port_data.as_ref(),
                    ) {
                        lines.push(line);
                    }

                    // Register length
                    let cursor = types::cursor::ModbusDashboardCursor::RegisterLength { index };
                    let selected = current_selection == cursor;
                    if let Ok(line) = create_line(
                        &lang().protocol.modbus.register_length,
                        cursor,
                        selected,
                        port_data.as_ref(),
                    ) {
                        lines.push(line);
                    }

                    // Register values
                    for reg_index in 0..item.register_length {
                        let cursor = types::cursor::ModbusDashboardCursor::Register {
                            slave_index: index,
                            register_index: reg_index as usize,
                        };
                        let selected = current_selection == cursor;
                        let label = format!("Register {}", item.register_address + reg_index);
                        if let Ok(line) = create_line(&label, cursor, selected, port_data.as_ref()) {
                            lines.push(line);
                        }
                    }

                    // Add separator between groups
                    if index < all_items.len() - 1 {
                        let sep = Span::styled(sep_str.clone(), Style::default().fg(Color::DarkGray));
                        lines.push(Line::from(vec![sep]));
                    }
                }
            }
        }
    }

    Ok(lines)
}

/// Generate status lines for modbus panel display
pub fn generate_modbus_status_lines() -> Vec<Line<'static>> {
    // Get the current selection index (for compatibility)
    let sel_index = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })
    .unwrap_or(0);

    // Use the new render function
    render_kv_lines_with_indicators(sel_index).unwrap_or_else(|_| vec![Line::from("Error loading modbus data")])
}

/// Render the modbus panel content with scrolling
pub fn render_modbus_content(frame: &mut Frame, area: Rect, lines: Vec<Line>, view_offset: usize) {
    use crate::tui::ui::components::boxed_paragraph::render_boxed_paragraph;
    // Use the view_offset from page state instead of calculating scroll params
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);
}
