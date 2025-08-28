use ratatui::{prelude::*, widgets::*};

use crate::{i18n::lang, protocol::status::Status};

pub fn render_bottom(f: &mut Frame, area: Rect, _app: &Status) {
    let help_block = Block::default().borders(Borders::NONE);

    // If app has an error message, display it on the first line (red),
    // and on the second line show instructions on how to clear it.
    if let Some(err) = &_app.error {
        // Split the provided area into two rows
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(area);

        let err_block = help_block.clone().style(
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        let msg = &err.0;
        let p = Paragraph::new(msg.as_str())
            .alignment(Alignment::Left)
            .block(err_block);
        f.render_widget(p, rows[0]);

        let instr = lang().press_c_clear.as_str().to_string();
        let instr_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::DarkGray));
        let instr_p = Paragraph::new(format!("{}   {}", instr, lang().press_q_quit.as_str()))
            .alignment(Alignment::Center)
            .block(instr_block);
        f.render_widget(instr_p, rows[1]);
    } else {
        let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::White));
        // Base hint
        let mut hint = lang().hint_line.as_str().to_string();
        // Determine if selected is a virtual trailing item
        let is_virtual = !_app.ports.is_empty() && _app.selected >= _app.ports.len();
        // Select appropriate movement/focus hint depending on whether selected port is occupied by this app
        if !_app.ports.is_empty() && !is_virtual {
            let state = _app
                .port_states
                .get(_app.selected)
                .cloned()
                .unwrap_or(crate::protocol::status::PortState::Free);
            if state == crate::protocol::status::PortState::OccupiedByThis {
                hint = lang().hint_move_with_panels.as_str().to_string();
            } else {
                hint = lang().hint_move_vertical.as_str().to_string();
            }
        } else if is_virtual {
            // virtual items don't allow switching panels
            hint = lang().hint_move_vertical.as_str().to_string();
        }

        // If focus is on ports (left) and there are ports, show context-aware Enter hint
        if matches!(_app.focus, crate::protocol::status::Focus::Left) && !_app.ports.is_empty() {
            // determine selected port state
            let is_virtual = _app.selected >= _app.ports.len();
            // determine selected port state (for real ports)
            let state = if !is_virtual {
                _app.port_states
                    .get(_app.selected)
                    .cloned()
                    .unwrap_or(crate::protocol::status::PortState::Free)
            } else {
                // treat virtual items as not-occupied
                crate::protocol::status::PortState::Free
            };
            // build enter hint; when occupied by other, remove any leading 'Press Enter' prefix
            let enter = match state {
                crate::protocol::status::PortState::Free => {
                    // for virtual items, if refresh selected show Enter -> refresh label
                    if is_virtual {
                        // determine which virtual item
                        let rel = _app.selected - _app.ports.len();
                        if rel == 0 {
                            // Refresh: show Press Enter to Refresh
                            format!("{}", lang().press_enter_refresh.as_str())
                        } else {
                            // Manual specify: show Press Enter to manual specify
                            format!("{}", lang().press_enter_manual_specify.as_str())
                        }
                    } else {
                        lang().press_enter_enable.as_str().to_string()
                    }
                }
                crate::protocol::status::PortState::OccupiedByThis => {
                    lang().press_enter_release.as_str().to_string()
                }
                crate::protocol::status::PortState::OccupiedByOther => {
                    // remove common "Press Enter" prefixes for different locales if present
                    let s = lang().press_enter_unavailable.as_str();
                    let s = s.trim();
                    let patterns = [
                        "按 Enter ",
                        "Press Enter ",
                        "按 Enter—",
                        "Press Enter—",
                        "按 Enter — ",
                        "Press Enter — ",
                    ];
                    let mut out = s.to_string();
                    for p in patterns.iter() {
                        if out.starts_with(p) {
                            out = out[p.len()..].trim_start().to_string();
                            break;
                        }
                    }
                    out
                }
            };
            hint = format!("{}   {}", hint, enter);
        }
        // always append quit hint
        let quit = lang().press_q_quit.as_str();
        let full_hint = format!("{}   {}", hint, quit);
        let help = Paragraph::new(full_hint)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(help_block);
        f.render_widget(help, area);
    }
}
