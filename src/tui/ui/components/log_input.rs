use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{i18n::lang, protocol::status::types::ui::InputMode, protocol::status::types::Status};

/// Render a small input area showing current mode and buffer. Height expected to be small (3 lines).
pub fn render_log_input(f: &mut Frame, area: Rect, app: &mut Status, selected_port: usize) {
    let mut lines: Vec<Line> = Vec::new();

    // For now, use ASCII mode as default since we don't have per-port input modes
    let input_mode = InputMode::Ascii;
    
    // Show buffer on the first content line (right under the title)
    let content = if input_mode == InputMode::Hex {
        let mut s = String::new();
        let mut chars = app
            .temporarily
            .input_raw_buffer
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
        app.temporarily.input_raw_buffer.clone()
    };

    // If buffer empty and not editing, show faint gray italic placeholder indicating current input mode
    if content.is_empty() {
        // Show as: " {input_mode_current} {mode_text}" with a leading space to align with title
        let mode_text = match input_mode {
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
        // Style editing content in yellow (match config page editing color)
        // Build spans in-place
        let spans = vec![
            Span::raw(" "), // Prefix editing line with single space (log list uses 2 incl. selector; here we keep compact)
            Span::styled(content.clone(), Style::default().fg(Color::Yellow)),
            // Visual cursor block (green background)
            Span::styled(" ", Style::default().bg(Color::Green).fg(Color::Black)),
        ];
        lines.push(ratatui::text::Line::from(spans));
    }

    // Keep a spare empty middle line so layout remains 3 lines
    // Spare empty middle line should also keep two spaces for alignment
    lines.push(Line::from(Span::raw(" ")));

    // Hint (short)
    // Show dual-key hint: Enter / i
    let edit_hint = format!(
        "[Enter / i] {}",
        lang().input.hint_input_edit_short.as_str()
    );
    lines.push(Line::from(Span::raw(format!(" {edit_hint}"))));

    // Choose title and block based on whether we're editing
    let (_title, block) = if !content.is_empty() {
        // Highlight title when editing (use library green)
        let title = Span::styled(
            format!(" {} ", lang().input.input_label.as_str()),
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
        (
            title.clone(),
            Block::default().borders(Borders::ALL).title(title),
        )
    } else {
        let title_text = format!(" {} ", lang().input.input_label.as_str());
        (
            Span::raw(title_text.clone()),
            Block::default().borders(Borders::ALL).title(title_text),
        )
    };

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, area);
}
