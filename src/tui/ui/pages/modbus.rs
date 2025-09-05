use crossterm::event::{KeyCode as KC, KeyEvent};
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{AppMode, EntryRole, MasterEditField, RegisterMode, Status, SubpageForm},
    tui::ui::components::{
        config_panel::render_config_panel, log_panel::render_log_panel,
        modbus_panel::render_modbus_panel, mqtt_panel::render_mqtt_panel,
    },
    tui::{input::Action, utils::edit},
};

/// Unified ModBus page (merges previous master / slave pages).
pub fn render_modbus(f: &mut Frame, area: Rect, app: &mut Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };
    let mid_label = match app.app_mode {
        AppMode::Modbus => lang().protocol.modbus.label_modbus_settings.as_str(),
        AppMode::Mqtt => "MQTT",
    };
    let tabs = [
        lang().tabs.tab_config.as_str(),
        mid_label,
        lang().tabs.tab_log.as_str(),
    ];
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));
    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
    ])
    .areas(area);

    let mode_text = app.app_mode.label();
    let right_label = format!("{port_name} - {mode_text}");
    let right_width = UnicodeWidthStr::width(right_label.as_str());
    let h_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Min(5),
            ratatui::layout::Constraint::Length((right_width + 2) as u16),
        ])
        .split(header_area);
    let titles = tabs
        .iter()
        .map(|t| Line::from(Span::raw(format!("  {t}  "))));
    let tabs_widget = Tabs::new(titles).select(tab_index);
    f.render_widget(tabs_widget, h_chunks[0]);
    let right_para = Paragraph::new(format!(" {right_label}"))
        .alignment(Alignment::Left)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(right_para, h_chunks[1]);
    match tab_index {
        0 => render_config_panel(f, content_area, app, None),
        1 => {
            if matches!(app.app_mode, AppMode::Modbus) {
                render_modbus_panel(f, content_area, app)
            } else {
                render_mqtt_panel(f, content_area, app)
            }
        }
        2 => render_log_panel(f, content_area, app),
        _ => render_config_panel(f, content_area, app, None),
    }
}

use crate::tui::utils::bus::Bus;

