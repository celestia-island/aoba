use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};

use crate::{
    i18n::lang,
    protocol::status::{types, write_status},
    tui::ui::components::styled_label::styled_spans,
};

/// Derive selection index for config panel from current page state
pub fn derive_selection(app: &types::Status) -> types::ui::ConfigPanelCursor {
    // For config panel, we need to determine which field is currently selected
    match &app.page {
        types::Page::ModbusConfig { cursor, .. } => {
            // cursor tracks both navigation and editing state
            *cursor
        }
        _ => types::ui::ConfigPanelCursor::EnablePort,
    }
}

/// Generate lines for config panel with 4:1:5 layout (label:indicator:value).
/// Returns lines that can be used with render_boxed_paragraph.
///
/// The structure follows the requirements:
/// - Group 1: "Enable Port" toggle + "Protocol Mode" selector + "Protocol Config" navigation
/// - Group 2: Serial port basic parameters (baud rate, parity, etc.)
///
/// Each line has the format: [Label____] [>] [Value_____] with proper spacing.
pub fn render_kv_lines_with_indicators() -> Result<Vec<Line<'static>>> {
    crate::protocol::status::read_status(|app| {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Get the currently selected port
        let sel_idx = match &app.page {
            types::Page::ModbusDashboard { selected_port, .. }
            | types::Page::ModbusConfig { selected_port, .. }
            | types::Page::ModbusLog { selected_port, .. } => *selected_port,
            types::Page::Entry {
                cursor: Some(types::ui::EntryCursor::Com { idx }),
                ..
            } => *idx,
            _ => 0usize,
        };

        // Get current port data
        let port_data = if let Some(port_name) = app.ports.order.get(sel_idx) {
            app.ports.map.get(port_name)
        } else {
            None
        };

        // Determine current selection for styling
        let current_selection = derive_selection(app);

        // GROUP 1: Port control and protocol
        render_group1_with_indicators(&mut lines, port_data, current_selection)?;

        // Empty line between groups
        lines.push(Line::from(vec![Span::raw("")]));

        // GROUP 2: Serial parameters
        render_group2_with_indicators(&mut lines, port_data, current_selection)?;

        // Empty line between groups
        lines.push(Line::from(vec![Span::raw("")]));

        // GROUP 3: Communication log
        render_group3_with_indicators(&mut lines, current_selection)?;

        Ok(lines)
    })
}

/// Render Group 1: Enable Port toggle + Protocol Mode selector + Protocol Config navigation
fn render_group1_with_indicators(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    current_selection: types::ui::ConfigPanelCursor,
) -> Result<()> {
    // 1. Enable Port toggle
    let enable_label = lang().protocol.common.enable_port.clone();
    let enable_value = if let Some(pd) = port_data {
        match pd.state {
            crate::protocol::status::types::port::PortState::OccupiedByThis => {
                lang().protocol.common.port_enabled.clone()
            }
            _ => lang().protocol.common.port_disabled.clone(),
        }
    } else {
        lang().protocol.common.port_disabled.clone()
    };

    let enable_selected = current_selection == types::ui::ConfigPanelCursor::EnablePort;
    lines.push(create_config_line(
        &enable_label,
        &enable_value,
        enable_selected,
        false,
    )?);

    // 2. Protocol Mode selector (split from the old combined field)
    let mode_label = lang().protocol.common.protocol_mode.clone();
    let mode_value = lang().protocol.common.mode_modbus.clone(); // Default to Modbus for now

    let mode_selected = current_selection == types::ui::ConfigPanelCursor::ProtocolMode;
    lines.push(create_config_line(
        &mode_label,
        &mode_value,
        mode_selected,
        true,
    )?);

    // 3. Protocol Config navigation 
    let config_label = lang().protocol.common.business_config.clone();
    let config_value = lang().protocol.common.enter_modbus_config.clone(); // Default to Modbus for now

    let config_selected = current_selection == types::ui::ConfigPanelCursor::ProtocolConfig;
    lines.push(create_config_line(
        &config_label,
        &config_value,
        config_selected,
        false,
    )?);

    Ok(())
}

