use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};

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
        boxed_paragraph::render_boxed_paragraph,
        styled_label::{input_spans, selector_spans, switch_spans, TextState},
    },
};

// Constants to avoid magic numbers/strings in layout calculation
const LABEL_PADDING_EXTRA: usize = 2; // extra spacing added before label when padding
const TARGET_LABEL_WIDTH: usize = 20; // target label column width for alignment (reduced by 2)
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
    label: impl ToString,
    cursor: types::cursor::ModbusDashboardCursor,
    selected: bool,
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
) -> Result<Line<'static>> {
    // Selection indicator (similar to config_panel layout)
    let indicator = if selected {
        INDICATOR_SELECTED
    } else {
        INDICATOR_UNSELECTED
    };

    // Read the global input buffer once to avoid calling `read_status` while
    // holding any per-port locks. Calling `read_status` while holding a
    // port-level read lock can cause a lock-order inversion with code paths
    // that take the global status write lock and then a port write lock,
    // leading to deadlocks. Cache the buffer here and consult it below.
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

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
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
                    if let Some(item) = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()))
                    {
                        match item.connection_mode {
                            ModbusConnectionMode::Master => 0usize,
                            ModbusConnectionMode::Slave => 1usize,
                        }
                    } else {
                        0usize // default to Master
                    }
                })
                .ok_or(anyhow!("Failed to read port data for ModbusMode"))?;
                // Determine whether the user is currently editing an index for
                // this field by consulting the cached input buffer (no
                // additional calls to `read_status` here).
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

                rendered_value_spans =
                    selector_spans::<ModbusConnectionMode>(selected_index, state)?;
            }
        }
        types::cursor::ModbusDashboardCursor::StationId { index } => {
            // Station ID input field
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
                    if let Some(item) = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()))
                    {
                        item.station_id.to_string()
                    } else {
                        "1".to_string()
                    }
                })
                .ok_or(anyhow!("Failed to read port data for StationId"))?;
                let editing =
                    selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)?;
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            // Register mode selector
            if let Some(port) = port_data {
                let current_mode = with_port_read(port, |port| {
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
                    if let Some(item) = masters
                        .get(index)
                        .or_else(|| slaves.get(index - masters.len()))
                    {
                        match item.register_mode {
                            RegisterMode::Coils => 0usize,
                            RegisterMode::DiscreteInputs => 1usize,
                            RegisterMode::Holding => 2usize,
                            RegisterMode::Input => 3usize,
                        }
                    } else {
                        2usize // default to Holding
                    }
                })
                .ok_or(anyhow!("Failed to read port data for RegisterMode"))?;
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

                rendered_value_spans = selector_spans::<RegisterMode>(selected_index, state)?;
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
            // Register start address input
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
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
                let editing =
                    selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)?;
            }
        }
        types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
            // Register length input
            if let Some(port) = port_data {
                let current_value = with_port_read(port, |port| {
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
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
                let editing =
                    selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

                let state = if editing {
                    TextState::Editing
                } else if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                };

                rendered_value_spans = input_spans(current_value, state)?;
            }
        }
        types::cursor::ModbusDashboardCursor::Register {
            slave_index,
            register_index,
        } => {
            // Individual register values
            if let Some(port) = port_data {
                let (register_mode, current_value) = with_port_read(port, |port| {
                    let types::port::PortConfig::Modbus { masters, slaves } = &port.config;
                    if let Some(item) = masters
                        .get(slave_index)
                        .or_else(|| slaves.get(slave_index - masters.len()))
                    {
                        let reg_mode = item.register_mode;
                        let value = item.values.get(register_index);
                        value.map(|v| (reg_mode, *v)).unwrap_or((reg_mode, 0))
                    } else {
                        (RegisterMode::Holding, 0)
                    }
                })
                .ok_or(anyhow!("Failed to read port data for Register"))?;
                let editing =
                    selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None);

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
                        rendered_value_spans = switch_spans(is_on, "ON", "OFF", state)?;
                    }
                    RegisterMode::Holding | RegisterMode::Input => {
                        // Render register values as hex with 0x prefix and 4 hex digits
                        let hex_str = format!("0x{:04X}", current_value);
                        rendered_value_spans = input_spans(hex_str.clone(), state)?;
                    }
                }
            }
        }
    }

    // Assemble the final line with proper formatting
    let padded_label_width = label.to_string().width();
    let padded_label = if padded_label_width < TARGET_LABEL_WIDTH {
        TARGET_LABEL_WIDTH - padded_label_width.saturating_sub(LABEL_PADDING_EXTRA)
    } else {
        0
    };
    let padded_label = " ".repeat(padded_label);
    let mut spans = vec![
        Span::raw(indicator),
        Span::raw(label.to_string()).add_modifier(Modifier::BOLD),
        Span::raw(padded_label),
    ];
    spans.extend(rendered_value_spans);

    Ok(Line::from(spans))
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
    // Selection indicator for the whole row: marked selected only if the
    // currently-selected register (if any) is inside this row for this
    // slave_index. Previously we marked a row selected if the item overlapped
    // the row at all which caused multiple rows to appear selected. Here we
    // compute the absolute register address of the selected register and test
    // membership in this row's address range.
    let indicator = if let types::cursor::ModbusDashboardCursor::Register {
        slave_index: si,
        register_index: ri,
    } = current_selection
    {
        if si == slave_index {
            let sel_addr = item.register_address as u16 + (ri as u16);
            if sel_addr >= row_base && sel_addr < row_base + 8 {
                INDICATOR_SELECTED
            } else {
                INDICATOR_UNSELECTED
            }
        } else {
            INDICATOR_UNSELECTED
        }
    } else {
        INDICATOR_UNSELECTED
    };

    // padding same as create_line
    let label_width = label.width();
    let padding_needed = if label_width < TARGET_LABEL_WIDTH {
        TARGET_LABEL_WIDTH - label_width
    } else {
        0
    };
    let padded_label = format!(
        "{}{}",
        label,
        " ".repeat(padding_needed + LABEL_PADDING_EXTRA)
    );

    // read global input buffer (for editing detection)
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw(indicator));
    spans.push(Span::raw(padded_label));

    // For each of the 8 slots in this row, render a value span or a placeholder
    let row_start = row_base as u16;
    // Use fixed per-slot widths: switch columns (Coils/DiscreteInputs) will
    // occupy a total of 4 chars (1 leading space + 3 content). Numeric
    // columns (Holding/Input) will occupy 6 chars (1 leading space + 5
    // digits). This avoids the previous behavior where switch spans produced
    // wide columns (e.g., 8 underscores) and looked awkward.
    const SWITCH_COL_TOTAL_WIDTH: usize = 4;
    const NUMERIC_COL_TOTAL_WIDTH: usize = 6;

    let mut col_widths: [usize; 8] = [0; 8];
    for slot in 0..8usize {
        col_widths[slot] = match item.register_mode {
            RegisterMode::Coils | RegisterMode::DiscreteInputs => SWITCH_COL_TOTAL_WIDTH,
            RegisterMode::Holding | RegisterMode::Input => NUMERIC_COL_TOTAL_WIDTH,
        };
    }

    for slot in 0..8usize {
        let addr = row_start + slot as u16;
        // determine if this addr belongs to this item
        let item_start = item.register_address as u16;
        let item_end = item_start + item.register_length as u16; // exclusive

        if addr >= item_start && addr < item_end {
            // actual register present
            let reg_index = (addr - item_start) as usize; // relative index within item

            // selection and editing state for this specific register
            // Use absolute address comparison: compute absolute address of
            // current slot and of the selected register (if any). This makes
            // selection unambiguous even when items span multiple rows.
            let slot_abs_addr = addr; // already absolute address in u16
            let slot_selected = if let types::cursor::ModbusDashboardCursor::Register {
                slave_index: si,
                register_index: ri,
            } = current_selection
            {
                si == slave_index && (item.register_address as u16 + ri as u16) == slot_abs_addr
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

            // build the value spans depending on register mode and append styled spans
            // directly so TextState styling (Selected/Editing) is preserved.
            let cell_spans = match item.register_mode {
                RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                    // If the values Vec is shorter than register_length (e.g. not yet polled),
                    // default missing values to 0 so the row still renders instead of returning Err.
                    let is_on = item.values.get(reg_index).copied().unwrap_or(0) != 0;
                    switch_spans(is_on, "ON", "OFF", state)?
                }
                RegisterMode::Holding | RegisterMode::Input => {
                    let current_value = item.values.get(reg_index).copied().unwrap_or(0);
                    let hex_str = format!("0x{:04X}", current_value);
                    input_spans(hex_str.clone(), state)?
                }
            };
            spans.extend(cell_spans);
            spans.push(Span::raw(" ")); // space after each slot
        } else {
            // placeholder underscore for empty slot. Use the per-column width
            // so placeholders match the allocated space.
            let placeholder_width = if slot > 0 {
                col_widths[slot].saturating_sub(1) // exclude leading space
            } else {
                col_widths[slot]
            };
            if slot > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                "_".repeat(placeholder_width),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    Ok(Line::from(spans))
}

/// Generate lines for modbus panel with 2:20:remaining layout (indicator:label:value).
/// Returns lines that can be used with render_boxed_paragraph.
pub fn render_kv_lines_with_indicators(_sel_index: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get the current port data
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

    // Get current cursor
    let current_selection = derive_selection()?;

    // Start with "Create Master/Slave" option
    let selected = matches!(
        current_selection,
        types::cursor::ModbusDashboardCursor::AddLine
    );
    if let Ok(line) = create_line(
        &lang().protocol.modbus.add_master_slave,
        types::cursor::ModbusDashboardCursor::AddLine,
        selected,
        port_data.as_ref(),
    ) {
        lines.push(line);
    }

    // Add separator after the first group only if there is at least one master or slave
    let sep_len = 64usize;
    let sep_str: String = std::iter::repeat('─').take(sep_len).collect();
    let sep = Span::styled(sep_str.clone(), Style::default().fg(Color::DarkGray));
    let has_any = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            if let Some(port_entry) = status.ports.map.get(&format!("COM{}", selected_port + 1)) {
                if let Ok(port_guard) = port_entry.read() {
                    match &port_guard.config {
                        types::port::PortConfig::Modbus { masters, slaves } => {
                            Ok(!(masters.is_empty() && slaves.is_empty()))
                        }
                    }
                } else {
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    })?;
    if has_any {
        lines.push(Line::from(vec![sep]));
    }

    // Add configuration groups for existing master/slave items
    if let Some(port_entry) = &port_data {
        if let Ok(port_data_guard) = port_entry.read() {
            let types::port::PortConfig::Modbus { masters, slaves } = &port_data_guard.config;
            // Merge masters and slaves into a single linear list; index below
            // refers to position in this merged list. This ensures Cursor index
            // semantics are consistent across the codebase and navigation.
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
                // The cursor index is the linear index in the merged list. When
                // we need to access the underlying master/slave item elsewhere,
                // convert accordingly (some helpers already do this by checking
                // masters first then slaves using index and masters.len()).
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

                // Register values: render as rows of 8 registers each. Each row
                // covers addresses [row_base, row_base+8). We compute the overall
                // span covering this item's addresses and render each row that
                // intersects it. Empty slots in a row are rendered as placeholders.
                if item.register_length > 0 {
                    let item_start = item.register_address as u16;
                    let item_end = item_start + item.register_length as u16; // exclusive

                    // align row_base to multiples of 8, iterate rows covering the item's range
                    let first_row = (item_start / 8) * 8;
                    let last_row = ((item_end + 7) / 8) * 8; // exclusive upper bound rounded up

                    let mut row = first_row;
                    while row < last_row {
                        // Short label to avoid expanding left column: show only the start address
                        // with two leading spaces (user requested format: "  0x0000").
                        let label = format!("  0x{:04X}", row);
                        if let Ok(line) = create_register_row_line(
                            &label,
                            index,
                            row as u16,
                            item,
                            current_selection,
                        ) {
                            lines.push(line);
                        }
                        row += 8;
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

    Ok(lines)
}

/// Generate status lines for modbus panel display
pub fn generate_modbus_status_lines() -> Result<Vec<Line<'static>>> {
    // Get the current selection index (for compatibility)
    let sel_index = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;

    // Use the new render function
    render_kv_lines_with_indicators(sel_index).map_err(|e| e)
}

/// Render the modbus panel content with scrolling
pub fn render_modbus_content(
    frame: &mut Frame,
    area: Rect,
    lines: Vec<Line>,
    view_offset: usize,
) -> Result<()> {
    // Use the view_offset from page state instead of calculating scroll params
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);
    Ok(())
}
