use std::sync::{Arc, RwLock};

use ratatui::{prelude::*, text::Line};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
    tui::ui::components::kv_line::render_kv_line,
    tui::ui::components::styled_label::{
        input_spans, link_spans, switch_spans, TextState,
    },
};

use super::utilities::{derive_selection, is_port_occupied_by_this, get_serial_param_value_by_cursor};

use anyhow::Result;

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
            let sep_len = 48usize;
            let sep_str: String = "─".repeat(sep_len);
            lines.push(Line::from(Span::styled(
                sep_str,
                Style::default().fg(Color::DarkGray),
            )));
        }

        let group_start = if group_i == 0 { 0 } else { group_boundaries[group_i - 1] };
        
        for cursor_i in group_start..group_end {
            if cursor_i < all.len() {
                let cursor = all[cursor_i];
                let is_selected = cursor == current_selection;

                // Skip items that are not visible when port is not occupied
                if !occupied_by_this {
                    match cursor {
                        types::cursor::ConfigPanelCursor::EnablePort
                        | types::cursor::ConfigPanelCursor::ProtocolMode => {
                            // These are always visible
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
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
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
        let mut rendered_value_spans: Vec<Span> = Vec::new();

        match cursor_type {
            types::cursor::ConfigPanelCursor::EnablePort => {
                let is_enabled = is_port_occupied_by_this(port_data);
                rendered_value_spans = switch_spans(is_enabled, "Enabled", "Disabled", text_state)?;
            }
            types::cursor::ConfigPanelCursor::ProtocolMode => {
                let val = if let Some(port) = port_data {
                    if let Some(v) = with_port_read(port, |port| match &port.config {
                        types::port::PortConfig::Modbus { .. } => {
                            lang().protocol.common.mode_modbus.clone()
                        }
                    }) {
                        v
                    } else {
                        lang().protocol.common.mode_modbus.clone()
                    }
                } else {
                    lang().protocol.common.mode_modbus.clone()
                };
                rendered_value_spans = vec![Span::raw(val)];
            }
            types::cursor::ConfigPanelCursor::ProtocolConfig => {
                rendered_value_spans = link_spans("Configure →", text_state)?;
            }
            types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                rendered_value_spans = link_spans("View →", text_state)?;
            }
            types::cursor::ConfigPanelCursor::BaudRate => {
                if let Some(port) = port_data {
                    let baud_value = with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &port.state {
                            runtime.current_cfg.baud
                        } else {
                            9600
                        }
                    }).unwrap_or(9600);

                    let sel = types::modbus::BaudRateSelector::from_u32(baud_value);
                    if matches!(text_state, TextState::Editing) {
                        if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                            let selected_sel = types::modbus::BaudRateSelector::from_index(*i);
                            rendered_value_spans = render_baud_rate_selector(selected_sel, text_state)?;
                        } else {
                            rendered_value_spans = render_baud_rate_selector(sel, text_state)?;
                        }
                    } else {
                        rendered_value_spans = render_baud_rate_selector(sel, text_state)?;
                    }
                } else {
                    rendered_value_spans = vec![Span::raw("9600 baud")];
                }
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                let data_bits = get_serial_param_value_by_cursor(port_data, cursor_type);
                if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        let options = ["5", "6", "7", "8"];
                        let display_text = options.get(*i).unwrap_or(&"8");
                        rendered_value_spans = input_spans(format!("{} bits", display_text), text_state)?;
                    } else {
                        rendered_value_spans = input_spans(format!("{} bits", data_bits), text_state)?;
                    }
                } else {
                    rendered_value_spans = input_spans(format!("{} bits", data_bits), text_state)?;
                }
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                let stop_bits = get_serial_param_value_by_cursor(port_data, cursor_type);
                if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        let display_text = if *i == 0 { "1" } else { "2" };
                        rendered_value_spans = input_spans(format!("{} bit", display_text), text_state)?;
                    } else {
                        rendered_value_spans = input_spans(format!("{} bit", stop_bits), text_state)?;
                    }
                } else {
                    rendered_value_spans = input_spans(format!("{} bit", stop_bits), text_state)?;
                }
            }
            types::cursor::ConfigPanelCursor::Parity => {
                if matches!(text_state, TextState::Editing) {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        let options = ["None", "Odd", "Even"];
                        let display_text = options.get(*i).unwrap_or(&"None");
                        rendered_value_spans = input_spans(display_text.to_string(), text_state)?;
                    } else {
                        let parity = get_serial_param_value_by_cursor(port_data, cursor_type);
                        rendered_value_spans = input_spans(parity, text_state)?;
                    }
                } else {
                    let parity = get_serial_param_value_by_cursor(port_data, cursor_type);
                    rendered_value_spans = input_spans(parity, text_state)?;
                }
            }
        }

        Ok(rendered_value_spans)
    };

    render_kv_line(label, text_state, value_closure)
}

fn render_baud_rate_selector(sel: types::modbus::BaudRateSelector, text_state: TextState) -> Result<Vec<Span<'static>>> {
    match sel {
        types::modbus::BaudRateSelector::Custom { baud } => {
            input_spans(format!("{} baud (custom)", baud), text_state)
        }
        _ => {
            input_spans(format!("{} baud", sel.as_u32()), text_state)
        }
    }
}

fn get_cursor_label(cursor: types::cursor::ConfigPanelCursor, occupied_by_this: bool) -> String {
    match cursor {
        types::cursor::ConfigPanelCursor::EnablePort => {
            lang().protocol.common.enable_port.clone()
        }
        types::cursor::ConfigPanelCursor::ProtocolMode => {
            lang().protocol.common.protocol_mode.clone()
        }
        types::cursor::ConfigPanelCursor::ProtocolConfig => {
            if occupied_by_this {
                lang().protocol.common.business_config.clone()
            } else {
                String::new()
            }
        }
        types::cursor::ConfigPanelCursor::BaudRate => {
            lang().protocol.common.label_baud.clone()
        }
        types::cursor::ConfigPanelCursor::DataBits { .. } => {
            lang().protocol.common.label_data_bits.clone()
        }
        types::cursor::ConfigPanelCursor::StopBits => {
            lang().protocol.common.label_stop_bits.clone()
        }
        types::cursor::ConfigPanelCursor::Parity => {
            lang().protocol.common.label_parity.clone()
        }
        types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
            if occupied_by_this {
                "View Communication Log".to_string()
            } else {
                String::new()
            }
        }
    }
}