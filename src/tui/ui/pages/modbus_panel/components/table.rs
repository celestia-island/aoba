use anyhow::Result;

use ratatui::{prelude::*, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    protocol::status::{
        read_status,
        types::{self, modbus::RegisterMode},
    },
    tui::ui::components::{
        kv_line::render_kv_line,
        styled_label::{input_spans, switch_spans, TextState},
    },
};

/// Determine the number of registers to display per row based on terminal width.
/// Returns 1, 4, or 8 depending on available space.
pub fn get_registers_per_row(terminal_width: u16) -> usize {
    // Each register needs approximately 7 characters (6 for "0x0000" + 1 space)
    // Plus label space (~10 chars for "0x0000 ")
    const LABEL_WIDTH: u16 = 10;
    const REG_WIDTH: u16 = 7;

    let available_width = terminal_width.saturating_sub(LABEL_WIDTH + 4); // 4 for margins

    if available_width >= REG_WIDTH * 8 {
        8
    } else if available_width >= REG_WIDTH * 4 {
        4
    } else {
        1
    }
}

/// Create a register row line that displays registers per line based on terminal width.
/// row_base is the absolute address of the first slot in this row (aligned to registers_per_row).
pub fn render_register_row_line(
    label: &str,
    slave_index: usize,
    row_base: u16,
    item: &crate::protocol::status::types::modbus::ModbusRegisterItem,
    current_selection: types::cursor::ModbusDashboardCursor,
    registers_per_row: usize,
) -> Result<Line<'static>> {
    let input_raw_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()))?;
    let row_selected = if let types::cursor::ModbusDashboardCursor::Register {
        slave_index: si,
        register_index: ri,
    } = current_selection
    {
        if si == slave_index {
            let sel_addr = item.register_address + (ri as u16);
            sel_addr >= row_base && sel_addr < row_base + (registers_per_row as u16)
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

    let value_closure = |_: TextState| -> Result<Vec<Span>> {
        let mut spans: Vec<Span> = Vec::new();

        let row_start = row_base;
        const SWITCH_COL_TOTAL_WIDTH: usize = 4;
        const NUMERIC_COL_TOTAL_WIDTH: usize = 6;

        let col_width_value = match item.register_mode {
            RegisterMode::Coils | RegisterMode::DiscreteInputs => SWITCH_COL_TOTAL_WIDTH,
            RegisterMode::Holding | RegisterMode::Input => NUMERIC_COL_TOTAL_WIDTH,
        };

        for slot in 0..registers_per_row {
            let addr = row_start + slot as u16;
            let item_start = item.register_address;
            let item_end = item_start + item.register_length;

            if slot > 0 {
                spans.push(Span::raw(" "));
            }

            if addr >= item_start && addr < item_end {
                let _reg_index = (addr - item_start) as usize;

                let slot_selected = if let types::cursor::ModbusDashboardCursor::Register {
                    slave_index: si,
                    register_index: ri,
                } = current_selection
                {
                    si == slave_index && (item.register_address + ri as u16) == addr
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

                let relative_index = (addr - item.register_address) as usize;
                let stored_value = item.last_values.get(relative_index).copied().unwrap_or(0);

                let cell_spans = match item.register_mode {
                    RegisterMode::Coils | RegisterMode::DiscreteInputs => {
                        let is_on = stored_value != 0;
                        switch_spans(is_on, "ON", "OFF", state)?
                    }
                    RegisterMode::Holding | RegisterMode::Input => {
                        let hex_str = format!("0x{stored_value:04X}");
                        input_spans(hex_str.clone(), state)?
                    }
                };

                spans.extend(cell_spans.iter().cloned());

                let cell_text: String = cell_spans.iter().map(|s| s.to_string()).collect();
                let cell_width = UnicodeWidthStr::width(cell_text.as_str());
                if cell_width < col_width_value {
                    spans.push(Span::raw(" ".repeat(col_width_value - cell_width)));
                }
            } else {
                let placeholder = "_".repeat(col_width_value);
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
