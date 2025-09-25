use anyhow::Result;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read, write_status},
    tui::ui::components::kv_line::render_kv_line,
    tui::ui::components::styled_label::{
        input_spans, link_spans, selector_spans, switch_spans, TextState,
    },
};

use types::modbus::ParityOption;

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    match read_status(|status| Ok(status.page.clone()))? {
        types::Page::ConfigPanel { cursor, .. } => Ok(cursor),
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
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
    let mut processed = 0usize;

    let group_boundaries: Vec<usize> = types::cursor::CONFIG_PANEL_GROUP_SIZES
        .iter()
        .scan(0usize, |acc, &size| {
            *acc += size;
            Some(*acc)
        })
        .collect();

    let items: Vec<(types::cursor::ConfigPanelCursor, String)> = all
        .iter()
        .map(|&cursor| {
            let label = match cursor {
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
                    if occupied_by_this {
                        lang().protocol.common.label_baud.clone()
                    } else {
                        String::new()
                    }
                }
                types::cursor::ConfigPanelCursor::DataBits { .. } => {
                    if occupied_by_this {
                        lang().protocol.common.label_data_bits.clone()
                    } else {
                        String::new()
                    }
                }
                types::cursor::ConfigPanelCursor::Parity => {
                    if occupied_by_this {
                        lang().protocol.common.label_parity.clone()
                    } else {
                        String::new()
                    }
                }
                types::cursor::ConfigPanelCursor::StopBits => {
                    if occupied_by_this {
                        lang().protocol.common.label_stop_bits.clone()
                    } else {
                        String::new()
                    }
                }
                types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                    if occupied_by_this {
                        lang().protocol.common.view_communication_log.clone()
                    } else {
                        String::new()
                    }
                }
            };
            (cursor, label)
        })
        .collect();

    for (cursor, label) in items.iter() {
        let selected = current_selection == *cursor;
        if let Ok(line) = create_line(label.as_str(), *cursor, selected, port_data.as_ref()) {
            lines.push(line);
        }

        processed += 1;
        if group_boundaries.contains(&processed) && processed < all.len() && occupied_by_this {
            let sep_len = 64usize; // adjust this value if a different length is desired
            let sep_str: String = std::iter::repeat_n('─', sep_len).collect();
            let sep = Span::styled(sep_str, Style::default().fg(Color::DarkGray));
            lines.push(Line::from(vec![sep]));
        }
    }

    Ok(lines)
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: &str,
    cursor: types::cursor::ConfigPanelCursor,
    selected: bool,
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
) -> Result<Line<'static>> {
    let _label_width = label.width();

    let value_closure = |_state: TextState| -> Result<Vec<Span<'static>>> {
        if label.is_empty() {
            return Ok(vec![Span::raw(String::new())]);
        }

        let mut rendered_value_spans: Vec<Span<'static>> = Vec::new();
        match cursor {
            types::cursor::ConfigPanelCursor::BaudRate
            | types::cursor::ConfigPanelCursor::DataBits { .. }
            | types::cursor::ConfigPanelCursor::StopBits => match cursor {
                types::cursor::ConfigPanelCursor::BaudRate => {
                    let cur_index = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                Some(
                                    types::modbus::BaudRateSelector::from_u32(
                                        runtime.current_cfg.baud,
                                    )
                                    .to_index(),
                                )
                            } else {
                                Some(types::modbus::BaudRateSelector::B9600.to_index())
                            }
                        })
                        .unwrap_or(Some(types::modbus::BaudRateSelector::B9600.to_index()))
                    } else {
                        Some(types::modbus::BaudRateSelector::B9600.to_index())
                    };

                    let buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))
                        .unwrap_or(types::ui::InputRawBuffer::None);

                    if selected {
                        write_status(|status| {
                            if let types::Page::ConfigPanel {
                                cursor:
                                    types::cursor::ConfigPanelCursor::DataBits {
                                        ref mut custom_mode,
                                    },
                                ..
                            } = &mut status.page
                            {
                                *custom_mode = false;
                            }
                            Ok(())
                        })?;
                    }

                    let selector_editing =
                        selected && matches!(buffer, types::ui::InputRawBuffer::Index(_));

                    let custom_editing = selected
                        && matches!(
                            buffer,
                            types::ui::InputRawBuffer::String {
                                bytes: _,
                                offset: _
                            }
                        );

                    if custom_editing {
                        let val = get_serial_param_value_by_cursor(port_data, cursor);
                        let state = TextState::Editing;
                        let spans = input_spans(val.clone(), state)
                            .unwrap_or_else(|_| vec![Span::raw(val)]);
                        rendered_value_spans = spans.into_iter().collect();
                    } else {
                        let selected_index = if selector_editing {
                            match buffer {
                                types::ui::InputRawBuffer::Index(i) => i,
                                _ => cur_index
                                    .unwrap_or(types::modbus::BaudRateSelector::B9600.to_index()),
                            }
                        } else {
                            cur_index.unwrap_or(types::modbus::BaudRateSelector::B9600.to_index())
                        };

                        let state = if selector_editing {
                            TextState::Editing
                        } else if selected {
                            TextState::Selected
                        } else {
                            TextState::Normal
                        };

                        let sel_enum = types::modbus::BaudRateSelector::from_index(selected_index);
                        if let types::modbus::BaudRateSelector::Custom { .. } = sel_enum {
                            let runtime_baud = if let Some(port) = port_data {
                                with_port_read(port, |port| {
                                    if let types::port::PortState::OccupiedByThis {
                                        runtime, ..
                                    } = &port.state
                                    {
                                        runtime.current_cfg.baud
                                    } else {
                                        types::modbus::BaudRateSelector::B9600.as_u32()
                                    }
                                })
                                .unwrap_or(types::modbus::BaudRateSelector::B9600.as_u32())
                            } else {
                                types::modbus::BaudRateSelector::B9600.as_u32()
                            };

                            let display =
                                format!("{} ({})", lang().protocol.common.custom, runtime_baud);

                            let spans = match state {
                                TextState::Editing => vec![
                                    Span::styled(
                                        "< ",
                                        Style::default()
                                            .fg(Color::Yellow)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::styled(
                                        display.clone(),
                                        Style::default().fg(Color::Green),
                                    ),
                                    Span::styled(
                                        " >",
                                        Style::default()
                                            .fg(Color::Yellow)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                ],
                                TextState::Selected => vec![Span::styled(
                                    display.clone(),
                                    Style::default().fg(Color::Green),
                                )],
                                TextState::Normal => vec![Span::raw(display.clone())],
                            };

                            rendered_value_spans = spans.into_iter().collect();
                        } else {
                            let spans = selector_spans::<types::modbus::BaudRateSelector>(
                                selected_index,
                                state,
                            )
                            .unwrap_or_else(|_| {
                                vec![Span::raw(
                                    types::modbus::BaudRateSelector::from_index(selected_index)
                                        .to_string(),
                                )]
                            });
                            rendered_value_spans = spans.into_iter().collect();
                        }
                    }
                }
                types::cursor::ConfigPanelCursor::DataBits { .. } => {
                    let cur_index = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                Some({
                                    let db = runtime.current_cfg.data_bits;
                                    match db {
                                        5 => 0usize,
                                        6 => 1usize,
                                        7 => 2usize,
                                        _ => 3usize,
                                    }
                                })
                            } else {
                                Some(3usize)
                            }
                        })
                        .unwrap_or(Some(3usize))
                    } else {
                        Some(3usize)
                    };
                    let global_editing = selected
                        && read_status(|s| Ok(!s.temporarily.input_raw_buffer.is_empty()))
                            .unwrap_or(false);
                    let selected_index = if global_editing {
                        match read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))
                            .unwrap_or(types::ui::InputRawBuffer::None)
                        {
                            types::ui::InputRawBuffer::Index(i) if i < 4 => i,
                            _ => cur_index.unwrap_or(3usize),
                        }
                    } else {
                        cur_index.unwrap_or(3usize)
                    };
                    let state = if global_editing {
                        TextState::Editing
                    } else if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    };
                    let spans =
                        selector_spans::<types::modbus::DataBitsOption>(selected_index, state)
                            .unwrap_or_else(|_| {
                                vec![Span::raw(
                                crate::protocol::status::types::modbus::DataBitsOption::from_repr(
                                    selected_index as u8,
                                )
                                .unwrap_or(
                                    crate::protocol::status::types::modbus::DataBitsOption::Eight,
                                )
                                .to_string(),
                            )]
                            });
                    rendered_value_spans = spans.into_iter().collect();
                }
                types::cursor::ConfigPanelCursor::StopBits => {
                    let cur_index = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                Some({
                                    match runtime.current_cfg.stop_bits {
                                        2 => 1usize,
                                        _ => 0usize,
                                    }
                                })
                            } else {
                                Some(0usize)
                            }
                        })
                        .unwrap_or(Some(0usize))
                    } else {
                        Some(0usize)
                    };
                    let global_editing = selected
                        && read_status(|s| Ok(!s.temporarily.input_raw_buffer.is_empty()))
                            .unwrap_or(false);
                    let selected_index = if global_editing {
                        match read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))
                            .unwrap_or(types::ui::InputRawBuffer::None)
                        {
                            types::ui::InputRawBuffer::Index(i) if i < 2 => i,
                            _ => cur_index.unwrap_or(0usize),
                        }
                    } else {
                        cur_index.unwrap_or(0usize)
                    };
                    let state = if global_editing {
                        TextState::Editing
                    } else if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    };
                    let spans =
                        selector_spans::<types::modbus::StopBitsOption>(selected_index, state)
                            .unwrap_or_else(|_| {
                                vec![Span::raw(
                                crate::protocol::status::types::modbus::StopBitsOption::from_repr(
                                    selected_index as u8,
                                )
                                .unwrap_or(
                                    crate::protocol::status::types::modbus::StopBitsOption::One,
                                )
                                .to_string(),
                            )]
                            });
                    rendered_value_spans = spans.into_iter().collect();
                }
                _ => {}
            },
            types::cursor::ConfigPanelCursor::Parity => {
                let opts: Vec<String> = ParityOption::iter().map(|p| p.to_string()).collect();
                let cur_index = if let Some(port) = port_data {
                    with_port_read(port, |port| {
                        if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state
                        {
                            match runtime.current_cfg.parity {
                                serialport::Parity::None => Some(0usize),
                                serialport::Parity::Odd => Some(1usize),
                                serialport::Parity::Even => Some(2usize),
                            }
                        } else {
                            Some(0usize)
                        }
                    })
                    .unwrap_or(Some(0usize))
                } else {
                    Some(0usize)
                };

                let global_editing = selected
                    && read_status(|status| Ok(!status.temporarily.input_raw_buffer.is_empty()))
                        .unwrap_or(false);
                let selected_index = if global_editing {
                    match read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))
                        .unwrap_or(types::ui::InputRawBuffer::None)
                    {
                        types::ui::InputRawBuffer::Index(i) if i < opts.len() => i,
                        _ => cur_index.unwrap_or(0usize),
                    }
                } else {
                    cur_index.unwrap_or(0usize)
                };

                let state = if global_editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };
                let spans =
                    selector_spans::<ParityOption>(selected_index, state).unwrap_or_else(|_| {
                        vec![Span::raw(
                            opts.get(selected_index).cloned().unwrap_or_default(),
                        )]
                    });
                rendered_value_spans = spans.into_iter().collect();
            }
            types::cursor::ConfigPanelCursor::EnablePort => {
                let enabled = if let Some(port) = port_data {
                    with_port_read(port, |port| {
                        matches!(port.state, types::port::PortState::OccupiedByThis { .. })
                    })
                    .unwrap_or(false)
                } else {
                    false
                };

                let val_enabled = lang().protocol.common.port_enabled.clone();
                let val_disabled = lang().protocol.common.port_disabled.clone();

                let spans = switch_spans(
                    enabled,
                    val_enabled.clone(),
                    val_disabled.clone(),
                    if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    },
                )
                .unwrap_or_else(|_| {
                    vec![Span::raw(if enabled { val_enabled } else { val_disabled })]
                });
                rendered_value_spans = spans.into_iter().collect();
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
                let spans = link_spans(
                    val.clone(),
                    if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    },
                )
                .unwrap_or_else(|_| vec![Span::raw(val.clone())]);
                rendered_value_spans = spans.into_iter().collect();
            }
            _ => {
                rendered_value_spans.push(Span::raw(String::new()));
            }
        }
        Ok(rendered_value_spans)
    };

    let buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))
        .unwrap_or(types::ui::InputRawBuffer::None);
    let text_state = if selected && !matches!(&buffer, types::ui::InputRawBuffer::None) {
        TextState::Editing
    } else if selected {
        TextState::Selected
    } else {
        TextState::Normal
    };

    render_kv_line(label, text_state, value_closure)
}

