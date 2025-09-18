use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};

use crate::{
    i18n::lang,
    protocol::status::types,
    tui::ui::components::styled_label::{styled_spans, StyledSpanKind, TextState},
};

/// Derive selection index for config panel from current page state
pub fn derive_selection(app: &types::Status) -> usize {
    match &app.page {
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        types::Page::Entry {
            cursor: Some(types::ui::EntryCursor::Com { idx }),
            ..
        } => *idx,
        _ => 0usize,
    }
}

/// Generate lines for a two-column key/value list for common serial settings and a few modbus fields.
/// Returns a vector of rendered lines without handling outer frame rendering.
///
/// The right-side values use StyledSpanKind::Text which allows external control over:
/// - TextState::Normal - default appearance
/// - TextState::Selected - green highlighting (hover)
/// - TextState::Chosen - yellow highlighting  
/// - TextState::Editing - yellow + bold (active editing)
///
/// To control individual value states, modify the StyledSpanKind::Text parameters below.
pub fn render_kv_lines() -> Result<Vec<Line<'static>>> {
    // Use read_status to access current Status snapshot and build list of kv pairs
    crate::protocol::status::read_status(|app| {
        let labels = vec![
            lang().protocol.common.label_port.clone(),
            lang().protocol.common.label_baud.clone(),
            lang().protocol.common.label_data_bits.clone(),
            lang().protocol.common.label_parity.clone(),
            lang().protocol.common.label_stop_bits.clone(),
        ];

        // Extend labels with more port/modbus-related keys available in i18n
        let more_labels: Vec<String> = vec![
            lang().protocol.common.label_type.clone(),
            lang().protocol.common.label_guid.clone(),
            lang().protocol.common.label_usb.clone(),
            lang().protocol.common.label_serial.clone(),
            lang().protocol.common.label_manufacturer.clone(),
            lang().protocol.common.label_product.clone(),
            lang().protocol.modbus.global_interval.clone(),
            lang().protocol.modbus.global_timeout.clone(),
            lang().protocol.modbus.refresh_rate.clone(),
        ];
        // Merge
        let labels = [labels, more_labels].concat();

        // Derive values from selected port based on current page variant
        let mut values: Vec<String> = Vec::new();
        // Determine selected index from page
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

        if let Some(port_name) = app.ports.order.get(sel_idx) {
            if let Some(pd) = app.ports.map.get(port_name) {
                values.push(pd.port_name.clone());
                if let Some(rt) = &pd.runtime {
                    values.push(rt.current_cfg.baud.to_string());
                    values.push(rt.current_cfg.data_bits.to_string());
                    values.push(format!("{:?}", rt.current_cfg.parity));
                    values.push(rt.current_cfg.stop_bits.to_string());
                } else {
                    values.push("??".to_string());
                    values.push("??".to_string());
                    values.push("??".to_string());
                    values.push("??".to_string());
                }

                // Add additional values from PortData / PortExtra and Modbus defaults
                values.push(pd.port_type.clone());
                // GUID / USB fields: try to obtain from extra metadata
                values.push(pd.extra.guid.clone().unwrap_or_default());
                // USB label: show VID:PID if present
                let usb_label = match (pd.extra.vid, pd.extra.pid) {
                    (Some(vid), Some(pid)) => format!("{vid:04x}:{pid:04x}"),
                    _ => String::new(),
                };
                values.push(usb_label);
                values.push(pd.extra.serial.clone().unwrap_or_default());
                values.push(pd.extra.manufacturer.clone().unwrap_or_default());
                values.push(pd.extra.product.clone().unwrap_or_default());
                // Modbus-related transient config defaults (placeholders)
                values.push("??".to_string()); // global_interval placeholder ms
                values.push("??".to_string()); // global_timeout placeholder ms
                values.push("??".to_string()); // refresh_rate placeholder
            } else {
                for _ in 0..labels.len() {
                    values.push(String::new());
                }
            }
        } else {
            for _ in 0..labels.len() {
                values.push(String::new());
            }
        }

        // Build lines using StyledSpanKind for values
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Calculate max label width for alignment
        let max_label_width = labels.iter().map(|l| l.len()).max().unwrap_or(0);

        for (label, value) in labels.into_iter().zip(values.into_iter()) {
            // Create left-aligned label span (bold)
            let label_span = Span::styled(
                format!("{:width$}", label, width = max_label_width),
                Style::default().add_modifier(Modifier::BOLD),
            );

            // Create value span using StyledSpanKind::Text
            let value_spans = styled_spans(StyledSpanKind::Text {
                text: &value,
                state: TextState::Normal,
                bold: false,
            });

            // Build the line with label, spacing, and value
            let mut line_spans = vec![label_span, Span::raw("  ")];
            line_spans.extend(value_spans);

            lines.push(Line::from(line_spans));
        }

        Ok(lines)
    })
}
