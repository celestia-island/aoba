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
        // When inside a subpage, show how to switch modes with Tab and how to quit
        let sub_hint = lang().hint_switch_tab.as_str().to_string();
        let instr_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::DarkGray));
        let instr_p = Paragraph::new(format!(
            "{}   {}   {}",
            instr,
            sub_hint,
            lang().press_q_quit.as_str()
        ))
        .alignment(Alignment::Center)
        .block(instr_block);
        f.render_widget(instr_p, rows[1]);
    } else {
        let help_block = help_block.style(Style::default().bg(Color::Gray).fg(Color::White));

        // Build hints as a list so we can guarantee the quit hint is always last.
        let mut hints: Vec<String> = Vec::new();
        if _app.active_subpage.is_some() {
            hints.push(lang().hint_back_list.as_str().to_string());
            // also show that Tab switches modes while in subpage
            hints.push(lang().hint_switch_tab.as_str().to_string());
        } else {
            // first hint: switching COM ports with Up/Down or k/j
            hints.push(lang().hint_move_vertical.as_str().to_string());
            // second hint: press 'l' to enter subpage (Right arrow intentionally disabled)
            hints.push(lang().hint_enter_subpage.as_str().to_string());
        }

        // if selected port is occupied by this app and no subpage overlay is active,
        // add mode menu hint before the quit hint. When a subpage is active 'm' is
        // intentionally disabled so we don't show the hint.
        let state = _app
            .port_states
            .get(_app.selected)
            .cloned()
            .unwrap_or(crate::protocol::status::PortState::Free);
        if state == crate::protocol::status::PortState::OccupiedByThis
            && _app.active_subpage.is_none()
        {
            hints.push(lang().hint_mode_menu.as_str().to_string());
        }

        // Always append quit hint as the last item
        hints.push(lang().press_q_quit.as_str().to_string());

        let text = hints.join("   ");
        let help = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(help_block);
        f.render_widget(help, area);
    }
}