pub fn handle_subpage_key(key: KeyEvent, app: &mut Status, bus: &Bus) -> bool {
    // Perform any pending sync scheduled by UI actions in prior iterations
    if let Some(pn) = app.pending_sync_port.take() {
        app.sync_form_to_slave_context(pn.as_str());
    }
    // Ensure form exists when interacting with modbus settings tab
    if app.subpage_tab_index == 1 && app.subpage_form.is_none() {
        app.init_subpage_form();
    }

    if app.subpage_tab_index == 1 {
        if let Some(form) = app.subpage_form.as_mut() {
            if form.master_field_editing {
                // Editing layer
                let is_type = matches!(form.master_edit_field, Some(MasterEditField::Type));
                let is_role = matches!(form.master_edit_field, Some(MasterEditField::Role));
                match key.code {
                    KC::Esc => {
                        form.master_input_buffer.clear();
                        form.master_field_editing = false;
                        return true;
                    }
                    KC::Enter => {
                        // Limit mutable borrow of form to this block
                        {
                            commit_master_field(form); // Commit current field
                        }
                        // Schedule immediate sync to avoid double borrow of app while form is still borrowed
                        if app.selected < app.ports.len() {
                            let pname = app.ports[app.selected].port_name.clone();
                            app.pending_sync_port = Some(pname);
                        }
                        form.master_field_editing = false;
                        return true;
                    }
                    KC::Left
                    | KC::Char('h')
                    | KC::Right
                    | KC::Char('l')
                    | KC::Up
                    | KC::Char('k')
                    | KC::Down
                    | KC::Char('j') => {
                        // Role / Type cycle while editing
                        if is_type {
                            let forward = matches!(
                                key.code,
                                KC::Right | KC::Char('l') | KC::Down | KC::Char('j')
                            );
                            cycle_type(form, forward);
                            return true;
                        }
                        if is_role {
                            let forward = matches!(
                                key.code,
                                KC::Right | KC::Char('l') | KC::Down | KC::Char('j')
                            );
                            cycle_role(form, forward);
                            return true;
                        }
                        // Value editing: coil toggle (left / right) in coils mode
                        if let Some(MasterEditField::Value(addr)) = form.master_edit_field.clone() {
                            if let Some(idx) = form.master_edit_index {
                                if let Some(entry) = form.registers.get_mut(idx) {
                                    if (entry.mode == RegisterMode::Coils
                                        || entry.mode == RegisterMode::DiscreteInputs)
                                        && matches!(
                                            key.code,
                                            KC::Left | KC::Right | KC::Char('h') | KC::Char('l')
                                        )
                                    {
                                        let off = (addr - entry.address) as usize;
                                        if off < entry.values.len() {
                                            entry.values[off] =
                                                if entry.values[off] == 0 { 1 } else { 0 };
                                        }
                                        return true;
                                    }
                                }
                            }
                        }
                        // Commit then move to next field keeping editing mode
                        {
                            commit_master_field(form);
                        }
                        // After any commit that changes register values/config, schedule sync
                        if app.selected < app.ports.len() {
                            let pname = app.ports[app.selected].port_name.clone();
                            app.pending_sync_port = Some(pname);
                        }
                        form.master_field_editing = false;
                        let enable_values = current_entry_is_master(form);
                        let dir = match key.code {
                            KC::Left | KC::Char('h') => Dir::Left,
                            KC::Right | KC::Char('l') => Dir::Right,
                            KC::Up | KC::Char('k') => Dir::Up,
                            KC::Down | KC::Char('j') => Dir::Down,
                            _ => Dir::Right,
                        };
                        move_master_field_dir(form, dir, enable_values);
                        form.master_field_editing = true;
                        return true;
                    }
                    KC::Backspace => {
                        if !(is_type || is_role) {
                            form.master_input_buffer.pop();
                        }
                        return true;
                    }
                    KC::Char(c) => {
                        if !(is_type || is_role) && c.is_ascii_hexdigit() {
                            form.master_input_buffer.push(c.to_ascii_lowercase());
                        }
                        return true;
                    }
                    _ => return true,
                }
            } else if form.master_field_selected {
                // Selection layer
                match key.code {
                    KC::Esc => {
                        form.master_field_selected = false;
                        form.master_edit_field = None;
                        form.master_edit_index = None;
                        return true;
                    }
                    KC::Enter => {
                        if let Some(field) = &form.master_edit_field {
                            if matches!(field, MasterEditField::Counter) {
                                if let Some(idx) = form.master_edit_index {
                                    if let Some(reg) = form.registers.get_mut(idx) {
                                        reg.req_success = 0;
                                        reg.req_total = 0;
                                    }
                                }
                                return true;
                            }
                        }
                        form.master_field_editing = true;
                        form.master_input_buffer.clear();
                        return true;
                    }
                    KC::Left
                    | KC::Char('h')
                    | KC::Right
                    | KC::Char('l')
                    | KC::Up
                    | KC::Char('k')
                    | KC::Down
                    | KC::Char('j') => {
                        let enable_values = current_entry_is_master(form);
                        let dir = match key.code {
                            KC::Left | KC::Char('h') => Dir::Left,
                            KC::Right | KC::Char('l') => Dir::Right,
                            KC::Up | KC::Char('k') => Dir::Up,
                            KC::Down | KC::Char('j') => Dir::Down,
                            _ => Dir::Right,
                        };
                        move_master_field_dir(form, dir, enable_values);
                        return true;
                    }
                    _ => {}
                }
            } else {
                // Browsing layer
                match key.code {
                    KC::Enter => {
                        if form.master_cursor == form.registers.len() {
                            // Create new entry (default Slave)
                            form.registers.push(crate::protocol::status::RegisterEntry {
                                slave_id: 1,
                                role: EntryRole::Slave,
                                mode: RegisterMode::Coils,
                                address: 0,
                                length: 1,
                                values: vec![0u16; 1],
                                req_success: 0,
                                req_total: 0,
                                next_poll_at: std::time::Instant::now(),
                                pending_requests: Vec::new(),
                            });
                            form.master_cursor = form.registers.len() - 1;
                        }
                        form.master_field_selected = true;
                        form.master_edit_index = Some(form.master_cursor);
                        form.master_edit_field = Some(MasterEditField::Role); // start at Role field
                        form.master_field_editing = true;
                        form.master_input_buffer.clear();
                        return true;
                    }
                    KC::Up | KC::Char('k') => {
                        let total = form.registers.len() + 1;
                        if total > 0 {
                            if form.master_cursor == 0 {
                                form.master_cursor = total - 1;
                            } else {
                                form.master_cursor -= 1;
                            }
                        }
                        return true;
                    }
                    KC::Down | KC::Char('j') => {
                        let total = form.registers.len() + 1;
                        if total > 0 {
                            form.master_cursor = (form.master_cursor + 1) % total;
                        }
                        return true;
                    }
                    KC::Char('d') => {
                        if form.master_cursor < form.registers.len() {
                            form.registers.remove(form.master_cursor);
                            if form.master_cursor >= form.registers.len() && form.master_cursor > 0
                            {
                                form.master_cursor -= 1;
                            }
                        }
                        return true;
                    }
                    _ => {}
                }
            }
        }
    }

    // Tab switching (shared across tabs)
    match key.code {
        KC::Tab => {
            app.subpage_tab_index = (app.subpage_tab_index + 1) % 3;
            true
        }
        KC::BackTab => {
            if app.subpage_tab_index == 0 {
                app.subpage_tab_index = 2;
            } else {
                app.subpage_tab_index -= 1;
            }
            true
        }
        KC::Enter => {
            if app.subpage_tab_index == 0 {
                if let Some(form) = app.subpage_form.as_mut() {
                    // If cursor on first item, toggle immediately and consume
                    if form.cursor == 0 {
                        form.loop_enabled = !form.loop_enabled;
                        // Thread-safe: inform core worker to pause/resume via bus message
                        if form.loop_enabled {
                            let _ = bus
                                .ui_tx
                                .send(crate::tui::utils::bus::UiToCore::ResumePolling);
                        } else {
                            let _ = bus
                                .ui_tx
                                .send(crate::tui::utils::bus::UiToCore::PausePolling);
                        }
                        return true;
                    }
                    // If cursor on second item, toggle master_passive and schedule sync
                    if form.cursor == 1 {
                        // Derived default: if any Master entries exist -> default = Passive
                        let derived_default_passive = form.registers.iter().any(|r| r.role == crate::protocol::status::EntryRole::Master);
                        // effective current: None -> default (passive if derived_default_passive), Some(v) -> v
                        let effective_passive = form.master_passive.unwrap_or(derived_default_passive);
                        // Toggle and store explicit choice
                        form.master_passive = Some(!effective_passive);
                        // schedule sync to avoid mutably borrowing app while still borrowed
                        if app.selected < app.ports.len() {
                            let pname = app.ports[app.selected].port_name.clone();
                            app.pending_sync_port = Some(pname);
                        }
                        return true;
                    }
                    if !form.editing {
                        edit::begin_edit(form);
                    } else {
                        edit::end_edit(form);
                    }
                }
                return true;
            } else if app.subpage_tab_index == 2 {
                // Let global mapping handle input editing
                return false;
            }
            false
        }
        _ => false,
    }
}

pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    if app.subpage_tab_index == 0 {
        // If cursor is on the first field (working toggle), show Enter-to-toggle hint and current state
        if let Some(form) = &app.subpage_form {
            if form.cursor == 0 {
                // Show the localized Enter hint and a status kv showing current running/paused
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
                // Also show movement hint
                hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
                return hints;
            }
        }
        hints.push(lang().hotkeys.press_enter_confirm_edit.as_str().to_string());
        hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
        return hints;
    }
    if app.subpage_tab_index == 1 {
        if let Some(form) = &app.subpage_form {
            if form.master_field_editing {
                if let Some(field) = &form.master_edit_field {
                    match field {
                        MasterEditField::Type | MasterEditField::Role => {
                            hints.push(lang().hotkeys.hint_master_field_apply.as_str().to_string());
                            hints.push(
                                lang()
                                    .hotkeys
                                    .hint_master_field_cancel_edit
                                    .as_str()
                                    .to_string(),
                            );
                            hints.push(lang().hotkeys.hint_master_type_switch.as_str().to_string());
                            // Reuse
                        }
                        MasterEditField::Id
                        | MasterEditField::Start
                        | MasterEditField::End
                        | MasterEditField::Counter
                        | MasterEditField::Value(_) => {
                            if !matches!(field, MasterEditField::Counter) {
                                hints.push(
                                    lang().hotkeys.hint_master_field_apply.as_str().to_string(),
                                );
                                hints.push(
                                    lang()
                                        .hotkeys
                                        .hint_master_field_cancel_edit
                                        .as_str()
                                        .to_string(),
                                );
                                hints
                                    .push(lang().hotkeys.hint_master_edit_hex.as_str().to_string());
                                hints.push(
                                    lang()
                                        .hotkeys
                                        .hint_master_edit_backspace
                                        .as_str()
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            } else if form.master_field_selected {
                hints.push(lang().hotkeys.hint_master_field_select.as_str().to_string());
                hints.push(lang().hotkeys.hint_master_field_move.as_str().to_string());
                hints.push(
                    lang()
                        .hotkeys
                        .hint_master_field_exit_select
                        .as_str()
                        .to_string(),
                );
            } else {
                hints.push(lang().hotkeys.hint_master_enter_edit.as_str().to_string());
                hints.push(lang().hotkeys.hint_master_delete.as_str().to_string());
            }
        }
        return hints;
    }
    if app.subpage_tab_index == 2 {
        if app.input_editing {
            hints.push(lang().hotkeys.press_enter_submit.as_str().to_string());
            hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
        } else {
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "Enter / i",
                lang().input.hint_input_edit_short.as_str(),
            ));
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "m",
                lang().input.hint_input_mode_short.as_str(),
            ));
            let action_label = if app.log_auto_scroll {
                lang().tabs.log.hint_follow_off.as_str()
            } else {
                lang().tabs.log.hint_follow_on.as_str()
            };
            hints.push(crate::tui::ui::bottom::format_kv_hint("p", action_label));
        }
        return hints;
    }
    hints
}

