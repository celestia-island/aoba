use anyhow::Result;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::{i18n::lang, protocol::status::types};

/// Render a two-column key/value list for common serial settings and a few modbus fields.
pub fn render_kv_lines(frame: &mut Frame, area: Rect) -> Result<Vec<Line<'static>>> {
    // Use read_status to access current Status snapshot and build list of kv pairs
    crate::protocol::status::read_status(|app| {
        // Build list of kv pairs: left label from i18n, right value from Status (first port if exists)
        let mut items: Vec<ListItem> = Vec::new();
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
            }
        }

        for (l, v) in labels.into_iter().zip(values.into_iter()) {
            let txt = format!("{l:20} {v}");
            items.push(ListItem::new(txt));
        }

        use ratatui::style::{Modifier, Style};

        // Determine selected row to highlight: use global selection derived from page
        // so the panel reflects the app's current selection (entry list or subpage selected_port).
        let mut selected_row: usize = 0usize;
        if let Some(sel) = Some(derive_global_selection(app)) {
            selected_row = sel;
        }
        if items.is_empty() {
            selected_row = 0usize;
        } else if selected_row >= items.len() {
            selected_row = items.len().saturating_sub(1);
        }

        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(selected_row));
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(lang().protocol.modbus.label_modbus_settings.clone()),
            )
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(list, area, &mut state);

        Ok(())
    })?;

    Ok(vec![])
}
