use std::sync::{Arc, RwLock};

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::{ui::components::render_boxed_paragraph, utils::bus::Bus},
};

// no local EditingField import required here

/// Render a configuration panel for a subpage. Only reads from Status, does not mutate.
pub fn render(
    f: &mut Frame,
    area: Rect,
    app: &Status,
    style: Option<Style>,
    _snap: &types::ui::ModbusConfigStatus,
) {
    // Consider the subpage active if `page` is one of the Modbus subpages.
    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. } | types::Page::ModbusDashboard { .. }
    );
    if !subpage_active {
        let lines = vec![ratatui::text::Line::from("No form loaded")];
        return render_boxed_paragraph(f, area, lines, style);
    }
    // Render KV panel when subpage active
    let snap = app.snapshot_modbus_config();
    render_kv_panel(f, area, app, &snap);
    // If the UI needs per-field state it should derive it from Status or local state.
    // For now render a simplified placeholder view.
    let lines = vec![ratatui::text::Line::from(
        "Configuration panel: details managed in UI layer",
    )];
    render_boxed_paragraph(f, area, lines, style);
}

/// Render a two-column key/value list for common serial settings and a few modbus fields.
pub fn render_kv_panel(
    f: &mut Frame,
    area: Rect,
    app: &Status,
    _snap: &types::ui::ModbusConfigStatus,
) {
    use ratatui::widgets::{Block, Borders, List, ListItem};

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
        types::Page::Entry { cursor } => match cursor {
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            _ => 0usize,
        },
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
                (Some(vid), Some(pid)) => format!("{:04x}:{:04x}", vid, pid),
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
            for _ in 0..5 {
                values.push(String::new());
            }
        }
    } else {
        for _ in 0..5 {
            values.push(String::new());
        }
    }

    for (l, v) in labels.into_iter().zip(values.into_iter()) {
        let txt = format!("{:20} {}", l, v);
        items.push(ListItem::new(txt));
    }

    use ratatui::style::{Modifier, Style};
    use ratatui::widgets::ListState;

    // Determine selected row to highlight: use global selection derived from page
    // so the panel reflects the app's current selection (entry list or subpage selected_port).
    let mut selected_row: usize = 0usize;
    // derive_selection is defined in the parent `pages` module; call via super::
    if let Some(sel) = Some(super::derive_selection(app)) {
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

pub fn page_bottom_hints(_app: &Status, _snap: &types::ui::ModbusConfigStatus) -> Vec<String> {
    let hints: Vec<String> = vec![
        lang().hotkeys.hint_move_vertical.as_str().to_string(),
        lang().hotkeys.press_enter_enable.as_str().to_string(),
    ];
    hints
}

/// Global hints for Modbus config/dashboard pages.
pub fn global_hints(app: &Status) -> Vec<String> {
    // Reuse page bottom hints; this keeps global hints consistent with page-level hints.
    let snap = app.snapshot_modbus_config();
    page_bottom_hints(app, &snap)
}

/// Handle input for config panel. Sends commands via UiToCore.
pub fn handle_input(
    key: crossterm::event::KeyEvent,
    app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
    _snap: &types::ui::ModbusConfigStatus,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // Derive selected row in panel (same logic as render_kv_panel)
    let mut selected_row: usize = 0usize;
    if let Some(sel) = Some(super::derive_selection(app)) {
        selected_row = sel;
    }

    // Determine number of fields shown in panel
    let labels_count = {
        let base = 5usize;
        let more = 9usize;
        base + more
    };

    // If currently in edit mode for this port, handle edit-specific keys
    // New location: per-page fields are stored inside Page::ModbusConfig
    let mut in_edit = false;
    if let types::Page::ModbusConfig {
        edit_active: config_edit_active,
        ..
    } = &app.page
    {
        in_edit = *config_edit_active;
    }

    if in_edit {
        // We are editing a field: handle text input and control keys
        match key.code {
            KC::Char(c) => {
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let pos = (*config_edit_cursor_pos).min(config_edit_buffer.len());
                        config_edit_buffer.insert(pos, c);
                        *config_edit_cursor_pos = pos + 1;
                    }
                    Ok(())
                });
                true
            }
            KC::Backspace => {
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let pos = *config_edit_cursor_pos;
                        if pos > 0 && pos <= config_edit_buffer.len() {
                            config_edit_buffer.remove(pos - 1);
                            *config_edit_cursor_pos = pos - 1;
                        }
                    }
                    Ok(())
                });
                true
            }
            KC::Left => {
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        if *config_edit_cursor_pos > 0 {
                            *config_edit_cursor_pos -= 1;
                        }
                    }
                    Ok(())
                });
                true
            }
            KC::Right => {
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        let len = config_edit_buffer.len();
                        if *config_edit_cursor_pos < len {
                            *config_edit_cursor_pos += 1;
                        }
                    }
                    Ok(())
                });
                true
            }
            KC::Enter => {
                // Commit edit: write buffer back to PortData field
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_active: config_edit_active,
                        edit_port: config_edit_port,
                        edit_field_index: config_edit_field_index,
                        edit_field_key: config_edit_field_key,
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        if let Some(port_name) = config_edit_port.clone() {
                            if let Some(pd) = s.ports.map.get_mut(&port_name) {
                                let val = config_edit_buffer.clone();
                                match *config_edit_field_index {
                                    0 => pd.port_name = val,
                                    1 => match val.parse::<u32>() {
                                        Ok(v) => {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.baud = v;
                                            }
                                        }
                                        Err(_) => {}
                                    },
                                    2 => match val.parse::<u8>() {
                                        Ok(v) => {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.data_bits = v as u8;
                                            }
                                        }
                                        Err(_) => {}
                                    },
                                    3 => {
                                        if let Some(rt) = pd.runtime.as_mut() {
                                            rt.current_cfg.parity = match val.as_str() {
                                                "None" | "none" => serialport::Parity::None,
                                                "Odd" | "odd" => serialport::Parity::Odd,
                                                "Even" | "even" => serialport::Parity::Even,
                                                _ => rt.current_cfg.parity.clone(),
                                            }
                                        }
                                    }
                                    4 => match val.parse::<u8>() {
                                        Ok(v) => {
                                            if let Some(rt) = pd.runtime.as_mut() {
                                                rt.current_cfg.stop_bits = v as u8;
                                            }
                                        }
                                        Err(_) => {}
                                    },
                                    5 => pd.port_type = val,
                                    6 => {
                                        pd.extra.guid =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    7 => {}
                                    8 => {
                                        pd.extra.serial =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    9 => {
                                        pd.extra.manufacturer =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    10 => {
                                        pd.extra.product =
                                            if val.is_empty() { None } else { Some(val) }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // Exit edit mode and clear buffer
                        *config_edit_active = false;
                        *config_edit_port = None;
                        *config_edit_field_key = None;
                        config_edit_buffer.clear();
                        *config_edit_cursor_pos = 0;
                    }
                    Ok(())
                });
                let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                true
            }
            KC::Esc => {
                // Cancel edit
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig {
                        edit_active: config_edit_active,
                        edit_port: config_edit_port,
                        edit_field_key: config_edit_field_key,
                        edit_buffer: config_edit_buffer,
                        edit_cursor_pos: config_edit_cursor_pos,
                        ..
                    } = &mut s.page
                    {
                        *config_edit_active = false;
                        *config_edit_port = None;
                        *config_edit_field_key = None;
                        config_edit_buffer.clear();
                        *config_edit_cursor_pos = 0;
                    }
                    Ok(())
                });
                let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                true
            }
            _ => false,
        }
    } else {
        // Not in edit mode: handle navigation and enter/e to begin editing
        match key.code {
            KC::Up | KC::Down | KC::Char('k') | KC::Char('j') => {
                // Update selected_port inside Page::ModbusConfig under write lock
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    if let types::Page::ModbusConfig { selected_port, .. } = &mut s.page {
                        // Move selection by delta based on key
                        match key.code {
                            KC::Up | KC::Char('k') => {
                                if *selected_port > 0 {
                                    *selected_port = selected_port.saturating_sub(1);
                                }
                            }
                            KC::Down | KC::Char('j') => {
                                let max = s.ports.order.len().saturating_sub(1);
                                if *selected_port < max {
                                    *selected_port += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                });
                let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                true
            }
            KC::Enter | KC::Char('e') => {
                // Begin edit for selected row if a real port exists at selection
                // Determine selected port name from app snapshot
                let sel_idx = selected_row.min(labels_count.saturating_sub(1));
                if let Some(port_name) = app.ports.order.get(sel_idx) {
                    // Initialize config_edit in Status
                    let init_buf = if let Some(pd) = app.ports.map.get(port_name) {
                        // Determine value to prefill based on sel_idx (same mapping as render)
                        let pre = match sel_idx {
                            0 => pd.port_name.clone(),
                            1 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.baud.to_string())
                                .unwrap_or_else(|| "9600".to_string()),
                            2 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.data_bits.to_string())
                                .unwrap_or_else(|| "8".to_string()),
                            3 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| format!("{:?}", rt.current_cfg.parity))
                                .unwrap_or_else(|| lang().protocol.common.parity_none.clone()),
                            4 => pd
                                .runtime
                                .as_ref()
                                .map(|rt| rt.current_cfg.stop_bits.to_string())
                                .unwrap_or_else(|| "1".to_string()),
                            5 => pd.port_type.clone(),
                            6 => pd.extra.guid.clone().unwrap_or_default(),
                            7 => match (pd.extra.vid, pd.extra.pid) {
                                (Some(vid), Some(pid)) => format!("{:04x}:{:04x}", vid, pid),
                                _ => String::new(),
                            },
                            8 => pd.extra.serial.clone().unwrap_or_default(),
                            9 => pd.extra.manufacturer.clone().unwrap_or_default(),
                            10 => pd.extra.product.clone().unwrap_or_default(),
                            _ => String::new(),
                        };
                        pre
                    } else {
                        String::new()
                    };

                    let _ = crate::protocol::status::write_status(app_arc, |s| {
                        if let types::Page::ModbusConfig {
                            edit_active: config_edit_active,
                            edit_port: config_edit_port,
                            edit_field_index: config_edit_field_index,
                            edit_field_key: config_edit_field_key,
                            edit_buffer: config_edit_buffer,
                            edit_cursor_pos: config_edit_cursor_pos,
                            ..
                        } = &mut s.page
                        {
                            *config_edit_active = true;
                            *config_edit_port = Some(port_name.clone());
                            *config_edit_field_index = sel_idx;
                            *config_edit_field_key = None;
                            *config_edit_buffer = init_buf.clone();
                            *config_edit_cursor_pos = init_buf.len();
                        }
                        Ok(())
                    });
                    let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                    true
                } else {
                    // No port under selection: just refresh
                    let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                    true
                }
            }
            KC::Esc => {
                // If we reach here we are not in per-field edit mode (in_edit == false)
                // so Esc should return the user to the main entry page.
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    s.page = types::Page::Entry { cursor: None };
                    Ok(())
                });
                let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
                true
            }
            _ => false,
        }
    }
}
