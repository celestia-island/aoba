use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};

use super::table::render_register_row_line;
use crate::{
    tui::{
        status as types,
        status::{
            modbus::{
                ModbusConnectionMode, ModbusMasterDataSource, ModbusMasterDataSourceKind,
                ModbusMasterDataSourceValueKind, RegisterMode,
            },
            read_status,
        },
        ui::components::{
            kv_line::render_kv_line,
            styled_label::{input_spans, input_spans_with_placeholder, selector_spans, TextState},
        },
    },
    utils::i18n::lang,
};

/// Get placeholder text for data source based on kind
fn get_data_source_placeholder(kind: ModbusMasterDataSourceKind) -> Option<String> {
    match kind {
        ModbusMasterDataSourceKind::Manual => None,
        ModbusMasterDataSourceKind::MqttServer => {
            Some(lang().protocol.modbus.data_source_placeholder_mqtt.clone())
        }
        ModbusMasterDataSourceKind::HttpServer => {
            Some(lang().protocol.modbus.data_source_placeholder_http.clone())
        }
        ModbusMasterDataSourceKind::IpcPipe => {
            #[cfg(unix)]
            let placeholder = lang()
                .protocol
                .modbus
                .data_source_placeholder_ipc_unix
                .clone();
            #[cfg(windows)]
            let placeholder = lang()
                .protocol
                .modbus
                .data_source_placeholder_ipc_windows
                .clone();
            Some(placeholder)
        }
        ModbusMasterDataSourceKind::PortForwarding => Some(
            lang()
                .protocol
                .modbus
                .data_source_placeholder_port_forwarding
                .clone(),
        ),
    }
}

