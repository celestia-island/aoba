use std::cmp::min;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
};

use crate::{i18n::lang, protocol::status::Status, tui::ui::components::render_boxed_paragraph};

/// Master list panel rendering.
/// - List all master configurations with a trailing "new" entry.
/// - Header per master: #idx, ID, Type, Address range.
/// - Followed by value grid (8 columns) aligned by absolute address.
/// - Pure UI layer; editing handled by key processing logic elsewhere.
pub fn render_master_list_panel(f: &mut Frame, area: Rect, app: &mut Status) {
    let mut all_lines: Vec<Line> = Vec::new();

    if let Some(form) = app.subpage_form.as_ref() {
        if form.registers.is_empty() {
            // Empty list -> highlight the 'new' entry with an arrow
            let selected = form.master_cursor == 0; // Cursor=0 means the 'new' entry
            let prefix = if selected { "> " } else { "  " };
            let content = format!("{}[+] {}", prefix, lang().protocol.new_master);
            if selected {
                all_lines.push(Line::styled(content, Style::default().fg(Color::Green)));
            } else {
                all_lines.push(Line::from(content));
            }
        } else {
            for (i, r) in form.registers.iter().enumerate() {
                let start = r.address as u32;
                let end_inclusive = start + r.length as u32 - 1; // Assume length >= 1
                let selected = form.master_cursor == i;
                let mut line_spans: Vec<Span> = Vec::new();
                // Fixed prefix: selector arrow + two spaces + # index (keeps # aligned).
                if selected {
                    line_spans.push(Span::raw("> ")); // Arrow
                }
                line_spans.push(Span::raw(format!("#{}", i + 1))); // '#' start column fixed
                line_spans.push(Span::raw(", "));
                // Determine layered states.
                let this_selected_master = form.master_cursor == i;
                let this_master_chosen =
                    form.master_field_selected && form.master_edit_index == Some(i);
                let this_master_editing_field =
                    form.master_field_editing && form.master_edit_index == Some(i);
                let cur_field = if this_master_chosen {
                    form.master_edit_field.clone()
                } else {
                    None
                };
                use crate::protocol::status::MasterEditField as MEF;
                // Style helpers.
                let browse_style = Style::default().fg(Color::Green); // Browse layer selection color
                let chosen_style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD); // Current field selection
                let active_field_style = Style::default().fg(Color::Yellow); // Editing this field plain yellow
                let normal_style = Style::default();
                let master_base_style = if this_selected_master {
                    browse_style
                } else {
                    normal_style
                };
                // ID field.
                let id_active = matches!(cur_field, Some(MEF::Id)) && this_master_editing_field;
                let id_style = if id_active {
                    active_field_style
                } else if this_master_chosen && matches!(cur_field, Some(MEF::Id)) {
                    chosen_style
                } else {
                    master_base_style
                };
                if id_active {
                    let content = if form.master_input_buffer.is_empty() {
                        "_".to_string()
                    } else {
                        form.master_input_buffer.clone()
                    };
                    line_spans.push(Span::styled(format!("ID = [{}]", content), id_style));
                } else {
                    line_spans.push(Span::styled(format!("ID = {:02X}", r.slave_id), id_style));
                }
                line_spans.push(Span::raw(", "));
                // Register type: single current value; in editing show spinner style.
                let types = [
                    lang().protocol.reg_type_coils.as_str(),
                    lang().protocol.reg_type_discrete_inputs.as_str(),
                    lang().protocol.reg_type_holding.as_str(),
                    lang().protocol.reg_type_input.as_str(),
                ];
                let cur_type_idx = ((r.mode as u8 as usize).saturating_sub(1)).min(3); // Enum discriminant 1..4 -> index 0..3
                if matches!(cur_field, Some(MEF::Type)) && this_master_editing_field {
                    line_spans.push(Span::styled("< ", master_base_style));
                    line_spans.push(Span::styled(
                        format!("[{}]", types[cur_type_idx]),
                        active_field_style,
                    ));
                    line_spans.push(Span::styled(" >", master_base_style));
                } else if this_master_chosen && matches!(cur_field, Some(MEF::Type)) {
                    line_spans.push(Span::styled(
                        format!("[{}]", types[cur_type_idx]),
                        chosen_style,
                    ));
                } else {
                    line_spans.push(Span::styled(
                        format!("{}", types[cur_type_idx]),
                        master_base_style,
                    ));
                }
                line_spans.push(Span::raw(", "));

                // Address range (start-end).
                let range_prefix_style = master_base_style;
                line_spans.push(Span::styled(
                    format!("{} = ", lang().protocol.label_address_range),
                    range_prefix_style,
                ));
                let start_active =
                    matches!(cur_field, Some(MEF::Start)) && this_master_editing_field;
                let start_style = if start_active {
                    active_field_style
                } else if this_master_chosen && matches!(cur_field, Some(MEF::Start)) {
                    chosen_style
                } else {
                    master_base_style
                };
                if start_active {
                    let content = if form.master_input_buffer.is_empty() {
                        "_".to_string()
                    } else {
                        form.master_input_buffer.clone()
                    };
                    line_spans.push(Span::styled(format!("0x[{}]", content), start_style));
                } else {
                    line_spans.push(Span::styled(format!("0x{:04X}", start), start_style));
                }
                line_spans.push(Span::raw(" - "));
                let end_active = matches!(cur_field, Some(MEF::End)) && this_master_editing_field;
                let end_style = if end_active {
                    active_field_style
                } else if this_master_chosen && matches!(cur_field, Some(MEF::End)) {
                    chosen_style
                } else {
                    master_base_style
                };
                if end_active {
                    let content = if form.master_input_buffer.is_empty() {
                        "_".to_string()
                    } else {
                        form.master_input_buffer.clone()
                    };
                    line_spans.push(Span::styled(format!("0x[{}]", content), end_style));
                } else {
                    line_spans.push(Span::styled(format!("0x{:04X}", end_inclusive), end_style));
                }
                all_lines.push(Line::from(line_spans));

                if r.length > 0 {
                    let mut addr = start as usize;
                    let end_exclusive = (end_inclusive + 1) as usize;
                    while addr < end_exclusive {
                        let line_base = (addr / 8) * 8;
                        let mut cols: Vec<String> = Vec::with_capacity(8);
                        for col in 0..8 {
                            let cur = line_base + col;
                            if cur < start as usize || cur >= end_exclusive {
                                cols.push("__".to_string()); // Non-selectable placeholder.
                            } else {
                                let offset = (cur - start as usize) as usize;
                                let val = r.values.get(offset).cloned().unwrap_or(0);
                                cols.push(format!("{:02X}", val));
                            }
                        }
                        let mut row_spans: Vec<Span> = Vec::new();
                        row_spans.push(Span::raw(format!("      0x{:04X}: ", line_base)));
                        let editing_this = this_master_chosen;
                        for (ci, text) in cols.iter().enumerate() {
                            if ci > 0 {
                                row_spans.push(Span::raw(" "));
                            }
                            let cell_addr = line_base + ci;
                            let style = if let Some(MEF::Value(a)) = &cur_field {
                                if *a as usize == cell_addr && this_master_editing_field {
                                    active_field_style
                                } else if !this_master_editing_field && *a as usize == cell_addr {
                                    chosen_style
                                } else if editing_this {
                                    browse_style
                                } else if this_selected_master {
                                    browse_style
                                } else {
                                    normal_style
                                }
                            } else if editing_this {
                                browse_style
                            } else if this_selected_master {
                                browse_style
                            } else {
                                normal_style
                            };
                            if let Some(MEF::Value(a)) = &cur_field {
                                if *a as usize == cell_addr && this_master_editing_field {
                                    let content = if form.master_input_buffer.is_empty() {
                                        "_".to_string()
                                    } else {
                                        form.master_input_buffer.clone()
                                    };
                                    row_spans.push(Span::styled(format!("[{}]", content), style));
                                    continue;
                                }
                            }
                            row_spans.push(Span::styled(text.clone(), style));
                        }
                        // Removed trailing live edit suffix display.
                        all_lines.push(Line::from(row_spans));
                        addr = line_base + 8;
                    }
                }
                all_lines.push(Line::from(""));
            }
            let new_sel = form.master_cursor == form.registers.len();
            let new_line = format!(
                "{}[+] {}",
                if new_sel { "> " } else { "  " },
                lang().protocol.new_master
            );
            if new_sel {
                all_lines.push(Line::styled(new_line, Style::default().fg(Color::Green)));
            } else {
                all_lines.push(Line::from(new_line));
            }
        }
    } else {
    all_lines.push(Line::from(lang().index.details_placeholder.as_str()));
    all_lines.push(Line::from(format!("[+] {}", lang().protocol.new_master)));
    }
    // Compute scroll window
    let inner_height = area.height.saturating_sub(2) as usize; // Number of lines visible inside the border
    let mut first_visible = 0usize;
    if let Some(form) = app.subpage_form.as_ref() {
        // Compute starting line index of current cursor's header
        let mut cursor_line = 0usize;
        let mut accum = 0usize;
        for (i, r) in form.registers.iter().enumerate() {
            if i == form.master_cursor {
                cursor_line = accum;
                break;
            }
            let value_lines = if r.length == 0 {
                0
            } else {
                (r.length as usize + 7) / 8
            };
            accum += 1 + value_lines + 1; // Header + values + blank
        }
        if form.master_cursor == form.registers.len() {
            // 'New' line
            cursor_line = accum; // Accumulated lines of all masters
        }
        // If editing a value field, ensure its line is visible
        if form.master_field_selected {
            if let Some(idx) = form.master_edit_index {
                if idx < form.registers.len() {
                    if let Some(field) = &form.master_edit_field {
                        if let crate::protocol::status::MasterEditField::Value(addr) = field {
                            let r_cur = &form.registers[idx];
                            if *addr >= r_cur.address && *addr < r_cur.address + r_cur.length {
                                // Compute the line number of that value
                                let mut line_no = 0usize;
                                for (i2, r2) in form.registers.iter().enumerate() {
                                    let val_lines2 = if r2.length == 0 {
                                        0
                                    } else {
                                        (r2.length as usize + 7) / 8
                                    };
                                    if i2 == idx {
                                        line_no += 1; // Header line
                                        if r2.length > 0 {
                                            let offset = *addr as usize - r2.address as usize;
                                            let val_line_index = offset / 8;
                                            line_no += val_line_index; // Value line offset
                                        }
                                        if line_no < first_visible {
                                            first_visible = line_no;
                                        } else if line_no >= first_visible + inner_height {
                                            first_visible = line_no + 1 - inner_height;
                                        }
                                        break;
                                    } else {
                                        line_no += 1 + val_lines2 + 1; // Header + values + blank
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if cursor_line >= first_visible + inner_height {
            first_visible = cursor_line + 1 - inner_height;
        }
    }
    let total_lines = all_lines.len();
    let last_start = total_lines.saturating_sub(inner_height);
    if first_visible > last_start {
        first_visible = last_start;
    }
    let end = min(total_lines, first_visible + inner_height);
    let window = &all_lines[first_visible..end];
    render_boxed_paragraph(f, area, window.to_vec(), None);

    // Scroll bar
    if total_lines > inner_height && inner_height > 0 {
        let bar_x = area.x + area.width.saturating_sub(1);
        let bar_y = area.y + 1; // Inside border
        let bar_h = area.height.saturating_sub(2);
        let denom = (total_lines.saturating_sub(inner_height)) as f32;
        let ratio = if denom > 0.0 {
            (first_visible as f32) / denom
        } else {
            0.0
        };
        let thumb_pos = bar_y + ((ratio * (bar_h.saturating_sub(1) as f32)).round() as u16);
        for i in 0..bar_h {
            let ch = if bar_y + i == thumb_pos { '█' } else { '│' };
            let p = ratatui::widgets::Paragraph::new(ch.to_string())
                .style(Style::default().fg(Color::DarkGray));
            let r = Rect::new(bar_x, bar_y + i, 1, 1);
            f.render_widget(p, r);
        }
    }
}
