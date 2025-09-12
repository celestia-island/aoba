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

    // Global Interval (idx 0)
    let global_interval_editing = form.editing && form.cursor == 0;
    let global_interval_val = form.global_interval_ms.to_string();
    push_field(&mut lines, 0, "Global Interval (ms)", global_interval_val, global_interval_editing, &form.input_buffer);

    // Master Passive (idx 1) - toggle field
    let master_passive_label = "Master Passive";
    let master_passive_val = match form.master_passive {
        Some(true) => "Enabled".to_string(),
        Some(false) => "Disabled".to_string(),
        None => "Auto".to_string(),
    };
    push_field(
        &mut lines,
        1,
        master_passive_label,
        master_passive_val,
        false,
        "",
    );

    // Baud Rate (idx 2)
    let baud_editing = form.editing && form.cursor == 2;
    let baud_val = form.baud.to_string();
    push_field(&mut lines, 2, "Baud Rate", baud_val, baud_editing, &form.input_buffer);

    // Data Bits (idx 3)
    let data_bits_editing = form.editing && form.cursor == 3;
    let data_bits_val = form.data_bits.to_string();
    push_field(&mut lines, 3, "Data Bits", data_bits_val, data_bits_editing, &form.input_buffer);

    // Parity (idx 4)
    let parity_editing = form.editing && form.cursor == 4;
    let parity_val = match form.parity {
        serialport::Parity::None => "None".to_string(),
        serialport::Parity::Odd => "Odd".to_string(),
        serialport::Parity::Even => "Even".to_string(),
    };
    push_field(&mut lines, 4, "Parity", parity_val, parity_editing, &form.input_buffer);

    // Stop Bits (idx 5)
    let stop_bits_editing = form.editing && form.cursor == 5;
    let stop_bits_val = form.stop_bits.to_string();
    push_field(&mut lines, 5, "Stop Bits", stop_bits_val, stop_bits_editing, &form.input_buffer);

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
pub fn handle_input(key: crossterm::event::KeyEvent, bus: &Bus) -> bool {
    use crossterm::event::KeyCode as KC;

    match key.code {
        KC::Up | KC::Char('k') => {
            // Navigate up in config fields
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::NavigateUp);
            true
        }
        KC::Down | KC::Char('j') => {
            // Navigate down in config fields
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::NavigateDown);
            true
        }
        KC::Enter => {
            // Begin editing the selected field OR confirm current edit
            send_config_action(bus, ConfigAction::StartEditOrConfirm);
            true
        }
        KC::Esc => {
            // Cancel editing and go back
            send_config_action(bus, ConfigAction::CancelEdit);
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::GoBack);
            true
        }
        KC::Char(c) if c.is_ascii_digit() || c == 'n' || c == 'e' || c == 'o' => {
            // Input for editing (digits for numbers, n/e/o for parity)
            send_config_action(bus, ConfigAction::InputChar(c));
            true
        }
        KC::Backspace => {
            // Delete character during editing
            send_config_action(bus, ConfigAction::Backspace);
            true
        }
        _ => false
    }
}

#[derive(Debug)]
enum ConfigAction {
    StartEditOrConfirm,
    CancelEdit,
    InputChar(char),
    Backspace,
}

fn send_config_action(bus: &Bus, action: ConfigAction) {
    match action {
        ConfigAction::StartEditOrConfirm => {
            log::debug!("[CONFIG] Start edit or confirm");
            // We'll let the core determine if this should start edit or confirm
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::StartEdit);
        }
        ConfigAction::CancelEdit => {
            log::debug!("[CONFIG] Canceling edit mode");
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::CancelEdit);
        }
        ConfigAction::InputChar(c) => {
            log::debug!("[CONFIG] Input char: {}", c);
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::InputChar(c));
        }
        ConfigAction::Backspace => {
            log::debug!("[CONFIG] Backspace");
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Backspace);
        }
    }
}