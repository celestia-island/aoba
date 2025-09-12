use ratatui::{
    prelude::*,
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::Status,
    tui::ui::components::{
        render_boxed_paragraph, styled_spans, styled_title_span, StyledSpanKind, TextState,
    },
    tui::utils::bus::Bus,
};

/// Render a configuration panel for a subpage. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, app: &Status, style: Option<Style>) {
    // Use subpage_form if present
    let form = if let Some(ref form) = app.page.subpage_form {
        form
    } else {
        // No form, render placeholder
        let lines = vec![ratatui::text::Line::from("No form loaded")];
        return render_boxed_paragraph(f, area, lines, style);
    };

    let mut lines: Vec<ratatui::text::Line> = Vec::new();

    // Helper to push a possibly highlighted field as a single line composed of multiple Spans:
    // - title (bold)
    // - filler spaces
    // - value (not bold, colored depending on state)
    // Uses unicode width to correctly align CJK and ASCII text.
    let push_field = |lines: &mut Vec<ratatui::text::Line>,
                      idx: usize,
                      label: &str,
                      value: String,
                      editing: bool,
                      buffer: &str| {
        let selected = idx == form.cursor;
        let base_prefix = "  ";
        let label_text = format!("{base_prefix}{label}");
        let left_width = 36usize; // label column width in character cells

        let right_text = if editing {
            lang().protocol.modbus.edit_suffix.replace("{}", buffer)
        } else if selected {
            format!("> {value}")
        } else {
            value
        };

        let label_w = UnicodeWidthStr::width(label_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };
        let filler = " ".repeat(pad_count);

        let mut spans = Vec::new();
        spans.push(styled_title_span(label_text.as_str(), selected, true));
        spans.push(Span::raw(filler));

        let state = if editing {
            TextState::Editing
        } else if selected {
            TextState::Selected
        } else {
            TextState::Normal
        };
        spans.append(&mut styled_spans(StyledSpanKind::Text {
            text: right_text.as_str(),
            state,
            bold: false,
        }));

        lines.push(ratatui::text::Line::from(spans));
    };

    // Global Interval (idx 0) - simplified version
    let _selected = 0 == form.cursor;
    let val = "Global interval display placeholder".to_string();
    push_field(&mut lines, 0, "Global Interval", val, false, "");

    // Master Passive (idx 1) - simplified
    let master_passive_label = "Master Passive";
    let master_passive_val = "Configuration display placeholder".to_string();
    push_field(
        &mut lines,
        1,
        master_passive_label,
        master_passive_val,
        false,
        "",
    );

    // Basic configuration fields display - simplified
    let baud_val = "9600".to_string();
    push_field(&mut lines, 2, "Baud Rate", baud_val, false, "");

    let data_bits_val = "8".to_string();
    push_field(&mut lines, 3, "Data Bits", data_bits_val, false, "");

    let parity_val = "None".to_string();
    push_field(&mut lines, 4, "Parity", parity_val, false, "");

    let stop_bits_val = "1".to_string();
    push_field(&mut lines, 5, "Stop Bits", stop_bits_val, false, "");

    render_boxed_paragraph(f, area, lines, style);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    hints.push("Enter: Edit".to_string());
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(_key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match _key.code {
        KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
            // Navigation
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Enter => {
            // Edit field
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Esc => {
            // Cancel edit
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false
    }
}