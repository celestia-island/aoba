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
    cursor: types::cursor::ModbusDashboardCursor,
    selected: bool,
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
) -> Result<Line<'static>> {
    // Indicator style defaults to no color; branches below may override.
    let mut indicator_style = Style::default();

    // Read the global input buffer once to avoid calling `read_status` while
    // holding any per-port locks. Calling `read_status` while holding a
    // port-level read lock can cause a lock-order inversion with code paths
    // that take the global status write lock and then a port write lock,
    // leading to deadlocks. Cache the buffer here and consult it below.
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;

    // We'll compute the value spans in a closure and then call the shared
    // `render_kv_line` which assembles indicator/label/padding and appends
    // the returned spans. The closure receives a `TextState` and should
    // return `Result<Vec<Span>>`.
    // TODO: 将这里的所有闭包都分别拆分出去，分给各自的 create_line 调用者自行传入；create_line 自身额外加一个没有参数的闭包参数用以渲染
    let value_closure = |_state: TextState| -> Result<Vec<Span>> {
        let mut rendered_value_spans: Vec<Span> = Vec::new();
        match cursor {
            types::cursor::ModbusDashboardCursor::AddLine => {
                rendered_value_spans = vec![];
            }
            types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
                // Connection mode selector
                if let Some(port) = port_data {
                    let current_mode = with_port_read(port, |port| {
                        let types::port::PortConfig::Modbus { masters, slaves } = &port.config;

                        // Indicator style: default unless overridden per-field below. We will set
                        // this to green for selected (non-editing) and yellow for editing.
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
                    // Determine whether the user is currently editing an index for
                    // this field by consulting the cached input buffer (no
                    // additional calls to `read_status` here).
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

                    // Color the left indicator according to text state
                    indicator_style = match state {
                        TextState::Editing => Style::default().fg(Color::Yellow),
                        TextState::Selected => Style::default().fg(Color::Green),
                        TextState::Normal => Style::default(),
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

                    indicator_style = match state {
                        TextState::Editing => Style::default().fg(Color::Yellow),
                        TextState::Selected => Style::default().fg(Color::Green),
                        TextState::Normal => Style::default(),
                    };

                    // Render station id with 0x prefix for display only
                    // (do not change underlying stored value)
                    let hex_display = if current_value.starts_with("0x") {
                        current_value.clone()
                    } else {
                        // try to parse decimal and render as hex; fallback to raw
                        if let Ok(n) = current_value.parse::<u32>() {
                            format!("0x{:04X}", n)
                        } else {
                            format!("0x{}", current_value)
                        }
                    };
                    rendered_value_spans = input_spans(hex_display, state)?;
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
                            (item.register_mode as u8 - 1u8) as usize
                        } else {
                            2usize // default to Holding
                        }
                    })
                    .ok_or(anyhow!("Failed to read port data for RegisterMode"))?;
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

                    indicator_style = match state {
                        TextState::Editing => Style::default().fg(Color::Yellow),
                        TextState::Selected => Style::default().fg(Color::Green),
                        TextState::Normal => Style::default(),
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

                    indicator_style = match state {
                        TextState::Editing => Style::default().fg(Color::Yellow),
                        TextState::Selected => Style::default().fg(Color::Green),
                        TextState::Normal => Style::default(),
                    };

                    // Render start address with 0x prefix for display only
                    let hex_display = if current_value.starts_with("0x") {
                        current_value.clone()
                    } else {
                        if let Ok(n) = current_value.parse::<u32>() {
                            format!("0x{:04X}", n)
                        } else {
                            format!("0x{}", current_value)
                        }
                    };
                    rendered_value_spans = input_spans(hex_display, state)?;
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

                    // Render length as hex with 0x prefix for display only
                    let hex_display = if current_value.starts_with("0x") {
                        current_value.clone()
                    } else {
                        if let Ok(n) = current_value.parse::<u32>() {
                            format!("0x{:04X}", n)
                        } else {
                            format!("0x{}", current_value)
                        }
                    };
                    rendered_value_spans = input_spans(hex_display, state)?;
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
        Ok(rendered_value_spans)
    };

    // Map local selected/editing semantics into a TextState for indicator
    // coloring. We consider editing if the global input buffer is non-empty
    // and this row is selected.
    let text_state = if selected && !matches!(&input_raw_buffer, types::ui::InputRawBuffer::None) {
        TextState::Editing
    } else if selected {
        TextState::Selected
    } else {
        TextState::Normal
    };

    // Use shared renderer; let it decide indicator text/style from TextState.
    render_kv_line(label, text_state, value_closure)
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
    // Determine row selection/editing state to map to TextState for indicator
    // coloring. Row is selected when the currently-selected register lies
    // within this row's address range for the same slave_index.
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
    let row_selected = if let types::cursor::ModbusDashboardCursor::Register {
        slave_index: si,
        register_index: ri,
    } = current_selection
    {
        if si == slave_index {
            let sel_addr = item.register_address as u16 + (ri as u16);
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

    // Build the value spans for the 8 slots in the row. This will be passed
    // as the third-column content to `render_kv_line`.
    let value_closure = |_: TextState| -> Result<Vec<Span>> {
        let mut spans: Vec<Span> = Vec::new();

        let row_start = row_base as u16;
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
            let item_start = item.register_address as u16;
            let item_end = item_start + item.register_length as u16;

            // Always add a single separator space before columns except the first one
            if slot > 0 {
                spans.push(Span::raw(" "));
            }

            if addr >= item_start && addr < item_end {
                let reg_index = (addr - item_start) as usize;

                // selection and editing state for this specific register
                let slot_selected = if let types::cursor::ModbusDashboardCursor::Register {
                    slave_index: si,
                    register_index: ri,
                } = current_selection
                {
                    si == slave_index && (item.register_address as u16 + ri as u16) == addr
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
                        let hex_str = format!("0x{:04X}", current_value);
                        input_spans(hex_str.clone(), state)?
                    }
                };

                // push cell spans
                spans.extend(cell_spans.iter().cloned());

                // Compute width from cell_spans directly to avoid measuring the whole line.
                let cell_text: String = cell_spans.iter().map(|s| s.to_string()).collect();
                let cell_width = UnicodeWidthStr::width(cell_text.as_str());
                let target = col_widths[slot];
                if cell_width < target {
                    spans.push(Span::raw(" ".repeat(target - cell_width)));
                }
            } else {
                // placeholder: render underscores filling the whole column width
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
    lines.push(create_line(
        &lang().protocol.modbus.add_master_slave,
        types::cursor::ModbusDashboardCursor::AddLine,
        matches!(
            current_selection,
            types::cursor::ModbusDashboardCursor::AddLine
        ),
        port_data.as_ref(),
    )?);

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