/// Derive selection index for modbus panel from current page state
pub fn derive_selection() -> Result<types::cursor::ModbusDashboardCursor> {
    read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { cursor, .. } = &status.page {
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
    render_kv_line(label, text_state, adapted, false)
}

/// Generate lines for modbus panel with 2:20:remaining layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
pub fn render_kv_lines_with_indicators(
    _sel_index: usize,
    terminal_width: u16,
) -> Result<Vec<Line<'static>>> {
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
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status
                .ports
                .order
                .get(*selected_port)
                .and_then(|port_name| status.ports.map.get(port_name).cloned()))
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
        if let Some(ref port) = port_data {
            let types::port::PortConfig::Modbus { mode, .. } = &port.config;
            let (current_mode, mode_obj) = (mode.to_index(), mode.clone());

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
            let current_text = format!("{mode_obj}");

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
                    // Use localized Display implementation instead of hardcoded strings
                    let selected_variant = variants.get(selected_index).unwrap_or(&variants[0]);
                    let localized_text = format!("{selected_variant}");
                    rendered_value_spans = vec![
                        Span::raw("< "),
                        Span::styled(localized_text, Style::default().fg(Color::Yellow)),
                        Span::raw(" >"),
                    ];
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

    if let Some(ref port) = port_data {
        let types::port::PortConfig::Modbus {
            mode,
            master_source,
            ..
        } = &port.config;

        if mode.is_master() {
            let master_source_renderer = || -> Result<Vec<Span<'static>>> {
                let Some(port) = port_data.as_ref() else {
                    return Ok(vec![]);
                };

                let types::port::PortConfig::Modbus { master_source, .. } = &port.config;

                let current_index = master_source.kind().to_index();
                let selected = matches!(
                    current_selection,
                    types::cursor::ModbusDashboardCursor::MasterSourceKind
                );

                let editing =
                    selected && matches!(input_raw_buffer, types::ui::InputRawBuffer::Index(_));

                let selected_index = if editing {
                    if let types::ui::InputRawBuffer::Index(i) = &input_raw_buffer {
                        *i
                    } else {
                        current_index
                    }
                } else {
                    current_index
                };

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                let rendered_value_spans =
                    selector_spans::<ModbusMasterDataSourceKind>(selected_index, state)?;
                Ok(rendered_value_spans)
            };

            lines.push(create_line(
                &lang().protocol.modbus.data_source,
                matches!(
                    current_selection,
                    types::cursor::ModbusDashboardCursor::MasterSourceKind
                ),
                master_source_renderer,
            )?);

            let value_kind = master_source.value_kind();
            if !matches!(value_kind, ModbusMasterDataSourceValueKind::None) {
                let value_label = match value_kind {
                    ModbusMasterDataSourceValueKind::Port => {
                        lang().protocol.modbus.data_source_port.clone()
                    }
                    ModbusMasterDataSourceValueKind::Url => {
                        lang().protocol.modbus.data_source_address.clone()
                    }
                    ModbusMasterDataSourceValueKind::Path => {
                        lang().protocol.modbus.data_source_path.clone()
                    }
                    ModbusMasterDataSourceValueKind::PortName => {
                        lang().protocol.modbus.data_source_source_port.clone()
                    }
                    ModbusMasterDataSourceValueKind::None => unreachable!(),
                };

                let master_source_value_renderer = || -> Result<Vec<Span<'static>>> {
                    let Some(port) = port_data.as_ref() else {
                        return Ok(vec![]);
                    };

                    let types::port::PortConfig::Modbus { master_source, .. } = &port.config;

                    let selected = matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::MasterSourceValue
                    );

                    // Special handling for PortForwarding - show selector or hint
                    if matches!(master_source, ModbusMasterDataSource::PortForwarding { .. }) {
                        let current_port_name = read_status(|status| {
                            if let crate::tui::status::Page::ModbusDashboard {
                                selected_port, ..
                            } = &status.page
                            {
                                Ok(status.ports.order.get(*selected_port).cloned())
                            } else {
                                Ok(None)
                            }
                        })?;

                        let all_ports = read_status(|status| Ok(status.ports.order.clone()))?;
                        let available_ports: Vec<String> = all_ports
                            .iter()
                            .filter(|p| Some(p.as_str()) != current_port_name.as_deref())
                            .cloned()
                            .collect();

                        if available_ports.is_empty() {
                            // No other ports available - show greyed hint
                            let hint_text = lang()
                                .protocol
                                .modbus
                                .data_source_port_forwarding_hint
                                .clone();
                            let style = if selected {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::ITALIC)
                            } else {
                                Style::default()
                                    .fg(Color::DarkGray)
                                    .add_modifier(Modifier::ITALIC)
                            };
                            return Ok(vec![Span::styled(hint_text, style)]);
                        }

                        let ModbusMasterDataSource::PortForwarding { source_port } = master_source
                        else {
                            unreachable!();
                        };

                        let editing = selected
                            && matches!(input_raw_buffer, types::ui::InputRawBuffer::Index(_));

                        let state = if editing {
                            TextState::Editing
                        } else if selected {
                            TextState::Selected
                        } else {
                            TextState::Normal
                        };

                        if editing {
                            // Show selector-style with arrow navigation
                            if let types::ui::InputRawBuffer::Index(idx) = &input_raw_buffer {
                                let selected_port_name = available_ports
                                    .get(*idx)
                                    .cloned()
                                    .unwrap_or_else(|| available_ports[0].clone());
                                return Ok(vec![
                                    Span::raw("< "),
                                    Span::styled(
                                        selected_port_name,
                                        Style::default().fg(Color::Yellow),
                                    ),
                                    Span::raw(" >"),
                                ]);
                            }
                        }

                        // Display current selection
                        let display_text = if source_port.is_empty() {
                            lang()
                                .protocol
                                .modbus
                                .data_source_placeholder_port_forwarding
                                .clone()
                        } else {
                            source_port.clone()
                        };

                        let style = match state {
                            TextState::Normal => Style::default().fg(Color::White),
                            TextState::Selected => Style::default().fg(Color::Yellow),
                            TextState::Editing => Style::default().fg(Color::Yellow),
                        };

                        return Ok(vec![Span::styled(display_text, style)]);
                    }

                    // Check if this is HttpServer (numeric port input) or text input
                    let is_http_server =
                        matches!(master_source, ModbusMasterDataSource::HttpServer { .. });

                    if is_http_server {
                        // Render as numeric input for HttpServer port
                        let ModbusMasterDataSource::HttpServer { port: port_num } = master_source
                        else {
                            unreachable!();
                        };

                        let editing = selected
                            && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. });

                        let state = if editing {
                            TextState::Editing
                        } else if selected {
                            TextState::Selected
                        } else {
                            TextState::Normal
                        };

                        if editing {
                            let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer
                            else {
                                unreachable!("editing state requires string buffer");
                            };
                            let custom_value = String::from_utf8_lossy(bytes).to_string();
                            let placeholder = get_data_source_placeholder(master_source.kind());
                            return input_spans_with_placeholder(custom_value, placeholder, state);
                        }

                        let placeholder = get_data_source_placeholder(master_source.kind());
                        input_spans_with_placeholder(port_num.to_string(), placeholder, state)
                    } else {
                        // Render as text input for other types (MQTT, IPC, PortForwarding)
                        let current_value = match master_source {
                            ModbusMasterDataSource::MqttServer { url } => url.clone(),
                            ModbusMasterDataSource::IpcPipe { path } => path.clone(),
                            ModbusMasterDataSource::PortForwarding { source_port } => {
                                source_port.clone()
                            }
                            _ => String::new(),
                        };

                        let editing = selected
                            && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. });

                        let state = if editing {
                            TextState::Editing
                        } else if selected {
                            TextState::Selected
                        } else {
                            TextState::Normal
                        };

                        if editing {
                            let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer
                            else {
                                unreachable!("editing state requires string buffer");
                            };
                            let custom_value = String::from_utf8_lossy(bytes).to_string();
                            let placeholder = get_data_source_placeholder(master_source.kind());
                            return input_spans_with_placeholder(custom_value, placeholder, state);
                        }

                        let placeholder = get_data_source_placeholder(master_source.kind());
                        input_spans_with_placeholder(current_value, placeholder, state)
                    }
                };

                lines.push(create_line(
                    value_label,
                    matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::MasterSourceValue
                    ),
                    master_source_value_renderer,
                )?);
            }
        }
    }

    // Add RequestInterval and Timeout only in Slave mode
    if let Some(ref port) = port_data {
        let types::port::PortConfig::Modbus { mode, .. } = &port.config;
        if mode.is_slave() {
            // RequestInterval field
            let request_interval_renderer = || -> Result<Vec<Span<'static>>> {
                if let Some(ref port) = port_data {
                    let current_value = port.serial_config.request_interval_ms;

                    let selected = matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::RequestInterval
                    );

                    let editing = selected
                        && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. });

                    let state = if editing {
                        TextState::Editing
                    } else if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    };

                    if editing {
                        if let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer {
                            let custom_value = String::from_utf8_lossy(bytes);
                            Ok(input_spans(format!("{custom_value} ms"), state)?)
                        } else {
                            Ok(input_spans(format!("{current_value} ms"), state)?)
                        }
                    } else {
                        Ok(input_spans(format!("{current_value} ms"), state)?)
                    }
                } else {
                    Ok(vec![])
                }
            };

            lines.push(create_line(
                &lang().protocol.common.label_request_interval,
                matches!(
                    current_selection,
                    types::cursor::ModbusDashboardCursor::RequestInterval
                ),
                request_interval_renderer,
            )?);

            // Timeout field
            let timeout_renderer = || -> Result<Vec<Span<'static>>> {
                if let Some(ref port) = port_data {
                    let current_value = port.serial_config.timeout_ms;

                    let selected = matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::Timeout
                    );

                    let editing = selected
                        && matches!(input_raw_buffer, types::ui::InputRawBuffer::String { .. });

                    let state = if editing {
                        TextState::Editing
                    } else if selected {
                        TextState::Selected
                    } else {
                        TextState::Normal
                    };

                    if editing {
                        if let types::ui::InputRawBuffer::String { bytes, .. } = &input_raw_buffer {
                            let custom_value = String::from_utf8_lossy(bytes);
                            Ok(input_spans(format!("{custom_value} ms"), state)?)
                        } else {
                            Ok(input_spans(format!("{current_value} ms"), state)?)
                        }
                    } else {
                        Ok(input_spans(format!("{current_value} ms"), state)?)
                    }
                } else {
                    Ok(vec![])
                }
            };

            lines.push(create_line(
                &lang().protocol.common.label_timeout,
                matches!(
                    current_selection,
                    types::cursor::ModbusDashboardCursor::Timeout
                ),
                timeout_renderer,
            )?);
        }
    }

    // Separator before stations will be added only when we actually have stations

    // reuse sep_len from above and helper for separator
    let has_any = read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port_entry) = status.ports.map.get(port_name) {
                    let port = port_entry;
                    match &port.config {
                        types::port::PortConfig::Modbus { stations, .. } => {
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
        let port_data = port_entry;
        let types::port::PortConfig::Modbus { stations, .. } = &port_data.config;
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
                    let types::port::PortConfig::Modbus { stations, .. } = &port_data.config;
                    let current_value = if let Some(item) = stations.get(index) {
                        item.station_id.to_string()
                    } else {
                        "?".to_string()
                    };

                    let selected = matches!(
                        current_selection,
                        types::cursor::ModbusDashboardCursor::StationId { index: i } if i == index
                    );

                    let editing =
                        selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

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
                    let rendered_value_spans: Vec<Span> = input_spans(hex_display, state)?;
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

                        let types::port::PortConfig::Modbus { stations, .. } =
                            &port_data.config;
                        let current_mode = if let Some(item) = stations.get(index) {
                            (item.register_mode as u8 - 1u8) as usize
                        } else {
                            2usize // default to Holding
                        };

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

                        let rendered_value_spans: Vec<Span> = selector_spans::<RegisterMode>(selected_index, state)?;
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

                        let types::port::PortConfig::Modbus { stations, .. } =
                            &port_data.config;
                        let current_value = if let Some(item) = stations.get(index) {
                            item.register_address.to_string()
                        } else {
                            "0".to_string()
                        };

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
                        let rendered_value_spans: Vec<Span> = input_spans(hex_display, state)?;
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

                        let types::port::PortConfig::Modbus { stations, .. } =
                            &port_data.config;
                        let current_value = if let Some(item) = stations.get(index) {
                            item.register_length.to_string()
                        } else {
                            "1".to_string()
                        };

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
                        let rendered_value_spans: Vec<Span> = input_spans(hex_display, state)?;
                        Ok(rendered_value_spans)
                    },
                )?);

            if item.register_length > 0 {
                let item_start = item.register_address;
                let item_end = item_start + item.register_length;

                // Calculate registers per row dynamically based on terminal width
                let registers_per_row = super::table::get_registers_per_row(terminal_width);

                let first_row = (item_start / registers_per_row as u16) * registers_per_row as u16;
                let last_row =
                    item_end.div_ceil(registers_per_row as u16) * registers_per_row as u16;

                let mut row = first_row;
                while row < last_row {
                    let label = format!("  0x{row:04X}");
                    if let Ok(line) = render_register_row_line(
                        &label,
                        index,
                        row,
                        item,
                        current_selection,
                        registers_per_row,
                    ) {
                        lines.push(line);
                    }
                    row += registers_per_row as u16;
                }
            }

            if index < all_items.len() - 1 {
                lines.push(separator_line(SEP_LEN));
            }
        }
    }

    Ok(lines)
}

/// Generate status lines for modbus panel display
pub fn render_modbus_status_lines(terminal_width: u16) -> Result<Vec<Line<'static>>> {
    let sel_index = read_status(|status| {
        if let crate::tui::status::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    render_kv_lines_with_indicators(sel_index, terminal_width)
}
