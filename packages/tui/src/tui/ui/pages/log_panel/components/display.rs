use anyhow::Result;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{i18n::lang, tui::status as types, tui::status::read_status};

/// Extract log data from current page state
pub fn extract_log_data() -> Result<Option<(Vec<types::port::PortLogEntry>, Option<usize>)>> {
    let res = read_status(|status| match &status.page {
        crate::tui::status::Page::LogPanel {
            selected_port,
            selected_item,
            ..
        } => {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    Ok(Some((port.logs.clone(), *selected_item)))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    })?;
    Ok(res)
}

/// Render the main log display area
pub fn render_log_display(
    frame: &mut Frame,
    area: Rect,
    logs: &[types::port::PortLogEntry],
    selected_item: Option<usize>,
) -> Result<()> {
    // Each log entry is rendered as a 3-line block
    let lines_per_item = 3usize;
    let content_height = area.height.saturating_sub(2) as usize; // Reserve space for borders
    let items_visible = std::cmp::max(1, content_height / lines_per_item);

    // Calculate which items to show based on selected_item
    let start_index = if let Some(selected_idx) = selected_item {
        // Manual mode: show selected item as first item
        selected_idx
    } else {
        // Auto-follow mode: show last items
        if logs.len() <= items_visible {
            0
        } else {
            logs.len() - items_visible
        }
    };

    let mut rendered_lines: Vec<Line> = Vec::new();

    for i in 0..items_visible {
        let idx = start_index.saturating_add(i);
        if let Some(entry) = logs.get(idx) {
            // Determine if this item is selected
            let selected = if let Some(sel_idx) = selected_item {
                sel_idx == idx
            } else {
                // Auto-follow mode: select the last item
                idx == logs.len().saturating_sub(1)
            };

            let mut lines = build_log_lines(entry);
            let prefix_span = if selected {
                Span::styled("> ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            };

            lines[0].spans.insert(0, prefix_span);

            rendered_lines.extend_from_slice(&lines);
        } else {
            // blank item filler to keep layout
            rendered_lines.push(Line::from(Span::raw("")));
            rendered_lines.push(Line::from(Span::raw("")));
            rendered_lines.push(Line::from(Span::raw("")));
        }
    }

    // Build the title with internationalized text
    let log_title = Span::styled(
        lang().tabs.tab_log.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    );

    // Fix: Use selected_item to determine follow status, not auto_scroll
    let follow_status = if selected_item.is_none() {
        Span::styled(
            format!(" ({})", lang().tabs.log.hint_follow_on.clone()),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::styled(
            format!(" ({})", lang().tabs.log.hint_follow_off.clone()),
            Style::default().fg(Color::Yellow),
        )
    };

    let title_line = Line::from(vec![
        Span::raw(" "),
        log_title,
        follow_status,
        Span::raw(" "),
    ]);

    // Create block with custom border and title
    let block = Block::default().borders(Borders::ALL).title(title_line);

    // Create inner area with 1 character left padding
    let inner = block.inner(area);
    let padded_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };

    // Render the block first
    frame.render_widget(block, area);

    // Then render the content with padding
    let paragraph = Paragraph::new(rendered_lines);
    frame.render_widget(paragraph, padded_area);

    // Add position counter at the bottom of the frame
    let current_pos = if let Some(sel_idx) = selected_item {
        sel_idx + 1
    } else {
        logs.len()
    };
    let total_items = logs.len();
    let position_text = format!(" {current_pos} / {total_items} ");

    // Render position counter at bottom-right of the frame
    let position_area = Rect {
        x: area.x + area.width.saturating_sub(position_text.len() as u16 + 2),
        y: area.y + area.height.saturating_sub(1),
        width: position_text.len() as u16 + 1,
        height: 1,
    };

    let position_paragraph =
        Paragraph::new(position_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(position_paragraph, position_area);

    Ok(())
}

/// Render the log input area
pub fn render_log_input(frame: &mut Frame, area: Rect) -> Result<()> {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", lang().input.input_label.clone()));

    let content = vec![Line::from(format!(
        "{} | {} | {}",
        lang().hotkeys.press_enter_toggle.clone(),
        lang().hotkeys.press_c_clear.clone(),
        lang().hotkeys.press_esc_cancel.clone()
    ))];

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);

    Ok(())
}

fn build_log_lines(entry: &types::port::PortLogEntry) -> [Line<'static>; 3] {
    let time_line = Line::from(vec![Span::raw(
        entry.when.format("%H:%M:%S%.3f").to_string(),
    )]);

    match &entry.metadata {
        Some(types::port::PortLogMetadata::Lifecycle(data)) => {
            build_lifecycle_lines(time_line, data)
        }
        Some(types::port::PortLogMetadata::Communication(data)) => {
            build_communication_lines(time_line, data)
        }
        Some(types::port::PortLogMetadata::Management(data)) => {
            build_management_lines(time_line, data)
        }
        None => build_legacy_lines(time_line, entry),
    }
}

fn build_lifecycle_lines(
    time_line: Line<'static>,
    lifecycle: &types::port::PortLifecycleLog,
) -> [Line<'static>; 3] {
    let lang = lang();
    let status_text = match lifecycle.phase {
        types::port::PortLifecyclePhase::Created => lang.tabs.log.lifecycle_started.clone(),
        types::port::PortLifecyclePhase::Shutdown => lang.tabs.log.lifecycle_shutdown.clone(),
        types::port::PortLifecyclePhase::Restarted => lang.tabs.log.lifecycle_restarted.clone(),
        types::port::PortLifecyclePhase::Failed => lang.tabs.log.lifecycle_failed.clone(),
    };

    let status_color = match lifecycle.phase {
        types::port::PortLifecyclePhase::Created => Color::Green,
        types::port::PortLifecyclePhase::Shutdown => Color::Green,
        types::port::PortLifecyclePhase::Restarted => Color::Yellow,
        types::port::PortLifecyclePhase::Failed => Color::Red,
    };

    let line_two = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            lang.tabs.log.lifecycle_label.clone(),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" | "),
        Span::styled(
            status_text,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let line_three = if let Some(note) = &lifecycle.note {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                lang.tabs.log.reason_label.clone(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(": "),
            Span::raw(note.clone()),
        ])
    } else {
        Line::from(vec![Span::raw("  ")])
    };

    [time_line, line_two, line_three]
}

fn build_communication_lines(
    time_line: Line<'static>,
    comm: &types::port::PortCommunicationLog,
) -> [Line<'static>; 3] {
    let lang = lang();
    match comm.role {
        types::modbus::StationMode::Master => build_master_comm_lines(time_line, comm, &lang),
        types::modbus::StationMode::Slave => build_slave_comm_lines(time_line, comm, &lang),
    }
}

fn build_master_comm_lines(
    time_line: Line<'static>,
    comm: &types::port::PortCommunicationLog,
    lang: &crate::i18n::Lang,
) -> [Line<'static>; 3] {
    let success = comm_is_success(comm);

    let mut line_two_spans = vec![
        Span::raw("  "),
        Span::styled(
            lang.tabs.log.comm_master_response.clone(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let config_value = comm
        .config_index
        .map(|index| format!("0x{index:04X} ({index})"))
        .unwrap_or_else(|| lang.tabs.log.comm_unknown.clone());

    line_two_spans.push(Span::raw(" #"));
    line_two_spans.push(Span::raw(config_value));
    line_two_spans.push(Span::raw(" | "));

    let (result_text, result_color) = if success {
        (lang.tabs.log.result_success.clone(), Color::Green)
    } else {
        (lang.tabs.log.result_failure.clone(), Color::Red)
    };

    line_two_spans.push(Span::styled(
        result_text,
        Style::default()
            .fg(result_color)
            .add_modifier(Modifier::BOLD),
    ));

    let line_two = Line::from(line_two_spans);

    let line_three = if success {
        build_comm_success_line(lang, comm)
    } else {
        let reason_label = lang.tabs.log.reason_label.clone();
        let reason = comm_failure_reason(comm, lang);
        Line::from(vec![
            Span::raw("  "),
            Span::raw(format!("{reason_label}: {reason}")),
        ])
    };

    [time_line, line_two, line_three]
}

fn build_slave_comm_lines(
    time_line: Line<'static>,
    comm: &types::port::PortCommunicationLog,
    lang: &crate::i18n::Lang,
) -> [Line<'static>; 3] {
    let success = comm_is_success(comm);

    let mut line_two_spans = vec![
        Span::raw("  "),
        Span::styled(
            lang.tabs.log.comm_slave_request.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let config_value = comm
        .config_index
        .map(|index| format!("0x{index:04X} ({index})"))
        .unwrap_or_else(|| lang.tabs.log.comm_unknown.clone());

    line_two_spans.push(Span::raw(" #"));
    line_two_spans.push(Span::raw(config_value));
    line_two_spans.push(Span::raw(" | "));

    let (result_text, result_color) = if success {
        (lang.tabs.log.result_success.clone(), Color::Green)
    } else {
        (lang.tabs.log.result_failure.clone(), Color::Red)
    };

    line_two_spans.push(Span::styled(
        result_text,
        Style::default()
            .fg(result_color)
            .add_modifier(Modifier::BOLD),
    ));

    let line_two = Line::from(line_two_spans);

    let line_three = if success {
        build_comm_success_line(lang, comm)
    } else {
        let reason_label = lang.tabs.log.reason_label.clone();
        let reason = comm_failure_reason(comm, lang);
        Line::from(vec![
            Span::raw("  "),
            Span::raw(format!("{reason_label}: {reason}")),
        ])
    };

    [time_line, line_two, line_three]
}

fn comm_is_success(comm: &types::port::PortCommunicationLog) -> bool {
    match comm.success_hint {
        Some(value) => value,
        None => comm.parse_error.is_none(),
    }
}

fn comm_failure_reason(
    comm: &types::port::PortCommunicationLog,
    lang: &crate::i18n::Lang,
) -> String {
    comm.failure_reason
        .clone()
        .or_else(|| comm.parse_error.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| lang.tabs.log.reason_none.clone())
}

fn build_comm_success_line(
    lang: &crate::i18n::Lang,
    comm: &types::port::PortCommunicationLog,
) -> Line<'static> {
    let station_label = lang.tabs.log.comm_station_id_label.clone();
    let unknown = lang.tabs.log.comm_unknown.clone();
    let station_value = comm
        .station_id
        .map(|station| format!("0x{station:02X}"))
        .unwrap_or_else(|| unknown.clone());
    let station_segment = format!("{station_label} {station_value}");

    let register_label = lang.tabs.log.comm_register_type_label.clone();
    let register_segment = match comm.register_mode {
        Some(mode) => {
            let (code, name) = register_mode_descriptor(mode);
            format!("{register_label} {code:02} {name}")
        }
        None => format!("{register_label} {}", unknown.clone()),
    };

    let computed_end = comm.register_end.or_else(|| {
        if let (Some(start), Some(count)) = (comm.register_start, comm.register_quantity) {
            Some(start.saturating_add(count.saturating_sub(1)))
        } else {
            None
        }
    });

    let range_label = lang.tabs.log.comm_address_range_label.clone();
    let range_value = match (comm.register_start, computed_end) {
        (Some(start), Some(end)) => format!("0x{start:04X} - 0x{end:04X}"),
        _ => unknown.clone(),
    };
    let range_segment = format!("{range_label} {range_value}");

    let combined = format!(
        "{}; {}; {}",
        station_segment, register_segment, range_segment
    );

    Line::from(vec![Span::raw("  "), Span::raw(combined)])
}

fn register_mode_descriptor(mode: types::modbus::RegisterMode) -> (u8, String) {
    match mode {
        types::modbus::RegisterMode::Coils => (1, "Coils".to_string()),
        types::modbus::RegisterMode::DiscreteInputs => (2, "Discrete".to_string()),
        types::modbus::RegisterMode::Holding => (3, "Holding".to_string()),
        types::modbus::RegisterMode::Input => (4, "Input".to_string()),
    }
}

fn build_management_lines(
    time_line: Line<'static>,
    management: &types::port::PortManagementLog,
) -> [Line<'static>; 3] {
    use types::port::PortManagementEvent as Event;

    let lang = lang();
    let label_style = Style::default().fg(Color::DarkGray);
    let reason_line = |detail: String| -> Line<'static> {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(lang.tabs.log.reason_label.clone(), label_style),
            Span::raw(": "),
            Span::raw(detail),
        ])
    };

    match &management.event {
        Event::StationsUpdate {
            station_count,
            success,
            error,
        } => {
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.stations_count_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(format!("0x{station_count:04X} ({station_count})")),
                Span::raw(" | "),
            ];

            let status_text = if *success {
                lang.tabs.log.result_success.clone()
            } else {
                lang.tabs.log.result_failure.clone()
            };
            let status_color = if *success { Color::Green } else { Color::Red };
            spans.push(Span::styled(status_text, Style::default().fg(status_color)));

            let line_two = Line::from(spans);
            let line_three = if let Some(err) = error {
                reason_line(err.clone())
            } else {
                reason_line(lang.tabs.log.reason_none.clone())
            };

            [time_line, line_two, line_three]
        }
        Event::ConfigSync {
            mode,
            config_index,
            station_id,
            register_mode,
            address_start,
            address_end,
            success,
            error,
        } => {
            let (heading_text, heading_color) = match mode {
                types::modbus::StationMode::Master => {
                    (lang.tabs.log.comm_master_response.clone(), Color::Green)
                }
                types::modbus::StationMode::Slave => {
                    (lang.tabs.log.comm_slave_request.clone(), Color::Yellow)
                }
            };

            let mut line_two_spans = vec![
                Span::raw("  "),
                Span::styled(
                    heading_text,
                    Style::default()
                        .fg(heading_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" #"),
                Span::raw(format!("0x{config_index:04X} ({config_index})")),
                Span::raw(" | "),
            ];

            let result_text = if *success {
                lang.tabs.log.result_success.clone()
            } else {
                lang.tabs.log.result_failure.clone()
            };
            let result_color = if *success { Color::Green } else { Color::Red };

            line_two_spans.push(Span::styled(
                result_text,
                Style::default()
                    .fg(result_color)
                    .add_modifier(Modifier::BOLD),
            ));

            let line_two = Line::from(line_two_spans);

            let line_three = if *success {
                let station_segment =
                    format!("{} 0x{station_id:02X}", lang.tabs.log.comm_station_id_label);
                let (register_code, register_name) = register_mode_descriptor(*register_mode);
                let register_segment = format!(
                    "{} {register_code:02} {register_name}",
                    lang.tabs.log.comm_register_type_label
                );
                let range_segment = format!(
                    "{} 0x{address_start:04X} - 0x{address_end:04X}",
                    lang.tabs.log.comm_address_range_label
                );

                Line::from(vec![
                    Span::raw("  "),
                    Span::raw(format!(
                        "{}; {}; {}",
                        station_segment, register_segment, range_segment
                    )),
                ])
            } else {
                let detail = error
                    .clone()
                    .unwrap_or_else(|| lang.tabs.log.reason_none.clone());

                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(lang.tabs.log.reason_label.clone(), label_style),
                    Span::raw(": "),
                    Span::raw(detail),
                ])
            };

            [time_line, line_two, line_three]
        }
        Event::StateLockRequest { requester } => {
            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    lang.tabs.log.state_lock_requester_label.clone(),
                    label_style,
                ),
                Span::raw(": "),
                Span::raw(requester.clone()),
            ]);

            let line_three = reason_line(lang.tabs.log.reason_none.clone());

            [time_line, line_two, line_three]
        }
        Event::StateLockAck { locked } => {
            let status_text = if *locked {
                lang.tabs.log.state_lock_locked.clone()
            } else {
                lang.tabs.log.state_lock_unlocked.clone()
            };
            let status_color = if *locked { Color::Yellow } else { Color::Green };

            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.management_label.clone(), label_style),
                Span::raw(": "),
                Span::styled(status_text, Style::default().fg(status_color)),
            ]);

            let line_three = reason_line(lang.tabs.log.reason_none.clone());

            [time_line, line_two, line_three]
        }
        Event::Status { status, details } => {
            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.status_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(status.clone()),
            ]);

            let line_three = if let Some(detail) = details {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(lang.tabs.log.status_details_label.clone(), label_style),
                    Span::raw(": "),
                    Span::raw(detail.clone()),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(lang.tabs.log.status_details_label.clone(), label_style),
                    Span::raw(": "),
                    Span::raw(lang.tabs.log.reason_none.clone()),
                ])
            };

            [time_line, line_two, line_three]
        }
        Event::LogMessage { level, message } => {
            let level_upper = level.to_uppercase();
            let level_color = match level_upper.as_str() {
                "ERROR" => Color::Red,
                "WARN" => Color::Yellow,
                "INFO" => Color::Green,
                "DEBUG" => Color::Cyan,
                _ => Color::Magenta,
            };

            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.log_level_label.clone(), label_style),
                Span::raw(": "),
                Span::styled(level_upper, Style::default().fg(level_color)),
            ]);

            let line_three = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.log_message_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(message.clone()),
            ]);

            [time_line, line_two, line_three]
        }
        Event::RuntimeRestart {
            reason,
            connection_mode,
        } => {
            let mode_label = lang.tabs.log.runtime_restart_mode_label.clone();
            let mode_text = match connection_mode {
                types::modbus::StationMode::Master => lang.protocol.modbus.role_master.clone(),
                types::modbus::StationMode::Slave => lang.protocol.modbus.role_slave.clone(),
            };

            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(mode_label, label_style),
                Span::raw(": "),
                Span::raw(mode_text),
            ]);

            let line_three = reason_line(reason.clone());

            [time_line, line_two, line_three]
        }
        Event::SubprocessSpawned { mode, pid } => {
            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.subprocess_mode_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(mode.clone()),
            ]);

            let pid_text = pid
                .map(|value| format!("0x{value:04X} ({value})"))
                .unwrap_or_else(|| lang.tabs.log.comm_unknown.clone());

            let line_three = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.subprocess_pid_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(pid_text),
            ]);

            [time_line, line_two, line_three]
        }
        Event::SubprocessStopped { reason } => {
            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.management_label.clone(), label_style),
                Span::raw(": "),
                Span::styled(
                    lang.tabs.log.subprocess_stopped_summary.clone(),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            let line_three = if let Some(reason) = reason {
                reason_line(reason.clone())
            } else {
                reason_line(lang.tabs.log.reason_none.clone())
            };

            [time_line, line_two, line_three]
        }
        Event::SubprocessExited { success, detail } => {
            let line_two = Line::from(vec![
                Span::raw("  "),
                Span::styled(lang.tabs.log.subprocess_exit_label.clone(), label_style),
                Span::raw(": "),
                Span::raw(detail.clone()),
            ]);

            let line_three = if let Some(is_success) = success {
                let status_label = if *is_success {
                    lang.tabs.log.result_success.clone()
                } else {
                    lang.tabs.log.result_failure.clone()
                };
                let status_color = if *is_success {
                    Color::Green
                } else {
                    Color::Red
                };

                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(lang.tabs.log.result_label.clone(), label_style),
                    Span::raw(": "),
                    Span::styled(status_label, Style::default().fg(status_color)),
                ])
            } else {
                reason_line(lang.tabs.log.reason_none.clone())
            };

            [time_line, line_two, line_three]
        }
    }
}

fn build_legacy_lines(
    time_line: Line<'static>,
    entry: &types::port::PortLogEntry,
) -> [Line<'static>; 3] {
    let second = Line::from(vec![Span::raw("  "), Span::raw(entry.raw.clone())]);
    let third = Line::from(vec![
        Span::raw("  "),
        Span::raw(entry.parsed.clone().unwrap_or_else(|| String::new())),
    ]);

    [time_line, second, third]
}
