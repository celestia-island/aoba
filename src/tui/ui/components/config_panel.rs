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
use serialport::Parity;
use unicode_width::UnicodeWidthStr;

/// Render a configuration panel for a subpage. Reads `app.subpage_form` and renders fields.
pub fn render_config_panel(f: &mut Frame, area: Rect, app: &mut Status, style: Option<Style>) {
    // Use transient form if present
    let form = app.subpage_form.as_ref().cloned().unwrap_or_default();

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
        let label_text = format!("{}{}", base_prefix, label);
        let left_width = 36usize; // label column width in character cells

        let right_text = if editing {
            lang().protocol.modbus.edit_suffix.replace("{}", buffer)
        } else if selected {
            format!("> {}", value)
        } else {
            value
        };

        // Measure displayed widths using unicode-width
        let label_w = UnicodeWidthStr::width(label_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };

        let filler = " ".repeat(pad_count);

        // Title style: always bold; selected title colored green, editing colored yellow
        let title_style = if selected && !editing {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if selected && editing {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let mut value_style = if editing {
            Style::default().fg(Color::Yellow)
        } else if selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };

        // Special-case first field (working toggle): show running=green, paused=yellow regardless of selection
        if idx == 0 {
            if form.loop_enabled {
                value_style = Style::default().fg(Color::LightGreen);
            } else {
                value_style = Style::default().fg(Color::Yellow);
            }
        }

        let spans = vec![
            Span::styled(label_text, title_style),
            Span::raw(filler),
            Span::styled(right_text, value_style),
        ];
        lines.push(ratatui::text::Line::from(spans));
    };

    let editing_field = form.editing_field.clone();

    // Loop enabled toggle (idx 0)
    let loop_label = lang().protocol.modbus.label_working.as_str();
    let loop_val = if form.loop_enabled {
        lang().protocol.modbus.status_running.clone()
    } else {
        lang().protocol.modbus.status_paused.clone()
    };
    // Display the working status as a non-editable toggle row; Enter will toggle directly in key handler.
    push_field(
        &mut lines,
        0,
        loop_label,
        loop_val,
        false,
        form.input_buffer.as_str(),
    );

    // Baud (idx 1)
    if matches!(editing_field, Some(EditingField::Baud)) {
        // Presets stop at 115200; append a 'Custom' slot so user can select and type a custom baud
        let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
        let mut options: Vec<String> = presets.iter().map(|p| p.to_string()).collect();
        options.push(lang().protocol.common.custom.clone());
        let custom_idx = options.len() - 1;

        // Determine current index in options from form.edit_choice_index when available
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

        // Render title + selector / edit line
        let idx_field = 1usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_baud.as_str()
        );
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
            // Confirmed editing: show buffer and yellow highlight
            let rendered = lang()
                .protocol
                .modbus
                .edit_suffix
                .replace("{}", form.input_buffer.as_str());
            // Editing confirmed: do not show '>' marker; show edit buffer only.
            let val_text = format!("{}{}", base_prefix, rendered);
            lines.push(ratatui::text::Line::from(Span::styled(
                val_text,
                Style::default().fg(Color::Yellow),
            )));
        } else if current_idx == custom_idx && !form.edit_confirmed {
            // Custom selected but not yet confirmed: show selector parts (no extra inline hint)
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
            1,
            lang().protocol.common.label_baud.as_str(),
            form.baud.to_string(),
            matches!(editing_field, Some(EditingField::Baud)),
            form.input_buffer.as_str(),
        );
    }

    // Parity (idx 2)
    let parity_text = match form.parity {
        Parity::None => lang().protocol.common.parity_none.clone(),
        Parity::Even => lang().protocol.common.parity_even.clone(),
        Parity::Odd => lang().protocol.common.parity_odd.clone(),
    };
    if matches!(editing_field, Some(EditingField::Parity)) {
        let options = vec![
            lang().protocol.common.parity_none.clone(),
            lang().protocol.common.parity_even.clone(),
            lang().protocol.common.parity_odd.clone(),
        ];
        let cur_idx = match form.parity {
            Parity::None => 0usize,
            Parity::Even => 1usize,
            Parity::Odd => 2usize,
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

        let idx_field = 2usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_parity.as_str()
        );
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
            2,
            lang().protocol.common.label_parity.as_str(),
            parity_text,
            matches!(editing_field, Some(EditingField::Parity)),
            form.input_buffer.as_str(),
        );
    }

    // Data bits (idx 3)
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

        let idx_field = 3usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_data_bits.as_str()
        );
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
            lang().protocol.common.label_data_bits.as_str(),
            data_bits_text,
            matches!(editing_field, Some(EditingField::DataBits)),
            form.input_buffer.as_str(),
        );
    }

    // Stop bits (idx 4)
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

        let idx_field = 4usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_stop_bits.as_str()
        );
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
            4,
            lang().protocol.common.label_stop_bits.as_str(),
            form.stop_bits.to_string(),
            matches!(editing_field, Some(EditingField::StopBits)),
            form.input_buffer.as_str(),
        );
    }

    crate::tui::ui::components::render_boxed_paragraph(f, area, lines, style);
}
