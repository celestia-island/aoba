use crate::i18n::lang;
use crate::protocol::status::{InputMode, Status};
use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Render a small input area showing current mode and buffer. Height expected to be small (3 lines).
pub fn render_log_input(f: &mut Frame, area: Rect, app: &mut Status) {
    let mut lines: Vec<Line> = Vec::new();

    if app.input_editing {
        // Show buffer on the first content line (right under the title)
        let content = if app.input_mode == InputMode::Hex {
            let mut s = String::new();
            let mut chars = app
                .input_buffer
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            if chars.len() % 2 == 1 {
                chars.push('_');
            }
            for (i, ch) in chars.chars().enumerate() {
                if i > 0 && i % 2 == 0 {
                    s.push(' ');
                }
                s.push(ch);
            }
            s
        } else {
            app.input_buffer.clone()
        };
        // Style editing content in yellow (match config page editing color)
        // Build spans in-place
        let spans = vec![
            Span::raw(" "), // Prefix editing line with single space (log list uses 2 incl. selector; here we keep compact)
            Span::styled(content.clone(), Style::default().fg(Color::Yellow)),
            // Visual cursor block (green background)
            Span::styled(" ", Style::default().bg(Color::Green).fg(Color::Black)),
        ];
        lines.push(ratatui::text::Line::from(spans));
        // Keep a spare empty middle line so layout remains 3 lines
        // Spare empty middle line should also keep two spaces for alignment
        lines.push(Line::from(Span::raw(" ")));
        // Editing hint (submit)
        lines.push(Line::from(Span::styled(
            format!(" {}", lang().hotkeys.press_enter_submit.as_str()),
            Style::default().fg(Color::LightGreen),
        )));

        // Highlight title when editing (use library green)
        let title = Span::styled(
            format!(" {} ", lang().input.input_label.as_str()),
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
        let block = Block::default().borders(Borders::ALL).title(title);
        let para = Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(para, area);

        // We render a visual cursor block; avoid moving the terminal cursor so its color
        // (terminal-managed) doesn't interfere with our visual cursor.
        return;
    }

    // Not editing: buffer preview and short hint (mode shown in title on right)

    // Buffer preview (or placeholder when empty)
    let content = if app.input_mode == InputMode::Hex {
        let mut s = String::new();
        let mut chars = app
            .input_buffer
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        if chars.len() % 2 == 1 {
            chars.push('_');
        }
        for (i, ch) in chars.chars().enumerate() {
            if i > 0 && i % 2 == 0 {
                s.push(' ');
            }
            s.push(ch);
        }
        s
    } else {
        app.input_buffer.clone()
    };
    // If buffer empty and not editing, show faint gray italic placeholder indicating current input mode
    if content.is_empty() {
        // Show as: " {input_mode_current} {mode_text}" with a leading space to align with title
        let mode_text = match app.input_mode {
            InputMode::Ascii => lang().input.input_mode_ascii.as_str(),
            InputMode::Hex => lang().input.input_mode_hex.as_str(),
        };
        // Add extra leading spaces to align with title
        let placeholder = format!(
            " {} {}",
            lang().input.input_mode_current.as_str(),
            mode_text
        ); // Keep single leading space
        lines.push(Line::from(Span::styled(
            placeholder,
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        // Raw content line also prefixed with two spaces for consistent alignment
        lines.push(Line::from(Span::raw(format!("  {content}"))));
    }

    // Hint (short)
    // Show dual-key hint: Enter / i
    let edit_hint = format!(
        "[Enter / i] {}",
        lang().input.hint_input_edit_short.as_str()
    );
    lines.push(Line::from(Span::raw(format!(" {edit_hint}"))));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", lang().input.input_label.as_str()));
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, area);
}
