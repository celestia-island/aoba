use std::cmp::min;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
};

use crate::{
    i18n::lang,
    protocol::status::{EntryRole, MasterEditField, RegisterEntry, Status},
    tui::ui::components::render_boxed_paragraph,
};

/// Unified ModBus list panel: each entry has a Role (Master / Slave) + fields previously split.
/// Added first editable field: Role, allowing switching between Master and Slave behaviors.
pub fn render_modbus_panel(f: &mut Frame, area: Rect, app: &mut Status) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(form) = app.subpage_form.as_ref() {
        if form.registers.is_empty() {
            let sel = form.master_cursor == 0;
            let prefix = if sel { "> " } else { "  " };
            let line = format!("{}[+] {}", prefix, lang().protocol.modbus.new_entry);
            if sel {
                lines.push(Line::styled(line, Style::default().fg(Color::Green)));
            } else {
                lines.push(Line::from(line));
            }
        } else {
            for (i, r) in form.registers.iter().enumerate() {
                render_entry_header(&mut lines, i, r, form);
                render_entry_values(&mut lines, i, r, form);
                lines.push(Line::from(""));
            }
            let new_sel = form.master_cursor == form.registers.len();
            let new_line = format!(
                "{}[+] {}",
                if new_sel { "> " } else { "  " },
                lang().protocol.modbus.new_entry
            );
            if new_sel {
                lines.push(Line::styled(new_line, Style::default().fg(Color::Green)));
            } else {
                lines.push(Line::from(new_line));
            }
        }
    } else {
        lines.push(Line::from(lang().index.details_placeholder.as_str()));
        lines.push(Line::from(format!(
            "[+] {}",
            lang().protocol.modbus.new_entry
        )));
    }

    // Scroll logic copied from master / pull panels
    let inner_height = area.height.saturating_sub(2) as usize;
    let mut first_visible = 0usize;
    if let Some(form) = app.subpage_form.as_ref() {
        let mut cursor_line = 0usize;
        let mut accum = 0usize;
        for (i, r) in form.registers.iter().enumerate() {
            if i == form.master_cursor {
                cursor_line = accum;
                break;
            }
            let val_lines = if r.length == 0 {
                0
            } else {
                (r.length as usize + 7) / 8
            };
            accum += 1 + val_lines + 1;
        }
        if form.master_cursor == form.registers.len() {
            cursor_line = accum;
        }
        if form.master_field_selected {
            if let Some(idx) = form.master_edit_index {
                if idx < form.registers.len() {
                    if let Some(field) = &form.master_edit_field {
                        if let MasterEditField::Value(a) = field {
                            let r_cur = &form.registers[idx];
                            if *a >= r_cur.address && *a < r_cur.address + r_cur.length {
                                let mut line_no = 0usize;
                                for (i2, r2) in form.registers.iter().enumerate() {
                                    let lines2 = if r2.length == 0 {
                                        0
                                    } else {
                                        (r2.length as usize + 7) / 8
                                    };
                                    if i2 == idx {
                                        line_no += 1;
                                        if r2.length > 0 {
                                            let off = *a as usize - r2.address as usize;
                                            line_no += off / 8;
                                        }
                                        if line_no < first_visible {
                                            first_visible = line_no;
                                        } else if line_no >= first_visible + inner_height {
                                            first_visible = line_no + 1 - inner_height;
                                        }
                                        break;
                                    } else {
                                        line_no += 1 + lines2 + 1;
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
    let total = lines.len();
    let last_start = total.saturating_sub(inner_height);
    if first_visible > last_start {
        first_visible = last_start;
    }
    let end = min(total, first_visible + inner_height);
    render_boxed_paragraph(f, area, lines[first_visible..end].to_vec(), None);
    if total > inner_height && inner_height > 0 {
        render_scroll_bar(f, area, first_visible, inner_height, total);
    }
}

fn render_entry_header(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
) {
    use MasterEditField as F;
    let selected = form.master_cursor == idx;
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };
    let browse = Style::default().fg(Color::Green);
    let chosen_style = Style::default().fg(Color::Yellow);
    let active = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default();
    let base = if selected { browse } else { normal };
    let mut spans: Vec<Span> = Vec::new();
    // Prefix + index coloring rule:
    //  - Cursor hover (selected but not chosen / editing): Green
    //  - Entry has a specific field selected / editing (chosen): Yellow (non-bold)
    //  - Otherwise: normal
    let prefix_style = if selected {
        if chosen {
            chosen_style
        } else {
            browse
        }
    } else {
        normal
    };
    if selected {
        spans.push(Span::styled("> ", prefix_style));
    }
    spans.push(Span::styled(format!("#{}", idx + 1), prefix_style));
    spans.push(Span::raw(", "));
    // Role field (new first editable)
    let role_active = matches!(cur_field, Some(F::Role)) && editing;
    let role_style = if role_active {
        active
    } else if chosen && matches!(cur_field, Some(F::Role)) {
        chosen_style
    } else {
        base
    };
    let role_label = match r.role {
        EntryRole::Master => lang().protocol.modbus.role_master.as_str(),
        EntryRole::Slave => lang().protocol.modbus.role_slave.as_str(),
    };
    if role_active {
        spans.push(Span::styled(format!("[{}]", role_label), role_style));
    } else if chosen && matches!(cur_field, Some(F::Role)) {
        // Chosen (non-edit) state: add brackets for non-color terminals
        spans.push(Span::styled(format!("[{}]", role_label), role_style));
    } else {
        spans.push(Span::styled(role_label.to_string(), role_style));
    }
    spans.push(Span::raw(", "));
    // ID
    let id_active = matches!(cur_field, Some(F::Id)) && editing;
    let id_style = if id_active {
        active
    } else if chosen && matches!(cur_field, Some(F::Id)) {
        chosen_style
    } else {
        base
    };
    if id_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("ID = [{}]", content), id_style));
    } else if chosen && matches!(cur_field, Some(F::Id)) {
        spans.push(Span::styled(format!("ID = [{:02X}]", r.slave_id), id_style));
    } else {
        spans.push(Span::styled(format!("ID = {:02X}", r.slave_id), id_style));
    }
    spans.push(Span::raw(", "));
    // Type
    let types = [
        lang().protocol.modbus.reg_type_coils.as_str(),
        lang().protocol.modbus.reg_type_discrete_inputs.as_str(),
        lang().protocol.modbus.reg_type_holding.as_str(),
        lang().protocol.modbus.reg_type_input.as_str(),
    ];
    let type_idx = ((r.mode as u8 as usize).saturating_sub(1)).min(3);
    if matches!(cur_field, Some(F::Type)) && editing {
        spans.push(Span::styled("< ", base));
        spans.push(Span::styled(format!("[{}]", types[type_idx]), active));
        spans.push(Span::styled(" >", base));
    } else if chosen && matches!(cur_field, Some(F::Type)) {
        spans.push(Span::styled(format!("[{}]", types[type_idx]), chosen_style));
    } else {
        spans.push(Span::styled(types[type_idx].to_string(), base));
    }
    spans.push(Span::raw(", "));
    // Address range
    let start = r.address as u32;
    let end_inclusive = start + r.length as u32 - 1;
    spans.push(Span::styled(
        format!("{} = ", lang().protocol.modbus.label_address_range),
        base,
    ));
    let start_active = matches!(cur_field, Some(F::Start)) && editing;
    let start_style = if start_active {
        active
    } else if chosen && matches!(cur_field, Some(F::Start)) {
        chosen_style
    } else {
        base
    };
    if start_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("0x[{}]", content), start_style));
    } else if chosen && matches!(cur_field, Some(F::Start)) {
        spans.push(Span::styled(format!("0x[{:04X}]", start), start_style));
    } else {
        spans.push(Span::styled(format!("0x{:04X}", start), start_style));
    }
    spans.push(Span::raw(" - "));
    let end_active = matches!(cur_field, Some(F::End)) && editing;
    let end_style = if end_active {
        active
    } else if chosen && matches!(cur_field, Some(F::End)) {
        chosen_style
    } else {
        base
    };
    if end_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("0x[{}]", content), end_style));
    } else if chosen && matches!(cur_field, Some(F::End)) {
        spans.push(Span::styled(
            format!("0x[{:04X}]", end_inclusive),
            end_style,
        ));
    } else {
        spans.push(Span::styled(format!("0x{:04X}", end_inclusive), end_style));
    }
    // Refresh + counter
    spans.push(Span::raw(", "));
    let refresh_active = matches!(cur_field, Some(F::Refresh)) && editing;
    let refresh_style = if refresh_active {
        active
    } else if chosen && matches!(cur_field, Some(F::Refresh)) {
        chosen_style
    } else {
        base
    };
    let refresh_label = lang().protocol.modbus.refresh_rate.as_str();
    if refresh_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(
            format!("{} = [{}] ms", refresh_label, content),
            refresh_style,
        ));
    } else if chosen && matches!(cur_field, Some(F::Refresh)) {
        spans.push(Span::styled(
            format!("{} = [{}] ms", refresh_label, r.refresh_ms),
            refresh_style,
        ));
    } else {
        spans.push(Span::styled(
            format!("{} = {} ms", refresh_label, r.refresh_ms),
            refresh_style,
        ));
    }
    spans.push(Span::raw(", "));
    let counter_label = lang().protocol.modbus.label_req_counter.as_str();
    let counter_selected = chosen && matches!(cur_field, Some(F::Counter));
    let counter_style = if counter_selected {
        chosen_style
    } else {
        Style::default().fg(Color::Green)
    };
    if counter_selected {
        spans.push(Span::styled(
            format!("{} = [{} / {}]", counter_label, r.req_success, r.req_total),
            counter_style,
        ));
    } else {
        spans.push(Span::styled(
            format!("{} = {} / {}", counter_label, r.req_success, r.req_total),
            counter_style,
        ));
    }
    out.push(Line::from(spans));
}

