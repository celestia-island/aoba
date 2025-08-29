use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

use crate::i18n::lang;
use crate::protocol::status::{Parity, Status};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// UI for configuring Modbus 从站 (pull) settings for the selected port.
pub fn render_pull(f: &mut Frame, area: Rect, app: &Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };

    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            port_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" - {}", lang().tab_slave.as_str())),
    ]);

    let block = Block::default()
        .title(title_line)
        .borders(ratatui::widgets::Borders::ALL);

    // Use transient form if present
    let form = app.subpage_form.as_ref().cloned().unwrap_or_default();

    let mut lines: Vec<ratatui::text::Line> = Vec::new();
    // Helper to push a possibly highlighted line
    let push_field = |lines: &mut Vec<ratatui::text::Line>,
                      idx: usize,
                      label: &str,
                      value: String,
                      editing: bool,
                      buffer: &str| {
        if editing {
            let s = format!("{} {}", label, lang().edit_suffix.replace("{}", buffer));
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
    // Baud
    push_field(
        &mut lines,
        0,
        lang().label_baud.as_str(),
        form.baud.to_string(),
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::Baud)
        ),
        form.input_buffer.as_str(),
    );
    // Parity
    let parity_text = match form.parity {
        Parity::None => "None".to_string(),
        Parity::Even => "Even".to_string(),
        Parity::Odd => "Odd".to_string(),
    };
    push_field(
        &mut lines,
        1,
        lang().label_parity.as_str(),
        parity_text,
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::Parity)
        ),
        form.input_buffer.as_str(),
    );
    // Stop bits
    push_field(
        &mut lines,
        2,
        lang().label_stop_bits.as_str(),
        form.stop_bits.to_string(),
        matches!(
            editing_field,
            Some(crate::protocol::status::EditingField::StopBits)
        ),
        form.input_buffer.as_str(),
    );
    lines.push(ratatui::text::Line::from(""));
    lines.push(ratatui::text::Line::from(lang().registers_list.as_str()));
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

/// Handle key events when pull page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // If editing, let the global editing handler take care of chars/backspace/enter
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
        KC::Char('n') => {
            // add register
            if app.subpage_form.is_none() {
                app.init_subpage_form();
            }
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.push(crate::protocol::status::RegisterEntry {
                    slave_id: 1,
                    mode: 1,
                    address: 0,
                    length: 1,
                });
            }
            return true;
        }
        KC::Char('d') => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.pop();
            }
            return true;
        }
        KC::Down | KC::Char('j') => {
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 3usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = (form.cursor + 1) % total;
                }
            }
            return true;
        }
        KC::Up | KC::Char('k') => {
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 3usize.saturating_add(form.registers.len());
                if total > 0 {
                    if form.cursor == 0 {
                        form.cursor = total - 1;
                    } else {
                        form.cursor -= 1;
                    }
                }
            }
            return true;
        }
        _ => {}
    }
    false
}
