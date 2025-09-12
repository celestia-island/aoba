use ratatui::{
    prelude::*,
    style::Modifier,
    style::{Color, Style},
    text::Span,
};
use serialport::Parity;
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{EditingField, Status},
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
        spans.append(&mut styled_spans(
            right_text.as_str(),
            state,
            StyledSpanKind::Value,
        ));

        lines.push(ratatui::text::Line::from(spans));
    };

    // Global Interval (idx 0)
    if matches!(form.editing_field, Some(EditingField::GlobalInterval)) {
        let base_prefix = "  ";
        let label = lang().protocol.modbus.global_interval.as_str();
        let label_text = format!("{base_prefix}{label}");
        let left_width = 36usize;
        let label_w = UnicodeWidthStr::width(label_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };
        let filler = " ".repeat(pad_count);

        let mut spans = Vec::new();
        spans.push(styled_title_span(label_text.as_str(), true, true));
        spans.push(Span::raw(filler));

        let buf = if form.input_buffer.is_empty() {
            "_".to_string()
        } else {
            form.input_buffer.clone()
        };
        spans.append(&mut styled_spans(
            format!("[{}]", buf).as_str(),
            TextState::Editing,
            StyledSpanKind::Value,
        ));
        lines.push(ratatui::text::Line::from(spans));
    } else {
        let selected = 0 == form.cursor;
        let val = if let Some(i) = form.global_interval_ms {
            format!("{i} ms")
        } else {
            format!(
                "{} ({})",
                "default",
                lang().protocol.modbus.label_default_marker.clone()
            )
        };
        push_field(&mut lines, 0, lang().protocol.modbus.global_interval.as_str(), val, false, "");
    }

    // Master Passive (idx 1)
    let derived_default_passive = form.port_cfg.as_ref().map(|c| c.baud > 9600).unwrap_or(false);
    let master_passive_label = lang().protocol.modbus.master_passive.as_str();
    let master_passive_val = match form.master_passive {
        Some(true) => lang().protocol.modbus.status_master_passive.clone(),
        Some(false) => lang().protocol.modbus.status_master_active.clone(),
        None => {
            if derived_default_passive {
                format!(
                    "{} ({})",
                    lang().protocol.modbus.status_master_passive.clone(),
                    lang().protocol.modbus.label_default_marker.clone()
                )
            } else {
                format!(
                    "{} ({})",
                    lang().protocol.modbus.status_master_active.clone(),
                    lang().protocol.modbus.label_default_marker.clone()
                )
            }
        }
    };
    push_field(
        &mut lines,
        1,
        master_passive_label,
        master_passive_val,
        false,
        "",
    );

    // Basic configuration fields display
    if let Some(ref cfg) = form.port_cfg {
        let baud_val = cfg.baud.to_string();
        push_field(&mut lines, 2, "Baud Rate", baud_val, false, "");

        let data_bits_val = cfg.data_bits.to_string();
        push_field(&mut lines, 3, "Data Bits", data_bits_val, false, "");

        let parity_val = match cfg.parity {
            Parity::None => "None",
            Parity::Even => "Even",
            Parity::Odd => "Odd",
        };
        push_field(&mut lines, 4, "Parity", parity_val.to_string(), false, "");

        let stop_bits_val = cfg.stop_bits.to_string();
        push_field(&mut lines, 5, "Stop Bits", stop_bits_val, false, "");
    }

    render_boxed_paragraph(f, area, lines, style);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    hints.push(lang().hotkeys.hint_edit_field.as_str().to_string());
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