/// Render Group 2: Serial port basic parameters
fn render_group2_with_indicators(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    current_selection: types::ui::ConfigPanelCursor,
) -> Result<()> {
    let serial_fields = [
        (
            types::ui::ConfigPanelCursor::BaudRate,
            lang().protocol.common.label_baud.clone(),
        ),
        (
            types::ui::ConfigPanelCursor::DataBits,
            lang().protocol.common.label_data_bits.clone(),
        ),
        (
            types::ui::ConfigPanelCursor::Parity,
            lang().protocol.common.label_parity.clone(),
        ),
        (
            types::ui::ConfigPanelCursor::StopBits,
            lang().protocol.common.label_stop_bits.clone(),
        ),
    ];

    for (cursor_type, label) in serial_fields.iter() {
        let value = get_serial_param_value_by_cursor(port_data, *cursor_type);
        let selected = current_selection == *cursor_type;

        lines.push(create_config_line(label, &value, selected, false)?);
    }

    Ok(())
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_config_line(
    label: &str,
    value: &str,
    selected: bool,
    is_selector: bool,
) -> Result<Line<'static>> {
    use unicode_width::UnicodeWidthStr;

    // Calculate the width of the label accurately accounting for Unicode
    let label_width = UnicodeWidthStr::width(label);

    // Create spans
    let mut line_spans = Vec::new();

    // Add label if not empty (for hyperlink-style entries, label will be empty)
    if !label.is_empty() {
        let label_span = Span::styled(
            label.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        );
        line_spans.push(label_span);

        // Calculate dynamic spacing to align values properly
        // Target alignment: labels should take ~40% of width, values start at ~45%
        let target_label_width = 20; // Target column width for alignment
        let padding_needed = if label_width < target_label_width {
            target_label_width - label_width + 2 // +2 for spacing
        } else {
            2 // Minimum spacing
        };

        // Add spacing
        line_spans.push(Span::raw(" ".repeat(padding_needed)));
    }

    // Add focus indicator
    let indicator_span = if selected {
        Span::styled("> ".to_string(), Style::default().fg(Color::Green))
    } else {
        Span::raw("  ".to_string()) // Two spaces for alignment
    };
    line_spans.push(indicator_span);

    // Create value spans using styled_spans
    let value_spans = if is_selector {
        styled_spans(
            crate::tui::ui::components::styled_label::StyledSpanKind::Selector {
                base_prefix: "",
                label: value,
                state: if selected {
                    crate::tui::ui::components::styled_label::TextState::Selected
                } else {
                    crate::tui::ui::components::styled_label::TextState::Normal
                },
            },
        )
    } else {
        styled_spans(
            crate::tui::ui::components::styled_label::StyledSpanKind::Text {
                text: value,
                state: if selected {
                    crate::tui::ui::components::styled_label::TextState::Selected
                } else {
                    crate::tui::ui::components::styled_label::TextState::Normal
                },
                bold: false,
            },
        )
    };

    // Add value spans
    line_spans.extend(value_spans);

    Ok(Line::from(line_spans))
}

/// Get serial parameter value by cursor type
fn get_serial_param_value_by_cursor(
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    cursor_type: types::ui::ConfigPanelCursor,
) -> String {
    if let Some(pd) = port_data {
        if let Some(rt) = &pd.runtime {
            match cursor_type {
                types::ui::ConfigPanelCursor::BaudRate => rt.current_cfg.baud.to_string(),
                types::ui::ConfigPanelCursor::DataBits => rt.current_cfg.data_bits.to_string(),
                types::ui::ConfigPanelCursor::Parity => format!("{:?}", rt.current_cfg.parity),
                types::ui::ConfigPanelCursor::StopBits => rt.current_cfg.stop_bits.to_string(),
                _ => "??".to_string(),
            }
        } else {
            match cursor_type {
                types::ui::ConfigPanelCursor::BaudRate => "9600".to_string(),
                types::ui::ConfigPanelCursor::DataBits => "8".to_string(),
                types::ui::ConfigPanelCursor::Parity => lang().protocol.common.parity_none.clone(),
                types::ui::ConfigPanelCursor::StopBits => "1".to_string(),
                _ => "??".to_string(),
            }
        }
    } else {
        "??".to_string()
    }
}

/// Render Group 3: Communication log access (hyperlink-style)
fn render_group3_with_indicators(
    lines: &mut Vec<Line<'static>>,
    current_selection: types::ui::ConfigPanelCursor,
) -> Result<()> {
    // 1. View Communication Log navigation
    let log_label = lang().protocol.common.log_monitoring.clone();
    let log_value = lang().protocol.common.view_communication_log.clone();
    let log_selected = current_selection == types::ui::ConfigPanelCursor::ViewCommunicationLog;
    lines.push(create_config_line(&log_label, &log_value, log_selected, false)?);

    Ok(())
}

/// Scroll the ConfigPanel view offset up by `amount` (saturating at 0).
pub fn config_panel_scroll_up(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::ModbusConfig { view_offset, .. } = &mut s.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ConfigPanel view offset down by `amount`.
pub fn config_panel_scroll_down(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::ModbusConfig { view_offset, .. } = &mut s.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}

/// Ensure the current cursor is visible by adjusting view_offset if needed
pub fn ensure_cursor_visible() -> Result<()> {
    use crate::protocol::status::read_status;
    read_status(|app| {
        if let types::Page::ModbusConfig { cursor, view_offset, .. } = &app.page {
            // Get total number of fields (8 fields total: EnablePort, ProtocolMode, ProtocolConfig, BaudRate, DataBits, Parity, StopBits, ViewCommunicationLog)
            let total_fields = 8;
            let cursor_index = cursor.to_index();
            
            // Assume visible area shows about 10 lines
            let visible_lines = 10;
            
            let should_scroll = if cursor_index < *view_offset {
                // Cursor is above visible area, scroll up
                Some(cursor_index)
            } else if cursor_index >= view_offset + visible_lines {
                // Cursor is below visible area, scroll down
                Some(cursor_index.saturating_sub(visible_lines - 1))
            } else {
                None
            };
            
            if let Some(new_offset) = should_scroll {
                let max_offset = total_fields.saturating_sub(visible_lines);
                let new_offset = new_offset.min(max_offset);
                
                // Update the view_offset
                write_status(|s| {
                    if let types::Page::ModbusConfig { view_offset, .. } = &mut s.page {
                        *view_offset = new_offset;
                    }
                    Ok(())
                })?;
            }
        }
        Ok(())
    })
}
