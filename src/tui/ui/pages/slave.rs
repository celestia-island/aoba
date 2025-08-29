use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

use crate::i18n::lang;
use crate::protocol::status::{Parity, Status};
use ratatui::style::{Color, Style, Modifier};
use ratatui::text::{Span, Line};

/// UI for configuring Modbus 主站 (master) settings for the selected port.
pub fn render_slave(f: &mut Frame, area: Rect, app: &Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };

    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(port_name.clone(), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(" - {}", lang().tab_master.as_str())),
    ]);

    let block = Block::default().title(title_line).borders(ratatui::widgets::Borders::ALL);

    let form = app.subpage_form.as_ref().cloned().unwrap_or_default();

    let mut lines: Vec<ratatui::text::Line> = Vec::new();
    let push_field = |lines: &mut Vec<ratatui::text::Line>,
                      idx: usize,
                      label: &str,
                      value: String,
                      editing: bool,
                      buffer: &str| {
        if editing {
            let s = format!("{}: {} [编辑: {}]", label, value, buffer);
            lines.push(ratatui::text::Line::from(Span::styled(
                s,
                Style::default().fg(Color::Yellow),
            )));
        } else if idx == form.cursor {
            let s = format!("{}: {}", label, value);
            lines.push(ratatui::text::Line::from(Span::styled(
                s,
                Style::default().bg(Color::LightBlue),
            )));
        } else {
            lines.push(ratatui::text::Line::from(format!("{}: {}", label, value)));
        }
    };
    let editing_field = form.editing_field.clone();
    push_field(
        &mut lines,
        0,
        "波特率",
        form.baud.to_string(),
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::Baud)
        ),
        form.input_buffer.as_str(),
    );
    let parity_text = match form.parity {
        Parity::None => "None".to_string(),
        Parity::Even => "Even".to_string(),
        Parity::Odd => "Odd".to_string(),
    };
    push_field(
        &mut lines,
        1,
        "校验",
        parity_text,
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::Parity)
        ),
        form.input_buffer.as_str(),
    );
    push_field(
        &mut lines,
        2,
        "停止位",
        form.stop_bits.to_string(),
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::StopBits)
        ),
        form.input_buffer.as_str(),
    );
    lines.push(ratatui::text::Line::from(""));
    lines.push(ratatui::text::Line::from("寄存器列表:"));
    for (i, r) in form.registers.iter().enumerate() {
        let idx = 3 + i;
        let line_text = format!(
            "{}. slave={} mode={} addr={} len={}",
            i + 1,
            r.slave_id,
            r.mode,
            r.address,
            r.length
        );
        if form.cursor == idx {
            lines.push(ratatui::text::Line::from(Span::styled(
                line_text,
                Style::default().bg(Color::LightBlue),
            )));
        } else {
            lines.push(ratatui::text::Line::from(line_text));
        }
    }

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(para, area);
}

/// Handle key events when slave page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    if let Some(form) = app.subpage_form.as_ref() {
        if form.editing {
            return true;
        }
    }

    match key.code {
        KC::Enter => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.editing = !form.editing;
                if form.editing {
                    form.input_buffer.clear();
                } else {
                    form.editing_field = None;
                    form.input_buffer.clear();
                }
            }
            return true;
        }
        KC::Char('e') => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.editing = !form.editing;
                if form.editing {
                    form.input_buffer.clear();
                }
            }
            return true;
        }
        _ => {}
    }
    false
}
