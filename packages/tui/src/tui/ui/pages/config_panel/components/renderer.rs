use anyhow::Result;

use ratatui::{prelude::*, text::Line};
use types::modbus::ParityOption;

use super::utilities::{derive_selection, is_port_occupied_by_this};
use crate::i18n::lang;
use crate::tui::status as types;
use crate::tui::status::read_status;
use crate::tui::ui::components::{
    kv_line::render_kv_line,
    styled_label::{input_spans, link_spans, selector_spans, switch_spans, TextState},
};

/// Generate lines for config panel with 1:4:5 layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
///
/// Each line has the format: [>] [Label____] [Value_____] with proper spacing.
pub fn render_kv_lines_with_indicators(sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let port_data = if let Some(port_name) =
        read_status(|status| Ok(status.ports.order.get(sel_index).cloned()))?
    {
        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
    } else {
        None
    };

    let current_selection = derive_selection()?;

    let occupied_by_this = is_port_occupied_by_this(port_data.as_ref());
    let all = types::cursor::ConfigPanelCursor::all();

    let group_boundaries: Vec<usize> = types::cursor::CONFIG_PANEL_GROUP_SIZES
        .iter()
        .scan(0usize, |acc, &size| {
            *acc += size;
            Some(*acc)
        })
        .collect();

    for (group_i, &group_end) in group_boundaries.iter().enumerate() {
        if group_i > 0 {
            // Only show separator if port is occupied by this instance
            if occupied_by_this {
                let sep_len = 48usize;
                let sep_str: String = "─".repeat(sep_len);
                lines.push(Line::from(Span::styled(
                    sep_str,
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        let group_start = if group_i == 0 {
            0
        } else {
            group_boundaries[group_i - 1]
        };

        for cursor_i in group_start..group_end {
            if cursor_i < all.len() {
                let cursor = all[cursor_i];
                let is_selected = cursor == current_selection;

                // Skip items that are not visible when port is not occupied
                if !occupied_by_this {
                    match cursor {
                        types::cursor::ConfigPanelCursor::EnablePort
                        | types::cursor::ConfigPanelCursor::ProtocolMode
                        | types::cursor::ConfigPanelCursor::ProtocolConfig
                        | types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                            // First group items are always visible
                        }
                        _ => {
                            continue;
                        }
                    }
                }

                let line = create_line(
                    get_cursor_label(cursor, is_port_occupied_by_this(port_data.as_ref())),
                    is_selected,
                    port_data.as_ref(),
                    cursor,
                )?;
                lines.push(line);
            }
        }
    }

    Ok(lines)
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: impl ToString,
    selected: bool,
    port_data: Option<&types::port::PortData>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> Result<Line<'static>> {
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

    let text_state = if selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None) {
        TextState::Editing
    } else if selected {
        TextState::Selected
    } else {
        TextState::Normal
    };

    let value_closure = |_ts: TextState| -> Result<Vec<Span<'static>>> {
        match cursor_type {
            types::cursor::ConfigPanelCursor::EnablePort => {
                let is_enabled = is_port_occupied_by_this(port_data);
                Ok(switch_spans(
                    is_enabled,
                    &lang().protocol.common.port_enabled,
                    &lang().protocol.common.port_disabled,
                    text_state,
                )?)
            }
            types::cursor::ConfigPanelCursor::ProtocolMode => {
                // Use selector_spans for protocol mode selection
                let current_index = if let Some(port) = port_data {
                    match &port.config {
                        types::port::PortConfig::Modbus { .. } => 0usize, // Only Modbus RTU for now
                    }
                } else {
                    0usize
                };

                let selected_index = if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        *i
                    } else {
                        current_index
                    }
                } else {
                    current_index
                };

                // For now only one option: Modbus RTU
                let protocol_options = [lang().protocol.common.mode_modbus.clone()];
                let display_text = protocol_options.get(selected_index).ok_or_else(|| {
                    anyhow::anyhow!("Invalid protocol mode index: {selected_index}")
                })?;
                Ok(match text_state {
                    TextState::Editing => input_spans(display_text.clone(), text_state)?,
                    TextState::Selected => input_spans(display_text.clone(), text_state)?,
                    TextState::Normal => {
                        vec![Span::raw(display_text.clone())]
                    }
                })
            }
            types::cursor::ConfigPanelCursor::ProtocolConfig => Ok(link_spans("", text_state)?),
            types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                Ok(link_spans("", text_state)?)
            }
            types::cursor::ConfigPanelCursor::BaudRate => {
                if let Some(port) = port_data {
                    let current_baud = port.serial_config.baud;

                    let current_selector = types::modbus::BaudRateSelector::from_u32(current_baud);
                    let current_index = current_selector.to_index();

                    let selected_index = if matches!(text_state, TextState::Editing) {
                        if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                            *i
                        } else {
                            current_index
                        }
                    } else {
                        current_index
                    };

                    // Check if we're in string editing mode (second phase for custom baud rate)
                    if matches!(text_state, TextState::Editing)
                        && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. })
                    {
                        if let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer {
                            let custom_value = String::from_utf8_lossy(bytes);
                            Ok(input_spans(
                                format!(
                                    "{} baud ({})",
                                    custom_value,
                                    lang().protocol.common.custom
                                ),
                                text_state,
                            )?)
                        } else {
                            Ok(input_spans(
                                format!(
                                    "{} baud ({})",
                                    current_baud,
                                    lang().protocol.common.custom
                                ),
                                text_state,
                            )?)
                        }
                    } else if (matches!(text_state, TextState::Normal)
                        || matches!(text_state, TextState::Selected))
                        && matches!(
                            current_selector,
                            types::modbus::BaudRateSelector::Custom { .. }
                        )
                    {
                        // Show custom value when not editing but custom is selected (both Normal and Selected states)
                        Ok(vec![Span::raw(format!(
                            "{} baud ({})",
                            current_baud,
                            lang().protocol.common.custom
                        ))])
                    } else {
                        // Use selector_spans for first phase (includes custom option selection)
                        Ok(selector_spans::<types::modbus::BaudRateSelector>(
                            selected_index,
                            text_state,
                        )?)
                    }
                } else {
                    Ok(vec![])
                }
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                let current_index = if let Some(port) = port_data {
                    match port.serial_config.data_bits {
                        5 => 0usize,
                        6 => 1usize,
                        7 => 2usize,
                        _ => 3usize,
                    }
                } else {
                    3usize
                };

                let selected_index = if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        *i
                    } else {
                        current_index
                    }
                } else {
                    current_index
                };

                Ok(selector_spans::<types::modbus::DataBitsOption>(
                    selected_index,
                    text_state,
                )?)
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                let current_index = if let Some(port) = port_data {
                    match port.serial_config.stop_bits {
                        1 => 0usize,
                        _ => 1usize,
                    }
                } else {
                    0usize
                };

                let selected_index = if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        *i
                    } else {
                        current_index
                    }
                } else {
                    current_index
                };

                Ok(selector_spans::<types::modbus::StopBitsOption>(
                    selected_index,
                    text_state,
                )?)
            }
            types::cursor::ConfigPanelCursor::Parity => {
                let current_index = if let Some(port) = port_data {
                    match port.serial_config.parity {
                        types::port::SerialParity::None => 0usize,
                        types::port::SerialParity::Odd => 1usize,
                        types::port::SerialParity::Even => 2usize,
                    }
                } else {
                    0usize
                };

                let selected_index = if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        *i
                    } else {
                        current_index
                    }
                } else {
                    current_index
                };

                Ok(selector_spans::<ParityOption>(selected_index, text_state)?)
            }
        }
    };

    render_kv_line(label, text_state, value_closure)
}

fn get_cursor_label(cursor: types::cursor::ConfigPanelCursor, _occupied_by_this: bool) -> String {
    match cursor {
        types::cursor::ConfigPanelCursor::EnablePort => lang().protocol.common.enable_port.clone(),
        types::cursor::ConfigPanelCursor::ProtocolMode => {
            lang().protocol.common.protocol_mode.clone()
        }
        types::cursor::ConfigPanelCursor::ProtocolConfig => {
            lang().protocol.common.enter_business_config.clone()
        }
        types::cursor::ConfigPanelCursor::BaudRate => lang().protocol.common.label_baud.clone(),
        types::cursor::ConfigPanelCursor::DataBits { .. } => {
            lang().protocol.common.label_data_bits.clone()
        }
        types::cursor::ConfigPanelCursor::StopBits => {
            lang().protocol.common.label_stop_bits.clone()
        }
        types::cursor::ConfigPanelCursor::Parity => lang().protocol.common.label_parity.clone(),
        types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
            lang().protocol.common.enter_log_page.clone()
        }
    }
}