fn render_entry_values(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
) {
    use MasterEditField as F;
    if r.length == 0 {
        return;
    }
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };
    let start = r.address as usize;
    let end_exclusive = start + r.length as usize;
    let browse = Style::default().fg(Color::Green);
    let chosen_style = Style::default().fg(Color::Yellow);
    let active = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default();
    let mut addr = start;
    while addr < end_exclusive {
        let line_base = (addr / 8) * 8;
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(format!("      0x{:04X}: ", line_base)));
        for col in 0..8 {
            if col > 0 {
                spans.push(Span::raw(" "));
            }
            let cur = line_base + col;
            if cur < start || cur >= end_exclusive {
                spans.push(Span::raw("__"));
                continue;
            }
            let offset = cur - start;
            let raw_val = r.values.get(offset).cloned().unwrap_or(0u16);
            let style = if let Some(F::Value(a)) = &cur_field {
                if *a as usize == cur && editing {
                    active
                } else if !editing && *a as usize == cur {
                    chosen_style
                } else if chosen {
                    browse
                } else {
                    normal
                }
            } else if chosen {
                browse
            } else {
                normal
            };
            if let Some(F::Value(a)) = &cur_field {
                if *a as usize == cur && editing {
                    // For Coils and DiscreteInputs show boolean editor; otherwise show numeric editor
                    if r.mode == crate::protocol::status::RegisterMode::Coils
                        || r.mode == crate::protocol::status::RegisterMode::DiscreteInputs
                    {
                        let lbl = if raw_val != 0 {
                            lang().protocol.modbus.value_true.as_str()
                        } else {
                            lang().protocol.modbus.value_false.as_str()
                        };
                        spans.push(Span::styled(format!("[{}]", lbl), style));
                    } else {
                        let content = if form.master_input_buffer.is_empty() {
                            "_"
                        } else {
                            form.master_input_buffer.as_str()
                        };
                        spans.push(Span::styled(format!("[{}]", content), style));
                    }
                    continue;
                }
            }
            let is_chosen_value = if let Some(F::Value(a)) = &cur_field {
                !editing && *a as usize == cur
            } else {
                false
            };
            if r.mode == crate::protocol::status::RegisterMode::Coils
                || r.mode == crate::protocol::status::RegisterMode::DiscreteInputs
            {
                let lbl = if raw_val != 0 {
                    lang().protocol.modbus.value_true.as_str()
                } else {
                    lang().protocol.modbus.value_false.as_str()
                };
                if is_chosen_value {
                    spans.push(Span::styled(format!("[{}]", lbl), style));
                } else {
                    spans.push(Span::styled(lbl.to_string(), style));
                }
            } else {
                if is_chosen_value {
                    spans.push(Span::styled(format!("[{:04X}]", raw_val), style));
                } else {
                    spans.push(Span::styled(format!("{:04X}", raw_val), style));
                }
            }
        }
        out.push(Line::from(spans));
        addr = line_base + 8;
    }
}

fn render_scroll_bar(
    f: &mut Frame,
    area: Rect,
    first_visible: usize,
    inner_height: usize,
    total: usize,
) {
    let bar_x = area.x + area.width.saturating_sub(1);
    let bar_y = area.y + 1;
    let bar_h = area.height.saturating_sub(2);
    let denom = (total.saturating_sub(inner_height)) as f32;
    let ratio = if denom > 0.0 {
        first_visible as f32 / denom
    } else {
        0.0
    };
    let thumb = bar_y + ((ratio * (bar_h.saturating_sub(1) as f32)).round() as u16);
    for i in 0..bar_h {
        let ch = if bar_y + i == thumb { '█' } else { '│' };
        let p = ratatui::widgets::Paragraph::new(ch.to_string())
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, Rect::new(bar_x, bar_y + i, 1, 1));
    }
}

// End of unified ModBus panel
