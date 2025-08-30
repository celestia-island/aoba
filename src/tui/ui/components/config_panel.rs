use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::Span,
};

use crate::{
    i18n::lang,
    protocol::status::{EditingField, Status},
};
use ratatui::style::Modifier;

/// Render a configuration panel for a subpage. Reads `app.subpage_form` and renders fields.
pub fn render_config_panel(f: &mut Frame, area: Rect, app: &mut Status, style: Option<Style>) {
    // Use transient form if present
    let form = app.subpage_form.as_ref().cloned().unwrap_or_default();

    let mut lines: Vec<ratatui::text::Line> = Vec::new();

    // Helper to push a possibly highlighted field as two lines: title (bold) and value
    let push_field = |lines: &mut Vec<ratatui::text::Line>,
                      idx: usize,
                      label: &str,
                      value: String,
                      editing: bool,
                      buffer: &str| {
        let selected = idx == form.cursor;
        // Always indent title/value by two spaces so items don't jump when navigating.
        let base_prefix = "  ";

        // Title line (bold) — do NOT add extra selection-indent here, only style changes.
        let title_text = format!("{}{}", base_prefix, label);
        let title_style = if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        lines.push(ratatui::text::Line::from(Span::styled(
            title_text,
            title_style,
        )));

        // Value line: if editing show edit buffer (yellow), otherwise show value (green if selected)
        if editing {
            // In editing state we do not show the selection marker '>' — only show the edit buffer.
            let rendered = lang().edit_suffix.replace("{}", buffer);
            let val_text = format!("{}{}", base_prefix, rendered);
            let val_style = Style::default().fg(Color::Yellow);
            lines.push(ratatui::text::Line::from(Span::styled(val_text, val_style)));
        } else {
            let val_text = if selected {
                format!("{}> {}", base_prefix, value)
            } else {
                format!("{}{}", base_prefix, value)
            };
            let val_style = if selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            lines.push(ratatui::text::Line::from(Span::styled(val_text, val_style)));
        }
    };

    let editing_field = form.editing_field.clone();

    // --- Baud (idx 0) ---
    if matches!(editing_field, Some(EditingField::Baud)) {
        // Presets stop at 115200; append a 'Custom' slot so user can select and type a custom baud
        let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
        let mut options: Vec<String> = presets.iter().map(|p| p.to_string()).collect();
        options.push(lang().custom.clone());
        let custom_idx = options.len() - 1;

        // determine current index in options from form.edit_choice_index when available
        let current_idx = form
            .edit_choice_index
            .or_else(|| presets.iter().position(|&p| p == form.baud))
            .unwrap_or(custom_idx);

        // Build parts showing all options but highlight current
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, opt) in options.iter().enumerate() {
            if i == current_idx {
                parts.push(format!("[{}]", opt));
            } else {
                parts.push(format!(" {} ", opt));
            }
        }
        parts.push("->".to_string());

        // Render title + selector/edit line
        let idx_field = 0usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!("{}{}", base_prefix, lang().label_baud.as_str());
        let title_style = if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        lines.push(ratatui::text::Line::from(Span::styled(
            title_text,
            title_style,
        )));

        // If custom is selected and we're in the deeper confirmed edit stage, show editable buffer
        if current_idx == custom_idx && form.edit_confirmed {
            // confirmed editing: show buffer and yellow highlight
            let rendered = lang().edit_suffix.replace("{}", form.input_buffer.as_str());
            // Editing confirmed: do not show '>' marker; show edit buffer only.
            let val_text = format!("{}{}", base_prefix, rendered);
            lines.push(ratatui::text::Line::from(Span::styled(
                val_text,
                Style::default().fg(Color::Yellow),
            )));
        } else if current_idx == custom_idx && !form.edit_confirmed {
            // custom selected but not yet confirmed: show selector parts (no extra inline hint)
            // Custom selector while editing: do not show selection marker here.
            let val_text = format!("{}{}", base_prefix, parts.join(" "));
            lines.push(ratatui::text::Line::from(Span::styled(
                val_text,
                Style::default().fg(Color::Yellow),
            )));
        } else {
            let val_text = format!("{}{}", base_prefix, parts.join(" "));
            lines.push(ratatui::text::Line::from(Span::styled(
                val_text,
                Style::default().fg(Color::Yellow),
            )));
        }
    } else {
        push_field(
            &mut lines,
            0,
            lang().label_baud.as_str(),
            form.baud.to_string(),
            matches!(editing_field, Some(EditingField::Baud)),
            form.input_buffer.as_str(),
        );
    }

    // --- Parity (idx 1) ---
    let parity_text = match form.parity {
        crate::protocol::status::Parity::None => lang().parity_none.clone(),
        crate::protocol::status::Parity::Even => lang().parity_even.clone(),
        crate::protocol::status::Parity::Odd => lang().parity_odd.clone(),
    };
    if matches!(editing_field, Some(EditingField::Parity)) {
        let options = vec![
            lang().parity_none.clone(),
            lang().parity_even.clone(),
            lang().parity_odd.clone(),
        ];
        let cur_idx = match form.parity {
            crate::protocol::status::Parity::None => 0usize,
            crate::protocol::status::Parity::Even => 1usize,
            crate::protocol::status::Parity::Odd => 2usize,
        };
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, opt) in options.iter().enumerate() {
            if i == cur_idx {
                parts.push(format!("[{}]", opt));
            } else {
                parts.push(format!(" {} ", opt));
            }
        }
        parts.push("->".to_string());

        let idx_field = 1usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!("{}{}", base_prefix, lang().label_parity.as_str());
        let title_style = if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        lines.push(ratatui::text::Line::from(Span::styled(
            title_text,
            title_style,
        )));
        // In editing state do not show the selection marker; show selector parts plainly.
        let val_text = format!("{}{}", base_prefix, parts.join(" "));
        lines.push(ratatui::text::Line::from(Span::styled(
            val_text,
            Style::default().fg(Color::Yellow),
        )));
    } else {
        push_field(
            &mut lines,
            1,
            lang().label_parity.as_str(),
            parity_text,
            matches!(editing_field, Some(EditingField::Parity)),
            form.input_buffer.as_str(),
        );
    }

    // --- Data bits (idx 2) ---
    let data_bits_text = format!("{}", form.data_bits);
    if matches!(editing_field, Some(EditingField::DataBits)) {
        let options = vec![5u8, 6u8, 7u8, 8u8];
        let cur_idx = options
            .iter()
            .position(|&d| d == form.data_bits)
            .unwrap_or(3);
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, opt) in options.iter().enumerate() {
            if i == cur_idx {
                parts.push(format!("[{}]", opt));
            } else {
                parts.push(format!(" {} ", opt));
            }
        }
        parts.push("->".to_string());

        let idx_field = 2usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!("{}{}", base_prefix, lang().label_data_bits.as_str());
        let title_style = if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        lines.push(ratatui::text::Line::from(Span::styled(
            title_text,
            title_style,
        )));
        // Editing selector — don't render '>' marker here.
        let val_text = format!("{}{}", base_prefix, parts.join(" "));
        lines.push(ratatui::text::Line::from(Span::styled(
            val_text,
            Style::default().fg(Color::Yellow),
        )));
    } else {
        push_field(
            &mut lines,
            2,
            lang().label_data_bits.as_str(),
            data_bits_text,
            matches!(editing_field, Some(EditingField::DataBits)),
            form.input_buffer.as_str(),
        );
    }

    // --- Stop bits (idx 3) ---
    if matches!(editing_field, Some(EditingField::StopBits)) {
        let opts_vals = vec![1, 2];
        let cur_idx = opts_vals
            .iter()
            .position(|&s| s == form.stop_bits)
            .unwrap_or(0);
        let opts_labels = vec!["1", "2"];
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, lbl) in opts_labels.iter().enumerate() {
            if i == cur_idx {
                parts.push(format!("[{}]", lbl));
            } else {
                parts.push(format!(" {} ", lbl));
            }
        }
        parts.push("->".to_string());

        let idx_field = 3usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!("{}{}", base_prefix, lang().label_stop_bits.as_str());
        let title_style = if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        lines.push(ratatui::text::Line::from(Span::styled(
            title_text,
            title_style,
        )));
        // Editing selector — don't render '>' marker here.
        let val_text = format!("{}{}", base_prefix, parts.join(" "));
        lines.push(ratatui::text::Line::from(Span::styled(
            val_text,
            Style::default().fg(Color::Yellow),
        )));
    } else {
        push_field(
            &mut lines,
            3,
            lang().label_stop_bits.as_str(),
            form.stop_bits.to_string(),
            matches!(editing_field, Some(EditingField::StopBits)),
            form.input_buffer.as_str(),
        );
    }

    crate::tui::ui::components::render_boxed_paragraph(f, area, lines, style);
}
