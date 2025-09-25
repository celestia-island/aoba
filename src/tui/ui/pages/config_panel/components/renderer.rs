use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

use ratatui::{prelude::*, text::Line};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
    tui::ui::components::kv_line::render_kv_line,
    tui::ui::components::styled_label::{
        input_spans, link_spans, selector_spans, switch_spans, TextState,
    },
};

use types::modbus::ParityOption;
use super::utilities::{derive_selection, is_port_occupied_by_this};

use anyhow::Result;

/// Helper function to create selector spans from a list of options
fn selector_spans_with_options(options: &[String], selected_index: usize, text_state: TextState) -> Result<Vec<Span<'static>>> {
    let display_text = options.get(selected_index).cloned().unwrap_or_default();
    match text_state {
        TextState::Editing => input_spans(display_text, text_state),
        TextState::Selected => input_spans(display_text, text_state),
        TextState::Normal => Ok(vec![Span::raw(display_text)]),
    }
}

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
                // Use selector_spans for protocol mode selection
                let current_index = if let Some(port) = port_data {
                    with_port_read(port, |port| match &port.config {
                        types::port::PortConfig::Modbus { .. } => 0usize, // Only Modbus RTU for now
                    }).unwrap_or(0usize)
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
                let protocol_options = vec![lang().protocol.common.mode_modbus.clone()];
                let spans = selector_spans_with_options(&protocol_options, selected_index, text_state)
                    .unwrap_or_else(|_| {
                        let display_text = protocol_options.get(selected_index).cloned().unwrap_or_else(|| lang().protocol.common.mode_modbus.clone());
                        vec![Span::raw(display_text)]
                    });
                rendered_value_spans = spans;
            }
            types::cursor::ConfigPanelCursor::ProtocolConfig => {
                rendered_value_spans = link_spans("", text_state)?; // Remove Configure →
            }
            types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                rendered_value_spans = link_spans("", text_state)?; // Remove View →
            }
            types::cursor::ConfigPanelCursor::BaudRate => {
                if let Some(port) = port_data {
                    let current_baud = with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &port.state {
                            runtime.current_cfg.baud
                        } else {
                            9600
                        }
                    }).unwrap_or(9600);

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
                    if matches!(text_state, TextState::Editing) && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. }) {
                        if let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer {
                            let custom_value = String::from_utf8_lossy(bytes);
                            rendered_value_spans = input_spans(format!("{} baud (custom)", custom_value), text_state)?;
                        } else {
                            rendered_value_spans = input_spans(format!("{} baud (custom)", current_baud), text_state)?;
                        }
                    } else if matches!(text_state, TextState::Normal) && matches!(current_selector, types::modbus::BaudRateSelector::Custom { .. }) {
                        // Show custom value when not in editing mode and current baud is custom
                        rendered_value_spans = vec![Span::raw(format!("{} baud (custom)", current_baud))];
                    } else {
                        // Use selector_spans for first phase (includes custom option selection)
                        let spans = selector_spans::<types::modbus::BaudRateSelector>(selected_index, text_state)
                            .unwrap_or_else(|_| {
                                let selected_selector = types::modbus::BaudRateSelector::from_index(selected_index);
                                vec![Span::raw(selected_selector.to_string())]
                            });
                        rendered_value_spans = spans;
                    }
                } else {
                    rendered_value_spans = vec![Span::raw("9600 baud")];
                }
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                let current_index = if let Some(port) = port_data {
                    with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
                            match runtime.current_cfg.data_bits {
                                5 => 0usize,
                                6 => 1usize,
                                7 => 2usize,
                                _ => 3usize,
                            }
                        } else {
                            3usize
                        }
                    }).unwrap_or(3usize)
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

                let spans = selector_spans::<types::modbus::DataBitsOption>(selected_index, text_state)
                    .unwrap_or_else(|_| {
                        vec![Span::raw(
                            types::modbus::DataBitsOption::from_repr(selected_index as u8)
                                .unwrap_or(types::modbus::DataBitsOption::Eight)
                                .to_string(),
                        )]
                    });
                rendered_value_spans = spans;
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                let current_index = if let Some(port) = port_data {
                    with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
                            match runtime.current_cfg.stop_bits {
                                1 => 0usize,
                                _ => 1usize,
                            }
                        } else {
                            0usize
                        }
                    }).unwrap_or(0usize)
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

                let spans = selector_spans::<types::modbus::StopBitsOption>(selected_index, text_state)
                    .unwrap_or_else(|_| {
                        vec![Span::raw(
                            types::modbus::StopBitsOption::from_repr(selected_index as u8)
                                .unwrap_or(types::modbus::StopBitsOption::One)
                                .to_string(),
                        )]
                    });
                rendered_value_spans = spans;
            }
            types::cursor::ConfigPanelCursor::Parity => {
                let current_index = if let Some(port) = port_data {
                    with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
                            match runtime.current_cfg.parity {
                                serialport::Parity::None => 0usize,
                                serialport::Parity::Odd => 1usize,
                                serialport::Parity::Even => 2usize,
                            }
                        } else {
                            0usize
                        }
                    }).unwrap_or(0usize)
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

                let spans = selector_spans::<ParityOption>(selected_index, text_state)
                    .unwrap_or_else(|_| {
                        let opts: Vec<String> = ParityOption::iter().map(|p| p.to_string()).collect();
                        vec![Span::raw(opts.get(selected_index).cloned().unwrap_or_default())]
                    });
                rendered_value_spans = spans;
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
                "进入业务配置页面".to_string() // TODO: Use proper internationalization
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
                "进入通信日志页面".to_string() // TODO: Use proper internationalization
            } else {
                String::new()
            }
        }
    }
}