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
    protocol::status::{EditingField, Page, Status},
    tui::ui::components::{
        render_boxed_paragraph, styled_spans, styled_title_span, StyledSpanKind, TextState,
    },
};

/// Render a configuration panel for a subpage. Reads `app.subpage_form` and renders fields.
pub fn render_config_panel(f: &mut Frame, area: Rect, app: &mut Status, style: Option<Style>) {
    // Use transient form if present (access via accessor to allow ephemeral migration)
    let form = match app.page {
        Page::ModbusConfig { selected_port: _ } => {
            if let Some(ref form) = app.page {
                form
            } else {
                // No form, render placeholder
                let lines = vec![ratatui::text::Line::from(
                    lang().protocol.modbus.no_form_loaded.as_str(),
                )];
                return render_boxed_paragraph(f, area, lines, style);
            }
        }
        _ => {
            // Not in config page, render placeholder
            let lines = vec![ratatui::text::Line::from(
                lang().protocol.modbus.no_form_loaded.as_str(),
            )];
            return render_boxed_paragraph(f, area, lines, style);
        }
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
        app.page.input_buffer.as_str(),
    );

    // Master passive toggle (idx 1) - when true the simulated master will not
    // proactively send requests on the wire and will only respond to incoming requests.
    let master_passive_label = lang().protocol.modbus.label_master_passive.as_str();
    // Determine display: if user hasn't set the option (None), show derived default
    // Policy: if any Master entries exist -> default = Passive (listen-only).
    let derived_default_passive = form
        .registers
        .iter()
        .any(|r| r.role == crate::protocol::status::EntryRole::Master);
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
        app.page.input_buffer.as_str(),
    );

    // Baud (idx 2)
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

        // Render title + selector / edit line
        let idx_field = 2usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_baud.as_str()
        );

        // If custom is selected and we're in the deeper confirmed edit stage, show editable buffer using input-style wrapper > [ ... ] <
        if current_idx == custom_idx && form.edit_confirmed {
            let buf = if form.input_buffer.is_empty() {
                "_".to_string()
            } else {
                form.input_buffer.clone()
            };
            lines.push(ratatui::text::Line::from(styled_spans(
                StyledSpanKind::Input {
                    base_prefix,
                    buffer: buf.as_str(),
                    hovered: selected,
                    editing: true,
                    with_prefix: true,
                },
            )));
        } else {
            // For cycling selector show < [current] > style; if custom but not confirmed, show [Custom]
            let cur_label = options
                .get(current_idx)
                .cloned()
                .unwrap_or_else(|| lang().protocol.common.custom.clone());
            // Render title and selector on the same line so the selector does not wrap to a new line.
            // Compute filler same as other in-line fields (36 char left column).
            let left_width = 36usize;
            let label_w = UnicodeWidthStr::width(title_text.as_str());
            let pad_count = if label_w >= left_width {
                2usize
            } else {
                left_width - label_w
            };
            let filler = " ".repeat(pad_count);

            let mut spans = Vec::new();
            // Mark title as editing when this field is the active editing field so styles align with other inputs.
            spans.push(styled_title_span(title_text.as_str(), selected, true));
            spans.push(Span::raw(filler));
            // Use Editing state for the selector when selected+editing, otherwise Normal/Selected mapping
            let sel_state = if selected {
                TextState::Editing
            } else {
                TextState::Normal
            };
            spans.extend(styled_spans(StyledSpanKind::Selector {
                base_prefix: "",
                label: cur_label.as_str(),
                state: sel_state,
            }));
            lines.push(ratatui::text::Line::from(spans));
        }
    } else {
        push_field(
            &mut lines,
            2,
            lang().protocol.common.label_baud.as_str(),
            form.baud.to_string(),
            matches!(editing_field, Some(EditingField::Baud)),
            form.input_buffer.as_str(),
        );
    }

    // Parity (idx 3)
    let parity_text = match form.parity {
        Parity::None => lang().protocol.common.parity_none.clone(),
        Parity::Even => lang().protocol.common.parity_even.clone(),
        Parity::Odd => lang().protocol.common.parity_odd.clone(),
    };
    if matches!(editing_field, Some(EditingField::Parity)) {
        let options = [
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
                parts.push(format!("[{opt}]"));
            } else {
                parts.push(format!(" {opt} "));
            }
        }
        parts.push("->".to_string());

        let idx_field = 3usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_parity.as_str()
        );
        let cur_label = options.get(cur_idx).cloned().unwrap_or_default();

        // Render title and selector on the same line while editing so the selector stays in-place.
        let left_width = 36usize;
        let label_w = UnicodeWidthStr::width(title_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };
        let filler = " ".repeat(pad_count);
        let mut spans = Vec::new();
        spans.push(styled_title_span(title_text.as_str(), selected, true));
        spans.push(Span::raw(filler));
        let sel_state = if selected {
            TextState::Editing
        } else {
            TextState::Normal
        };
        spans.extend(styled_spans(StyledSpanKind::Selector {
            base_prefix: "",
            label: cur_label.as_str(),
            state: sel_state,
        }));
        lines.push(ratatui::text::Line::from(spans));
    } else {
        push_field(
            &mut lines,
            3,
            lang().protocol.common.label_parity.as_str(),
            parity_text,
            matches!(editing_field, Some(EditingField::Parity)),
            form.input_buffer.as_str(),
        );
    }

    // Data bits (idx 4)
    let data_bits_text = format!("{}", form.data_bits);
    if matches!(editing_field, Some(EditingField::DataBits)) {
        let options = [5u8, 6u8, 7u8, 8u8];
        let cur_idx = options
            .iter()
            .position(|&d| d == form.data_bits)
            .unwrap_or(3);
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, opt) in options.iter().enumerate() {
            if i == cur_idx {
                parts.push(format!("[{opt}]"));
            } else {
                parts.push(format!(" {opt} "));
            }
        }
        parts.push("->".to_string());

        let idx_field = 4usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_data_bits.as_str()
        );
        let cur_label = options
            .get(cur_idx)
            .map(|d| d.to_string())
            .unwrap_or_else(|| "8".to_string());

        let left_width = 36usize;
        let label_w = UnicodeWidthStr::width(title_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };
        let filler = " ".repeat(pad_count);
        let mut spans = Vec::new();
        spans.push(styled_title_span(title_text.as_str(), selected, true));
        spans.push(Span::raw(filler));
        let sel_state = if selected {
            TextState::Editing
        } else {
            TextState::Normal
        };
        spans.extend(styled_spans(StyledSpanKind::Selector {
            base_prefix: "",
            label: cur_label.as_str(),
            state: sel_state,
        }));
        lines.push(ratatui::text::Line::from(spans));
    } else {
        push_field(
            &mut lines,
            4,
            lang().protocol.common.label_data_bits.as_str(),
            data_bits_text,
            matches!(editing_field, Some(EditingField::DataBits)),
            form.input_buffer.as_str(),
        );
    }

    // Stop bits (idx 5)
    if matches!(editing_field, Some(EditingField::StopBits)) {
        let opts_vals = [1, 2];
        let cur_idx = opts_vals
            .iter()
            .position(|&s| s == form.stop_bits)
            .unwrap_or(0);
        let opts_labels = ["1", "2"];
        let mut parts: Vec<String> = Vec::new();
        parts.push("<-".to_string());
        for (i, lbl) in opts_labels.iter().enumerate() {
            if i == cur_idx {
                parts.push(format!("[{lbl}]"));
            } else {
                parts.push(format!(" {lbl} "));
            }
        }
        parts.push("->".to_string());

        let idx_field = 5usize;
        let selected = idx_field == form.cursor;
        let base_prefix = "  ";
        let title_text = format!(
            "{}{}",
            base_prefix,
            lang().protocol.common.label_stop_bits.as_str()
        );
        // title style computed inline via helper; removed unused binding
        let cur_label = opts_labels.get(cur_idx).cloned().unwrap_or("1").to_string();

        let left_width = 36usize;
        let label_w = UnicodeWidthStr::width(title_text.as_str());
        let pad_count = if label_w >= left_width {
            2usize
        } else {
            left_width - label_w
        };
        let filler = " ".repeat(pad_count);
        let mut spans = Vec::new();
        spans.push(styled_title_span(title_text.as_str(), selected, true));
        spans.push(Span::raw(filler));
        let sel_state = if selected {
            TextState::Editing
        } else {
            TextState::Normal
        };
        spans.extend(styled_spans(StyledSpanKind::Selector {
            base_prefix: "",
            label: cur_label.as_str(),
            state: sel_state,
        }));
        lines.push(ratatui::text::Line::from(spans));
    } else {
        push_field(
            &mut lines,
            5,
            lang().protocol.common.label_stop_bits.as_str(),
            form.stop_bits.to_string(),
            matches!(editing_field, Some(EditingField::StopBits)),
            form.input_buffer.as_str(),
        );
    }

    // Global interval (idx 6) — render value in right column like other fields. When editing,
    // Show input-style spans but keep them on the same line as the title.
    {
        let idx_field = 6usize;
        let selected = idx_field == form.cursor;
        if matches!(editing_field, Some(EditingField::GlobalInterval)) {
            // Build title + filler then append input spans (skip the input's base_prefix)
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
            spans.push(styled_title_span(label_text.as_str(), selected, true));
            spans.push(Span::raw(filler));

            let buf = if form.input_buffer.is_empty() {
                "_".to_string()
            } else {
                form.input_buffer.clone()
            };
            let input_spans = styled_spans(StyledSpanKind::Input {
                base_prefix,
                buffer: buf.as_str(),
                hovered: selected,
                editing: true,
                with_prefix: false,
            });
            spans.extend(input_spans);
            lines.push(ratatui::text::Line::from(spans));
        } else {
            push_field(
                &mut lines,
                6,
                lang().protocol.modbus.global_interval.as_str(),
                format!("{} ms", form.global_interval_ms),
                false,
                form.input_buffer.as_str(),
            );
        }
    }

    // Global timeout (idx 7) — same behavior as interval above
    {
        let idx_field = 7usize;
        let selected = idx_field == form.cursor;
        if matches!(editing_field, Some(EditingField::GlobalTimeout)) {
            let base_prefix = "  ";
            let label = lang().protocol.modbus.global_timeout.as_str();
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
            spans.push(styled_title_span(label_text.as_str(), selected, true));
            spans.push(Span::raw(filler));

            let buf = if form.input_buffer.is_empty() {
                "_".to_string()
            } else {
                form.input_buffer.clone()
            };
            let input_spans = styled_spans(StyledSpanKind::Input {
                base_prefix,
                buffer: buf.as_str(),
                hovered: selected,
                editing: true,
                with_prefix: false,
            });
            spans.extend(input_spans);
            lines.push(ratatui::text::Line::from(spans));
        } else {
            push_field(
                &mut lines,
                7,
                lang().protocol.modbus.global_timeout.as_str(),
                format!("{} ms", form.global_timeout_ms),
                false,
                form.input_buffer.as_str(),
            );
        }
    }

    render_boxed_paragraph(f, area, lines, style);
}

/// Page-level map_key placeholder to allow pages::mod to dispatch key mapping.
pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

use crate::tui::utils::bus::Bus;
use crossterm::event::KeyEvent;

pub fn handle_subpage_key(_key: KeyEvent, _app: &mut Status, _bus: &Bus) -> bool {
    // Config panel currently has no special per-subpage key handling; do not consume.
    false
}

// Provide bottom hints for this page (used by pages::mod routing)
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    // reuse existing helper semantics from previous components
    let mut hints: Vec<String> = Vec::new();
    if let Some(form) = app.page.subpage_form.as_ref() {
        if form.cursor == 0 {
            if form.loop_enabled {
                hints.push(
                    lang()
                        .protocol
                        .modbus
                        .hint_enter_pause_work
                        .as_str()
                        .to_string(),
                );
            } else {
                hints.push(
                    lang()
                        .protocol
                        .modbus
                        .hint_enter_start_work
                        .as_str()
                        .to_string(),
                );
            }
            hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
            return hints;
        }
    }
    hints.push(lang().hotkeys.press_enter_confirm_edit.as_str().to_string());
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    hints
}
