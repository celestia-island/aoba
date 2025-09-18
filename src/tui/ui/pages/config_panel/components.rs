use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};

use crate::{i18n::lang, protocol::status::types, tui::ui::components::styled_label::styled_spans};

/// Derive selection index for config panel from current page state
pub fn derive_selection(app: &types::Status) -> usize {
    // For config panel, we need to determine which field is currently selected
    match &app.page {
        types::Page::ModbusConfig { edit_cursor, .. } => {
            // edit_cursor tracks both navigation and editing state
            *edit_cursor
        }
        _ => 0usize,
    }
}

/// Generate lines for a two-column key/value list for config panel settings.
/// Returns a vector of rendered lines without handling outer frame rendering.
///
/// The structure follows the requirements:
/// - Group 1: "Enable Port" toggle + "Communication Protocol" selector  
/// - Group 2: Serial port basic parameters (baud rate, parity, etc.)
///
/// The right-side values use StyledSpanKind with appropriate states for highlighting/editing.
pub fn render_kv_lines() -> Result<Vec<Line<'static>>> {
    // Use read_status to access current Status snapshot
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

        // Determine current row selection for styling
        let current_selection = derive_selection(app);

        // GROUP 1: Port control and protocol selection
        render_group1_lines(&mut lines, port_data, current_selection)?;

        // Empty line between groups
        lines.push(Line::from(vec![Span::raw("")]));

        // GROUP 2: Serial parameters
        render_group2_lines(&mut lines, port_data, current_selection)?;

        Ok(lines)
    })
}

/// Render Group 1: Enable Port toggle + Communication Protocol selector
fn render_group1_lines(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    current_selection: usize,
) -> Result<()> {
    // 1. Enable Port toggle (row 0)
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

    // Create toggle-style spans for the enable value
    let enable_state = if current_selection == 0 {
        crate::tui::ui::components::styled_label::TextState::Selected
    } else {
        crate::tui::ui::components::styled_label::TextState::Normal
    };

    lines.push(create_kv_line(
        &enable_label,
        &enable_value,
        enable_state,
        false,
    )?);

    // 2. Communication Protocol selector (row 1)
    let protocol_label = lang().protocol.common.protocol_selection.clone();
    let protocol_value = lang().protocol.common.mode_modbus.clone(); // Default to Modbus for now

    let protocol_state = if current_selection == 1 {
        crate::tui::ui::components::styled_label::TextState::Selected
    } else {
        crate::tui::ui::components::styled_label::TextState::Normal
    };

    lines.push(create_kv_line(
        &protocol_label,
        &protocol_value,
        protocol_state,
        true,
    )?);

    Ok(())
}

/// Render Group 2: Serial port basic parameters
fn render_group2_lines(
    lines: &mut Vec<Line<'static>>,
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    current_selection: usize,
) -> Result<()> {
    // Define the serial parameter labels and extract values
    let serial_labels = vec![
        lang().protocol.common.label_baud.clone(),      // row 3
        lang().protocol.common.label_data_bits.clone(), // row 4
        lang().protocol.common.label_parity.clone(),    // row 5
        lang().protocol.common.label_stop_bits.clone(), // row 6
    ];

    for (idx, label) in serial_labels.iter().enumerate() {
        let row_idx = idx + 3; // Starting from row 3 (after group 1 + empty line)
        let value = get_serial_param_value(port_data, idx);

        let state = if current_selection == row_idx {
            crate::tui::ui::components::styled_label::TextState::Selected
        } else {
            crate::tui::ui::components::styled_label::TextState::Normal
        };

        lines.push(create_kv_line(label, &value, state, false)?);
    }

    Ok(())
}

/// Create a key-value line with proper styling
fn create_kv_line(
    label: &str,
    value: &str,
    state: crate::tui::ui::components::styled_label::TextState,
    is_selector: bool,
) -> Result<Line<'static>> {
    // Create bold label span
    let label_span = Span::styled(
        format!("{}  ", label),
        Style::default().add_modifier(Modifier::BOLD),
    );

    // Create value spans using styled_spans
    let value_spans = if is_selector {
        // Use selector-style spans for protocol selection
        styled_spans(
            crate::tui::ui::components::styled_label::StyledSpanKind::Selector {
                base_prefix: "",
                label: value,
                state,
            },
        )
    } else {
        // Use simple text spans for other values
        styled_spans(
            crate::tui::ui::components::styled_label::StyledSpanKind::Text {
                text: value,
                state,
                bold: false,
            },
        )
    };

    // Build the line with label and value
    let mut line_spans = vec![label_span];
    line_spans.extend(value_spans);

    Ok(Line::from(line_spans))
}

/// Get serial parameter value by index
fn get_serial_param_value(
    port_data: Option<&crate::protocol::status::types::port::PortData>,
    param_idx: usize,
) -> String {
    if let Some(pd) = port_data {
        if let Some(rt) = &pd.runtime {
            match param_idx {
                0 => rt.current_cfg.baud.to_string(),
                1 => rt.current_cfg.data_bits.to_string(),
                2 => format!("{:?}", rt.current_cfg.parity),
                3 => rt.current_cfg.stop_bits.to_string(),
                _ => "??".to_string(),
            }
        } else {
            match param_idx {
                0 => "9600".to_string(),
                1 => "8".to_string(),
                2 => lang().protocol.common.parity_none.clone(),
                3 => "1".to_string(),
                _ => "??".to_string(),
            }
        }
    } else {
        "??".to_string()
    }
}
