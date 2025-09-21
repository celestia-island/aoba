use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::components::styled_label::{styled_spans, StyledSpanKind, TextState},
};

// Constants to avoid magic numbers/strings in layout calculation
const TARGET_LABEL_WIDTH: usize = 20; // target label column width for alignment
const LABEL_PADDING_EXTRA: usize = 2; // extra spacing added after label when padding
const INDICATOR_SELECTED: &str = "> ";
const INDICATOR_UNSELECTED: &str = "  ";

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    // For config panel, we need to determine which field is currently selected
    match read_status(|s| Ok(s.page.clone()))? {
        types::Page::ConfigPanel { cursor, .. } => {
            // cursor tracks both navigation and editing state
            Ok(cursor)
        }
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
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
pub fn render_kv_lines_with_indicators(sel_idx: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Get current port data
    let port_data =
        if let Some(port_name) = read_status(|s| Ok(s.ports.order.get(sel_idx).cloned()))? {
            read_status(|s| Ok(s.ports.map.get(&port_name).cloned()))?
        } else {
            None
        };

    // Determine current selection for styling
    let current_selection = derive_selection()?;

    // Determine whether the port is occupied by this instance. Only in that case
    // we display the full set of controls (group2, group3 and protocol config
    // navigation inside group1).
    let occupied_by_this = is_port_occupied_by_this(port_data.as_ref());

    // GROUP 1: Port control and protocol (protocol config item may be hidden)
    render_group1_with_indicators(
        &mut lines,
        port_data.as_ref(),
        current_selection,
        occupied_by_this,
    )?;

    if occupied_by_this {
        // Empty line between groups
        lines.push(Line::from(vec![Span::raw("")]));

        // GROUP 2: Serial parameters
        render_group2_with_indicators(&mut lines, port_data.as_ref(), current_selection)?;

        // Empty line between groups
        lines.push(Line::from(vec![Span::raw("")]));

        // GROUP 3: Communication log
        render_group3_with_indicators(&mut lines, current_selection)?;
    }

    Ok(lines)
}

/// Render Group 1: Enable Port toggle + Protocol Mode selector + Protocol Config navigation
fn render_group1_with_indicators(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&types::port::PortData>,
    current_selection: types::cursor::ConfigPanelCursor,
    occupied_by_this: bool,
) -> Result<()> {
    // 1. Enable Port toggle
    let enable_label = lang().protocol.common.enable_port.clone();
    let enable_value = if let Some(pd) = port_data {
        match pd.state {
            types::port::PortState::OccupiedByThis { .. } => {
                lang().protocol.common.port_enabled.clone()
            }
            _ => lang().protocol.common.port_disabled.clone(),
        }
    } else {
        lang().protocol.common.port_disabled.clone()
    };

    let enable_selected = current_selection == types::cursor::ConfigPanelCursor::EnablePort;
    // Use a Selector-like presentation for the enable switch so it can show as toggled
    // visually. We'll use the label string (enabled/disabled) as the selector label.
    lines.push(create_line(
        &enable_label,
        StyledSpanKind::Selector {
            base_prefix: "",
            label: &enable_value,
            state: if enable_selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
        },
        enable_selected,
    )?);

    // 2. Protocol Mode selector (split from the old combined field)
    let mode_label = lang().protocol.common.protocol_mode.clone();
    // Protocol mode selector: reflect the current enum variant in PortConfig if available.
    let mode_selected = current_selection == types::cursor::ConfigPanelCursor::ProtocolMode;

    // Determine label from port_data.config if present
    let mode_value = if let Some(pd) = port_data {
        match &pd.config {
            types::port::PortConfig::Modbus { .. } => lang().protocol.common.mode_modbus.clone(),
        }
    } else {
        lang().protocol.common.mode_modbus.clone()
    };

    lines.push(create_line(
        &mode_label,
        StyledSpanKind::Selector {
            base_prefix: "",
            label: &mode_value,
            state: if mode_selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
        },
        mode_selected,
    )?);

    // 3. Protocol Config navigation
    // Only show the protocol config navigation if the port is occupied by this
    // instance. When not occupied we hide this navigation item as requested.
    if occupied_by_this {
        let config_label = lang().protocol.common.business_config.clone();
        let config_value = lang().protocol.common.enter_modbus_config.clone(); // Default to Modbus for now

        let config_selected = current_selection == types::cursor::ConfigPanelCursor::ProtocolConfig;
        // This is a navigable text entry (press Enter to go to Modbus panel)
        lines.push(create_line(
            &config_label,
            StyledSpanKind::Text {
                text: &config_value,
                state: if config_selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                },
                bold: false,
            },
            config_selected,
        )?);
    }

    Ok(())
}

