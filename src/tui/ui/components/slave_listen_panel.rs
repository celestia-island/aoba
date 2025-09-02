use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::Line,
};

use crate::{
    i18n::lang,
    protocol::status::{RegisterEntry, Status},
    tui::ui::components::render_boxed_paragraph,
};

/// Slave listen panel: shows passive register stacks with per-entry refresh + counters line.
pub fn render_slave_listen_panel(f: &mut Frame, area: Rect, app: &mut Status) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(form) = app.subpage_form.as_ref() {
        if form.registers.is_empty() {
            lines.push(Line::from(lang().protocol.label_slave_listen.as_str()));
        } else {
            for (i, r) in form.registers.iter().enumerate() {
                // First line already merged: refresh rate + counter
                render_entry_header(&mut lines, i, r, form, app);
                render_entry_values(&mut lines, i, r, form, app);
                lines.push(Line::from(""));
            }
        }
    } else {
        lines.push(Line::from(lang().index.details_placeholder.as_str()));
    }
    render_boxed_paragraph(f, area, lines, None);
}

fn render_entry_header(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
    _app: &Status,
) {
    use crate::protocol::status::MasterEditField as MEF;
    let selected = form.master_cursor == idx;
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing_field = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };

    let browse_style = Style::default().fg(Color::Green);
    // Style rule: selected = plain yellow, editing = bold yellow
    let chosen_style = Style::default().fg(Color::Yellow); // selected (not editing)
    let active_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD); // editing
    let normal_style = Style::default();

    let base_style = if selected { browse_style } else { normal_style };
    let mut spans: Vec<Span> = Vec::new();
    if selected {
        spans.push(Span::raw("> "));
    }
    spans.push(Span::raw(format!("#{}", idx + 1)));
    spans.push(Span::raw(", "));

    // ID
    let id_active = matches!(cur_field, Some(MEF::Id)) && editing_field;
    let id_style = if id_active {
        active_style
    } else if chosen && matches!(cur_field, Some(MEF::Id)) {
        chosen_style
    } else {
        base_style
    };
    if id_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("ID = [{}]", content), id_style));
    } else {
        spans.push(Span::styled(format!("ID = {:02X}", r.slave_id), id_style));
    }
    spans.push(Span::raw(", "));

    // Type (non-editable in listen mode except cycle placeholder?) still use same visuals
    let types = [
        lang().protocol.reg_type_coils.as_str(),
        lang().protocol.reg_type_discrete_inputs.as_str(),
        lang().protocol.reg_type_holding.as_str(),
        lang().protocol.reg_type_input.as_str(),
    ];
    let cur_type_idx = ((r.mode as u8 as usize).saturating_sub(1)).min(3);
    let type_style = if chosen && matches!(cur_field, Some(MEF::Type)) {
        if editing_field {
            active_style
        } else {
            chosen_style
        }
    } else {
        base_style
    };
    spans.push(Span::styled(types[cur_type_idx].to_string(), type_style));
    spans.push(Span::raw(", "));
    // Address range
    let start_active = matches!(cur_field, Some(MEF::Start)) && editing_field;
    let start_style = if start_active {
        active_style
    } else if chosen && matches!(cur_field, Some(MEF::Start)) {
        chosen_style
    } else {
        base_style
    };
    let end_active = matches!(cur_field, Some(MEF::End)) && editing_field;
    let end_style = if end_active {
        active_style
    } else if chosen && matches!(cur_field, Some(MEF::End)) {
        chosen_style
    } else {
        base_style
    };
    if start_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("0x[{}]", content), start_style));
    } else {
        spans.push(Span::styled(format!("0x{:04X}", r.address), start_style));
    }
    spans.push(Span::raw(" - "));
    if end_active {
        let content = if form.master_input_buffer.is_empty() {
            "_"
        } else {
            form.master_input_buffer.as_str()
        };
        spans.push(Span::styled(format!("0x[{}]", content), end_style));
    } else {
        let end_inclusive = r.address as u32 + r.length as u32 - 1;
        spans.push(Span::styled(format!("0x{:04X}", end_inclusive), end_style));
    }

    // Append refresh rate and request counter
    spans.push(Span::raw(", "));
    let refresh_active = matches!(cur_field, Some(MEF::Refresh)) && editing_field;
    let refresh_selected = chosen && matches!(cur_field, Some(MEF::Refresh));
    let refresh_style = if refresh_active {
        active_style
    } else if refresh_selected {
        chosen_style
    } else {
        base_style
    };
    let refresh_label = lang().protocol.refresh_rate.as_str();
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
    } else {
        spans.push(Span::styled(
            format!("{} = {} ms", refresh_label, r.refresh_ms),
            refresh_style,
        ));
    }
    spans.push(Span::raw(", "));
    let counter_label = lang().protocol.label_req_counter.as_str();
    let counter_selected = chosen && matches!(cur_field, Some(MEF::Counter));
    let counter_style = if counter_selected {
        chosen_style
    } else {
        Style::default().fg(Color::Green)
    };
    spans.push(Span::styled(
        format!("{} = {} / {}", counter_label, r.req_success, r.req_total),
        counter_style,
    ));
    out.push(Line::from(spans));
}

// Legacy: render_entry_refresh_line merged into header (stub retained for potential future reference)
#[allow(dead_code)]
fn render_entry_refresh_line(
    _out: &mut Vec<Line>,
    _: usize,
    _: &RegisterEntry,
    _: &crate::protocol::status::SubpageForm,
    _: &Status,
) {
}

fn render_entry_values(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
    _app: &Status,
) {
    use crate::protocol::status::MasterEditField as MEF;
    if r.length == 0 {
        return;
    }
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing_field = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };
    let start = r.address as usize;
    let end_exclusive = start + r.length as usize;
    let mut addr = start;
    while addr < end_exclusive {
        let line_base = (addr / 8) * 8;
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(format!("      0x{:04X}: ", line_base))); // six leading spaces for alignment
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
            let val = r.values.get(offset).cloned().unwrap_or(0);
            let style = if let Some(MEF::Value(a)) = &cur_field {
                if *a as usize == cur && editing_field {
                    // 编辑中 -> 加粗黄色
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if !editing_field && *a as usize == cur {
                    // 仅选中 -> 普通黄色
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                }
            } else if chosen {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            if let Some(MEF::Value(a)) = &cur_field {
                if *a as usize == cur && editing_field {
                    let content = if form.master_input_buffer.is_empty() {
                        "_"
                    } else {
                        form.master_input_buffer.as_str()
                    };
                    spans.push(Span::styled(format!("[{}]", content), style));
                    continue;
                }
            }
            spans.push(Span::styled(format!("{:02X}", val), style));
        }
        out.push(Line::from(spans));
        addr = line_base + 8;
    }
}