/// Helper: whether a port is occupied by this instance
fn is_port_occupied_by_this(port_data: Option<&Arc<RwLock<types::port::PortData>>>) -> bool {
    if let Some(port) = port_data {
        if let Some(v) = with_port_read(port, |port| {
            matches!(&port.state, types::port::PortState::OccupiedByThis { .. })
        }) {
            return v;
        }
    }
    false
}

/// Get serial parameter value by cursor type
fn get_serial_param_value_by_cursor(
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> String {
    if let Some(port) = port_data {
        if let Some(s) = with_port_read(port, |port| {
            if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &port.state {
                match cursor_type {
                    types::cursor::ConfigPanelCursor::BaudRate => {
                        return runtime.current_cfg.baud.to_string()
                    }
                    types::cursor::ConfigPanelCursor::DataBits { .. } => {
                        return runtime.current_cfg.data_bits.to_string()
                    }
                    types::cursor::ConfigPanelCursor::Parity => {
                        return format!("{:?}", runtime.current_cfg.parity)
                    }
                    types::cursor::ConfigPanelCursor::StopBits => {
                        return runtime.current_cfg.stop_bits.to_string()
                    }
                    _ => return "??".to_string(),
                }
            }
            "??".to_string()
        }) {
            return s;
        } else {
            log::warn!("get_serial_param_value_by_cursor: failed to acquire read lock");
        }
    }

    "??".to_string()
}
