use anyhow::Result;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read, write_status},
    tui::ui::components::styled_label::{
        input_spans, link_spans, selector_spans, switch_spans, TextState,
    },
};

use types::modbus::ParityOption;

// Constants to avoid magic numbers/strings in layout calculation
const LABEL_PADDING_EXTRA: usize = 2; // extra spacing added before label when padding
const TARGET_LABEL_WIDTH: usize = 20; // target label column width for alignment
const INDICATOR_SELECTED: &str = "> ";
const INDICATOR_UNSELECTED: &str = "  ";

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    // For config panel, we need to determine which field is currently selected
    match read_status(|status| Ok(status.page.clone()))? {
        types::Page::ConfigPanel { cursor, .. } => {
            // cursor tracks both navigation and editing state
            Ok(cursor)
        }
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
    }
}

/// Generate lines for config panel with 1:4:5 layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
///
/// Each line has the format: [>] [Label____] [Value_____] with proper spacing.
pub fn render_kv_lines_with_indicators(sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Get current port data
    let port_data = if let Some(port_name) =
        read_status(|status| Ok(status.ports.order.get(sel_index).cloned()))?
    {
        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
    } else {
        None
    };

    // Determine current selection for styling
    let current_selection = derive_selection()?;

    // Determine whether the port is occupied by this instance. Only in that case
    // we display the full set of controls (group2, group3 and protocol config
    // navigation inside group1).
    let occupied_by_this = is_port_occupied_by_this(port_data.as_ref());
    // Build lines from ConfigPanelCursor::all() and CONFIG_PANEL_GROUP_SIZES so
    // the logical cursor indices map to visual rows (including separators).
    let all = types::cursor::ConfigPanelCursor::all();
    // cumulative index for processed items
    let mut processed = 0usize;

    let group_boundaries: Vec<usize> = types::cursor::CONFIG_PANEL_GROUP_SIZES
        .iter()
        .scan(0usize, |acc, &size| {
            *acc += size;
            Some(*acc)
        })
        .collect();

    // Precompute (cursor, label) pairs to avoid repeating the match logic
    // inside the render loop. This also makes the rendering loop focused
    // only on presentation (selection and pushing lines).
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
        // create_line will early-return an empty line for empty labels
        if let Ok(line) = create_line(label.as_str(), *cursor, selected, port_data.as_ref()) {
            lines.push(line);
        }

        processed += 1;
        // Re-add a dedicated visual separator line at group boundaries. We
        // use a repeated unicode box-drawing char and a dim style so it is
        // clearly visible but not selectable (it isn't associated with any
        // cursor item). This preserves the original behavior where view
        // offsets count these blank rows for scrolling logic.
        if group_boundaries.contains(&processed) && processed < all.len() {
            // Only show group separators when this port is actually occupied by
            // this instance. If the port is disabled/hidden we shouldn't draw
            // the separator.
            if occupied_by_this {
                // Build a separator long enough for typical panels. Exact width
                // will be clipped by the rendering area.
                // Generate a separator using an iterator so the length can be
                // easily adjusted and is not duplicated as a raw string literal.
                let sep_len = 64usize; // adjust this value if a different length is desired
                let sep_str: String = std::iter::repeat('─').take(sep_len).collect();
                let sep = Span::styled(sep_str, Style::default().fg(Color::DarkGray));
                lines.push(Line::from(vec![sep]));
            }
        }
    }

    return Ok(lines);
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: &str,
    cursor: types::cursor::ConfigPanelCursor,
    selected: bool,
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
) -> Result<Line<'static>> {
    let label = label;
    // Calculate the width of the label accurately accounting for Unicode
    let label_width = label.width();

    // Create spans
    let mut line_spans = Vec::new();

    // Add indicator first (moved to left). We'll add an empty placeholder for
    // separator lines as well.
    // Determine selected indicator span
    // Note: we don't push it yet for separator (empty label) because separators
    // should remain empty lines.

    // We'll render the value area now using the styled_label helpers. Also
    // decide whether this row is a separator (empty label + empty text).
    let mut rendered_value_spans: Vec<Span<'static>> = Vec::new();

    // If this is a separator (empty label) and the cursor maps to a separator
    // position, return an empty line to avoid rendering any value widget.
    if label.is_empty() {
        return Ok(Line::from(vec![Span::raw(String::new())]));
    }

    // Determine runtime-dependent values and call appropriate template helpers
    match cursor {
        types::cursor::ConfigPanelCursor::BaudRate
        | types::cursor::ConfigPanelCursor::DataBits { .. }
        | types::cursor::ConfigPanelCursor::StopBits => {
            // For BaudRate we render an editable input span (string). For DataBits
            // and StopBits we render selectors backed by enums.
            match cursor {
                types::cursor::ConfigPanelCursor::BaudRate => {
                    // Determine current preset index from runtime baud or
                    // default to 9600 when unknown.
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

                    // Decide whether we're in selector editing or custom text
                    // input mode. We must read the temporary buffer each render
                    // to reflect current (possibly uncommitted) edit state.
                    // Rules:
                    // - When selected and temporary buffer is Index -> selector editing
                    // - When selected and temporary buffer is String -> custom input mode
                    // - Otherwise show normal/selected visual state
                    let buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))
                        .unwrap_or(types::ui::InputRawBuffer::None);

                    // If the page cursor is currently BaudRate and the row is selected,
                    // ensure DataBits.custom_mode is reset to false so other custom
                    // edits are disabled when focusing BaudRate.
                    if selected {
                        // When this row is focused, ensure DataBits.custom_mode is false
                        // so other custom editing modes are disabled. We perform a
                        // minimal mutation: match the page and update the cursor
                        // if it's the DataBits variant.
                        write_status(|status| {
                            match &mut status.page {
                                types::Page::ConfigPanel { cursor, .. } => {
                                    if let types::cursor::ConfigPanelCursor::DataBits {
                                        ref mut custom_mode,
                                    } = cursor
                                    {
                                        *custom_mode = false;
                                    }
                                }
                                _ => {}
                            }
                            Ok(())
                        })?;
                    }

                    // Selector editing active when buffer is Index and this row is selected
                    let selector_editing =
                        selected && matches!(buffer, types::ui::InputRawBuffer::Index(_));

                    // Custom text editing active when buffer is String and this row is selected
                    let custom_editing = selected
                        && matches!(
                            buffer,
                            types::ui::InputRawBuffer::String {
                                bytes: _,
                                offset: _
                            }
                        );

                    if custom_editing {
                        // Show input box using the string buffer content
                        let val = get_serial_param_value_by_cursor(port_data, cursor);
                        let state = TextState::Editing;
                        let spans = input_spans::<'static, ()>(val.clone(), state)
                            .unwrap_or_else(|_| vec![Span::raw(val)]);
                        rendered_value_spans = spans.into_iter().map(|s| s).collect();
                    } else {
                        // Render selector. If selector_editing is true, selected_index
                        // should come from the Index buffer; otherwise from runtime.
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

                        // If the selected index maps to Custom, render "Custom (value)"
                        // using the runtime baud value so the specific number is visible.
                        let sel_enum = types::modbus::BaudRateSelector::from_index(selected_index);
                        if let types::modbus::BaudRateSelector::Custom { .. } = sel_enum {
                            // obtain runtime baud value (fallback to 9600)
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

                            // Build spans according to state to mimic selector_spans styling
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

                            rendered_value_spans = spans.into_iter().map(|s| s).collect();
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
                            rendered_value_spans = spans.into_iter().map(|s| s).collect();
                        }
                    }
                }
                types::cursor::ConfigPanelCursor::DataBits { .. } => {
                    // render selector for data bits
                    let cur_index = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                Some(
                                    types::modbus::DataBitsOption::from_u8(
                                        runtime.current_cfg.data_bits,
                                    )
                                    .to_index(),
                                )
                            } else {
                                Some(types::modbus::DataBitsOption::Eight.to_index())
                            }
                        })
                        .unwrap_or(Some(types::modbus::DataBitsOption::Eight.to_index()))
                    } else {
                        Some(types::modbus::DataBitsOption::Eight.to_index())
                    };
                    // Consider we're in global editing mode only when the
                    // temporary input buffer is non-empty AND this row is the
                    // currently selected one. This prevents other rows from
                    // showing the Editing state when some other field is being
                    // edited.
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
                                    crate::protocol::status::types::modbus::
                                        DataBitsOption::from_index(selected_index)
                                        .to_string(),
                                )]
                            });
                    rendered_value_spans = spans.into_iter().map(|s| s).collect();
                }
                types::cursor::ConfigPanelCursor::StopBits => {
                    let cur_index = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                Some(
                                    types::modbus::StopBitsOption::from_u8(
                                        runtime.current_cfg.stop_bits,
                                    )
                                    .to_index(),
                                )
                            } else {
                                Some(types::modbus::StopBitsOption::One.to_index())
                            }
                        })
                        .unwrap_or(Some(types::modbus::StopBitsOption::One.to_index()))
                    } else {
                        Some(types::modbus::StopBitsOption::One.to_index())
                    };
                    // Only treat as editing when this row is selected and the
                    // global temporary buffer has content.
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
                                    crate::protocol::status::types::modbus::
                                        StopBitsOption::from_index(selected_index)
                                        .to_string(),
                                )]
                            });
                    rendered_value_spans = spans.into_iter().map(|s| s).collect();
                }
                _ => {}
            }
        }
        types::cursor::ConfigPanelCursor::Parity => {
            let opts: Vec<String> = ParityOption::iter().map(|p| p.to_string()).collect();
            let cur_index = if let Some(port) = port_data {
                with_port_read(port, |port| {
                    if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
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

            // Determine whether in global editing mode and buffered index
            // Only mark Parity as Editing when the panel buffer is set and
            // this row is the currently selected one. This avoids showing
            // the editing decoration on other rows.
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
            rendered_value_spans = spans.into_iter().map(|s| s).collect();
        }
        types::cursor::ConfigPanelCursor::EnablePort => {
            // Determine whether the port is actually enabled/occupied by this
            // instance. Use that value as the switch state rather than the
            // `selected` UI flag which denotes cursor selection.
            let enabled = if let Some(port) = port_data {
                with_port_read(port, |port| match port.state {
                    types::port::PortState::OccupiedByThis { .. } => true,
                    _ => false,
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
            .unwrap_or_else(|_| vec![Span::raw(if enabled { val_enabled } else { val_disabled })]);
            rendered_value_spans = spans.into_iter().map(|s| s).collect();
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
            rendered_value_spans = spans.into_iter().map(|s| s).collect();
        }
        _ => {
            // Fallback: display nothing or placeholder
            rendered_value_spans.push(Span::raw(String::new()));
        }
    }

    // Now build final visual order: indicator (left), label (center), value (right)
    // Create indicator span
    let indicator_span = if selected {
        Span::styled(
            INDICATOR_SELECTED.to_string(),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::raw(INDICATOR_UNSELECTED.to_string())
    };

    // Create label span with bold style
    let label_span = Span::styled(
        label.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    );

    // Calculate dynamic spacing after label to reach TARGET_LABEL_WIDTH
    let padding_needed = if label_width < TARGET_LABEL_WIDTH {
        TARGET_LABEL_WIDTH - label_width + LABEL_PADDING_EXTRA
    } else {
        LABEL_PADDING_EXTRA
    };

    // Assemble: indicator, label, padding, value spans
    line_spans.clear();
    line_spans.push(indicator_span);
    line_spans.push(label_span);
    line_spans.push(Span::raw(" ".repeat(padding_needed)));
    line_spans.extend(rendered_value_spans);

    // Previously we appended an inline marker for group boundaries which
    // caused visual artifacts. Group separators are now rendered as their
    // own dedicated lines (see above) and should not be appended inline.

    Ok(Line::from(line_spans))
}

/// Helper: whether a port is occupied by this instance
fn is_port_occupied_by_this(port_data: Option<&Arc<RwLock<types::port::PortData>>>) -> bool {
    if let Some(port) = port_data {
        if let Some(v) = with_port_read(port, |port| match &port.state {
            types::port::PortState::OccupiedByThis { .. } => true,
            _ => false,
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
