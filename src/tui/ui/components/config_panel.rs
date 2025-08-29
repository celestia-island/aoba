use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::Span,
};

use crate::{
    i18n::lang,
    protocol::status::{EditingField, Status},
};

/// Render a configuration panel for a subpage. Reads `app.subpage_form` and renders fields.
pub fn render_config_panel(f: &mut Frame, area: Rect, app: &Status, style: Option<Style>) {
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
        matches!(editing_field, Some(EditingField::Baud)),
        form.input_buffer.as_str(),
    );
    // Parity
    let parity_text = match form.parity {
        crate::protocol::status::Parity::None => lang().parity_none.clone(),
        crate::protocol::status::Parity::Even => lang().parity_even.clone(),
        crate::protocol::status::Parity::Odd => lang().parity_odd.clone(),
    };
    push_field(
        &mut lines,
        1,
        lang().label_parity.as_str(),
        parity_text,
        matches!(editing_field, Some(EditingField::Parity)),
        form.input_buffer.as_str(),
    );
    // Stop bits
    push_field(
        &mut lines,
        2,
        lang().label_stop_bits.as_str(),
        form.stop_bits.to_string(),
        matches!(editing_field, Some(EditingField::StopBits)),
        form.input_buffer.as_str(),
    );

    crate::tui::ui::components::render_boxed_paragraph(f, area, lines, style);
}