pub fn map_key(key: KeyEvent, _app: &Status) -> Option<Action> {
    match key.code {
        KC::Tab => Some(Action::SwitchNext),
        KC::BackTab => Some(Action::SwitchPrev),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn commit_master_field(form: &mut SubpageForm) {
    if let Some(idx) = form.master_edit_index {
        if let Some(field) = &form.master_edit_field {
            if let Some(entry) = form.registers.get_mut(idx) {
                use MasterEditField::*;
                if form.master_input_buffer.is_empty() {
                    match field {
                        Id => entry.slave_id = 1,
                        Role => {}
                        Type => {}
                        Start => entry.address = 0,
                        End => {
                            entry.length = 1;
                            entry.values.resize(entry.length as usize, 0);
                        }
                        Value(a) => {
                            // If editing value and buffer empty: for Coils/DiscreteInputs keep previous/toggle behaviour;
                            // For other register types treat empty as zero.
                            if entry.mode != RegisterMode::Coils
                                && entry.mode != RegisterMode::DiscreteInputs
                            {
                                let off = (*a as usize).saturating_sub(entry.address as usize);
                                if off < entry.values.len() {
                                    entry.values[off] = 0;
                                }
                            }
                        }
                        Counter => {}
                    }
                } else {
                    let buf = form.master_input_buffer.trim().to_string();
                    let parse_u16 = || {
                        u16::from_str_radix(&buf, 16)
                            .ok()
                            .or_else(|| buf.parse::<u16>().ok())
                    };
                    let parse_u8 = || {
                        u8::from_str_radix(&buf, 16)
                            .ok()
                            .or_else(|| buf.parse::<u8>().ok())
                    };
                    match field {
                        Id => {
                            if let Some(v) = parse_u8() {
                                entry.slave_id = if v == 0 { 1 } else { v };
                            }
                        }
                        Role => {}
                        Type => {
                            if let Some(v) = parse_u8() {
                                entry.mode = RegisterMode::from_u8(v);
                            }
                        }
                        Start => {
                            if let Some(v) = parse_u16() {
                                entry.address = v;
                            }
                        }
                        End => {
                            if let Some(v) = parse_u16() {
                                if v >= entry.address {
                                    let new_len = v - entry.address + 1;
                                    entry.length = new_len;
                                    entry.values.resize(new_len as usize, 0u16);
                                }
                            }
                        }
                        Value(a) => {
                            if let Some(v) = parse_u8() {
                                let off = (*a as usize).saturating_sub(entry.address as usize);
                                if off < entry.values.len() {
                                    entry.values[off] = v as u16;
                                }
                            }
                        }
                        Counter => {}
                    }
                }
                form.master_input_buffer.clear();
            }
        }
    }
}

pub(crate) fn cycle_type(form: &mut SubpageForm, forward: bool) {
    if let Some(idx) = form.master_edit_index {
        if let Some(MasterEditField::Type) = form.master_edit_field {
            if let Some(entry) = form.registers.get_mut(idx) {
                let all = RegisterMode::all();
                let cur_pos = all
                    .iter()
                    .position(|m| *m as u8 == entry.mode as u8)
                    .unwrap_or(0);
                let len = all.len() as i32;
                let mut ni = cur_pos as i32 + if forward { 1 } else { -1 };
                if ni < 0 {
                    ni = len - 1;
                }
                if ni >= len {
                    ni = 0;
                }
                entry.mode = all[ni as usize];
            }
        }
    }
}

pub(crate) fn cycle_role(form: &mut SubpageForm, forward: bool) {
    if let Some(idx) = form.master_edit_index {
        if let Some(MasterEditField::Role) = form.master_edit_field {
            if let Some(entry) = form.registers.get_mut(idx) {
                entry.role = match (entry.role, forward) {
                    (EntryRole::Slave, true) => EntryRole::Master,
                    (EntryRole::Slave, false) => EntryRole::Master,
                    (EntryRole::Master, true) => EntryRole::Slave,
                    (EntryRole::Master, false) => EntryRole::Slave,
                };
            }
        }
    }
}

pub(crate) fn move_master_field_dir(form: &mut SubpageForm, dir: Dir, enable_values: bool) {
    use MasterEditField as F;
    if let Some(idx) = form.master_edit_index {
        if let Some(entry) = form.registers.get(idx) {
            // Build coordinate map
            let mut coords: Vec<(usize, usize, F)> = vec![
                (0, 0, F::Role),
                (0, 1, F::Id),
                (0, 2, F::Type),
                (0, 3, F::Start),
                (0, 4, F::End),
                (0, 5, F::Counter),
            ];
            if enable_values {
                for off in 0..entry.length {
                    let addr = entry.address + off;
                    let row = 1 + (off / 8) as usize;
                    let col = (off % 8) as usize;
                    coords.push((row, col, F::Value(addr)));
                }
            }
            let cur = form.master_edit_field.clone().unwrap_or(F::Role);
            let (crow, ccol) = coords
                .iter()
                .find(|(_, _, f)| *f == cur)
                .map(|(r, c, _)| (*r, *c))
                .unwrap_or((0, 0));
            let target = match dir {
                Dir::Up => {
                    if crow == 0 {
                        (crow, ccol)
                    } else {
                        let nr = crow - 1;
                        if nr == 0 {
                            (0, ccol.min(6))
                        } else {
                            (nr, ccol)
                        }
                    }
                }
                Dir::Down => {
                    let max_row = coords.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
                    if crow == max_row {
                        (crow, ccol)
                    } else {
                        let nr = crow + 1;
                        if nr == 1 {
                            (nr, ccol.min(7))
                        } else {
                            (nr, ccol)
                        }
                    }
                }
                Dir::Left => {
                    if ccol == 0 {
                        (crow, ccol)
                    } else {
                        (crow, ccol - 1)
                    }
                }
                Dir::Right => {
                    let row_width = if crow == 0 { 7 } else { 8 };
                    if ccol + 1 >= row_width {
                        (crow, ccol)
                    } else {
                        (crow, ccol + 1)
                    }
                }
            };
            if let Some((_, _, f)) = coords
                .iter()
                .find(|(r, c, _)| *r == target.0 && *c == target.1)
            {
                form.master_edit_field = Some(f.clone());
                form.master_input_buffer.clear();
            }
        }
    }
}

pub(crate) fn current_entry_is_master(form: &SubpageForm) -> bool {
    if let Some(idx) = form.master_edit_index {
        if let Some(entry) = form.registers.get(idx) {
            return entry.role == EntryRole::Master;
        }
    }
    false
}
