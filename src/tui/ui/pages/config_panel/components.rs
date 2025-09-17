use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::ui::components::render_boxed_paragraph,
};

/// Check if a subpage is active for configuration
pub fn is_subpage_active(app: &Status) -> bool {
    matches!(
        app.page,
        types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. }
    )
}

/// Render a two-column key/value list for common serial settings and a few modbus fields.
pub fn render_kv_panel(
    f: &mut Frame,
    area: Rect,
    app: &Status,
    _snap: &types::ui::ModbusConfigStatus,
) {
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
                values.push("9600".to_string());
                values.push("8".to_string());
                values.push(lang().protocol.common.parity_none.clone());
                values.push("1".to_string());
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
            values.push("1000".to_string()); // global_interval placeholder ms
            values.push("200".to_string()); // global_timeout placeholder ms
            values.push("1000".to_string()); // refresh_rate placeholder
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

    f.render_stateful_widget(list, area, &mut state);
}

/// Extract configuration snapshot from current page state
pub fn extract_config_snapshot(app: &Status) -> types::ui::ModbusConfigStatus {
    match &app.page {
        types::Page::ModbusConfig {
            selected_port,
            edit_active,
            edit_port,
            edit_field_index,
            edit_field_key,
            edit_buffer,
            edit_cursor_pos,
            ..
        } => types::ui::ModbusConfigStatus {
            selected_port: *selected_port,
            edit_active: *edit_active,
            edit_port: edit_port.clone(),
            edit_field_index: *edit_field_index,
            edit_field_key: edit_field_key.clone(),
            edit_buffer: edit_buffer.clone(),
            edit_cursor_pos: *edit_cursor_pos,
        },
        other => panic!(
            "Expected ModbusConfig page for KV panel, found: {:?}",
            other
        ),
    }
}

/// Derive global selection from app state
fn derive_global_selection(app: &Status) -> usize {
    match &app.page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}

/// Render simplified content for config panel
pub fn render_simplified_content(f: &mut Frame, area: Rect, style: Option<Style>) {
    let lines = vec![ratatui::text::Line::from(
        "Configuration panel: details managed in UI layer",
    )];
    render_boxed_paragraph(f, area, lines, style);
}

/// Derive current selection index from full app Status.
pub(crate) fn derive_selection(app: &Status) -> usize {
    match &app.page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::About) => app.ports.order.len().saturating_add(2),
            Some(types::ui::EntryCursor::Refresh) => app.ports.order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => app.ports.order.len().saturating_add(1),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}