/// Helper: true if port_data indicates PortState::OccupiedByThis
fn is_port_occupied_by_this(port_data: Option<&types::port::PortData>) -> bool {
    if let Some(pd) = port_data {
        matches!(pd.state, types::port::PortState::OccupiedByThis { .. })
    } else {
        false
    }
}

/// Render Group 2: Serial port basic parameters
fn render_group2_with_indicators(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&types::port::PortData>,
    current_selection: types::cursor::ConfigPanelCursor,
) -> Result<()> {
    let serial_fields = [
        (
            types::cursor::ConfigPanelCursor::BaudRate,
            lang().protocol.common.label_baud.clone(),
        ),
        (
            types::cursor::ConfigPanelCursor::DataBits,
            lang().protocol.common.label_data_bits.clone(),
        ),
        (
            types::cursor::ConfigPanelCursor::Parity,
            lang().protocol.common.label_parity.clone(),
        ),
        (
            types::cursor::ConfigPanelCursor::StopBits,
            lang().protocol.common.label_stop_bits.clone(),
        ),
    ];

    for (cursor_type, label) in serial_fields.iter() {
        let value = get_serial_param_value_by_cursor(port_data, *cursor_type);
        let selected = current_selection == *cursor_type;

        // Use Selector presentation for serial parameter so cycling via left/right
        // shows as a selectable list-like widget.
        lines.push(create_line(
            label,
            StyledSpanKind::Selector {
                base_prefix: "",
                label: &value,
                state: if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                },
            },
            selected,
        )?);
    }

    Ok(())
}

/// Render Group 3: Communication log access (hyperlink-style)
fn render_group3_with_indicators(
    lines: &mut Vec<Line<'static>>,
    current_selection: types::cursor::ConfigPanelCursor,
) -> Result<()> {
    // 1. View Communication Log navigation
    let log_label = lang().protocol.common.log_monitoring.clone();
    let log_value = lang().protocol.common.view_communication_log.clone();
    let log_selected = current_selection == types::cursor::ConfigPanelCursor::ViewCommunicationLog;
    lines.push(create_line(
        &log_label,
        StyledSpanKind::Text {
            text: &log_value,
            state: if log_selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        },
        log_selected,
    )?);

    Ok(())
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: &str,
    // Accept a StyledSpanKind so caller provides how the value is rendered.
    value_kind: crate::tui::ui::components::styled_label::StyledSpanKind<'_>,
    selected: bool,
) -> Result<Line<'static>> {
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
        let padding_needed = if label_width < TARGET_LABEL_WIDTH {
            TARGET_LABEL_WIDTH - label_width + LABEL_PADDING_EXTRA
        } else {
            LABEL_PADDING_EXTRA // Minimum spacing
        };

        // Add spacing
        line_spans.push(Span::raw(" ".repeat(padding_needed)));
    }

    // Add focus indicator
    let indicator_span = if selected {
        Span::styled(
            INDICATOR_SELECTED.to_string(),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::raw(INDICATOR_UNSELECTED.to_string())
    };
    line_spans.push(indicator_span);

    // If caller provided a Text/Selector kind that references TextState, we may want to
    // override the state according to whether the whole line is selected. To keep
    // caller control while ensuring consistent selection behavior, map certain
    // variants to use the computed TextState when appropriate.
    use crate::tui::ui::components::styled_label::{StyledSpanKind, TextState};

    let value_spans = match value_kind {
        StyledSpanKind::Selector {
            base_prefix,
            label,
            state: _,
        } => styled_spans(StyledSpanKind::Selector {
            base_prefix,
            label,
            state: if selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
        }),
        StyledSpanKind::Text {
            text,
            state: _,
            bold,
        } => styled_spans(StyledSpanKind::Text {
            text,
            state: if selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold,
        }),
        // For other kinds pass through but attempt to normalize selection state where there
        // is a state field (Input and PrefixIndex don't carry TextState so pass-through).
        other => styled_spans(other),
    };

    // Add value spans to the line
    line_spans.extend(value_spans);

    Ok(Line::from(line_spans))
}

/// Get serial parameter value by cursor type
fn get_serial_param_value_by_cursor(
    port_data: Option<&types::port::PortData>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> String {
    if let Some(pd) = port_data {
        if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &pd.state {
            match cursor_type {
                types::cursor::ConfigPanelCursor::BaudRate => {
                    return runtime.current_cfg.baud.to_string()
                }
                types::cursor::ConfigPanelCursor::DataBits => {
                    return runtime.current_cfg.data_bits.to_string()
                }
                types::cursor::ConfigPanelCursor::Parity => {
                    return format!("{:?}", runtime.current_cfg.parity)
                }
                types::cursor::ConfigPanelCursor::StopBits => {
                    return runtime.current_cfg.stop_bits.to_string()
                }
                _ => return "??".to_string(),
            }
        }
    }

    "??".to_string()
}
