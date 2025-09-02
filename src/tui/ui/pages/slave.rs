use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::Status,
    tui::ui::components::{
        config_panel::render_config_panel, log_panel::render_log_panel,
        master_list_panel::render_master_list_panel,
    },
    tui::{input::Action, utils::edit},
};

/// UI for configuring Modbus master settings for the selected port.
pub fn render_slave(f: &mut Frame, area: Rect, app: &mut Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };

    // Middle tab label varies by mode:
    // Master => editable list of simulated master requests
    // SlaveStack => passive listening list (register entries not directly editable except header fields)
    let middle_tab = match app.port_mode {
        crate::protocol::status::PortMode::Master => lang().protocol.label_master_list.as_str(),
        crate::protocol::status::PortMode::SlaveStack => {
            lang().protocol.label_slave_listen.as_str()
        }
    };
    let tabs = vec![
        lang().tabs.tab_config.as_str(),
        middle_tab,
        lang().tabs.tab_log.as_str(),
    ];
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));

    // Use a single-line header so tabs sit directly above the content
    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
    ])
    .areas(area);

    // Right-side mode label to separate from tabs and avoid sticking
    let mode_text = match app.port_mode {
        crate::protocol::status::PortMode::Master => lang().tabs.master_mode.as_str(),
        crate::protocol::status::PortMode::SlaveStack => lang().tabs.slave_mode.as_str(),
    };
    let right_label = format!("{} - {}", port_name, mode_text);
    let right_width = UnicodeWidthStr::width(right_label.as_str());
    let h_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Min(5),
            ratatui::layout::Constraint::Length((right_width + 2) as u16),
        ])
        .split(header_area);

    // Tabs left
    let titles = tabs
        .iter()
        .map(|t| Line::from(Span::raw(format!("  {}  ", t))));
    let tabs_widget = Tabs::new(titles).select(tab_index);
    f.render_widget(tabs_widget, h_chunks[0]);

    // Right label
    let right_para = Paragraph::new(format!(" {}", right_label))
        .alignment(Alignment::Left)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(right_para, h_chunks[1]);

    match tab_index {
        0 => render_slave_config(f, content_area, app),
        1 => match app.port_mode {
            crate::protocol::status::PortMode::Master => {
                render_master_list_panel(f, content_area, app)
            }
            crate::protocol::status::PortMode::SlaveStack => {
                crate::tui::ui::components::slave_listen_panel::render_slave_listen_panel(
                    f,
                    content_area,
                    app,
                )
            }
        },
        2 => render_slave_log(f, content_area, app),
        _ => render_slave_config(f, content_area, app),
    }
}

fn render_slave_config(f: &mut Frame, area: Rect, app: &mut Status) {
    render_config_panel(f, area, app, None);
}

fn render_slave_log(f: &mut Frame, area: Rect, _app: &mut Status) {
    render_log_panel(f, area, _app);
}

/// Handle key events when slave page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    let master_tab = app.subpage_tab_index == 1;
    // (Removed legacy listen-mode flag)
    if !master_tab {
        if let Some(form) = app.subpage_form.as_ref() {
            if form.editing {
                return true;
            }
        }
    }
    if master_tab && app.subpage_form.is_none() {
        app.init_subpage_form();
    }
    if master_tab {
        if let Some(form) = app.subpage_form.as_mut() {
            // Editing layer
            if form.master_field_editing {
                let is_type = matches!(
                    form.master_edit_field,
                    Some(crate::protocol::status::MasterEditField::Type)
                );
                match key.code {
                    KC::Esc => {
                        form.master_input_buffer.clear();
                        form.master_field_editing = false;
                        return true;
                    }
                    KC::Enter => {
                        if !is_type {
                            commit_master_field(form);
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
                        let mut handled_move = false;
                        if let Some(crate::protocol::status::MasterEditField::Value(addr)) =
                            form.master_edit_field.clone()
                        {
                            if let Some(idx) = form.master_edit_index {
                                if let Some(entry) = form.registers.get_mut(idx) {
                                    if entry.mode == crate::protocol::status::RegisterMode::Coils {
                                        if key.code == KC::Left
                                            || key.code == KC::Right
                                            || key.code == KC::Char('h')
                                            || key.code == KC::Char('l')
                                        {
                                            let off = (addr - entry.address) as usize;
                                            if off < entry.values.len() {
                                                entry.values[off] =
                                                    if entry.values[off] == 0 { 1 } else { 0 };
                                            }
                                            handled_move = true;
                                        }
                                    }
                                }
                            }
                        }
                        if handled_move {
                            return true;
                        }
                        if is_type {
                            match key.code {
                                KC::Left | KC::Char('h') | KC::Up | KC::Char('k') => {
                                    cycle_type(form, false)
                                }
                                KC::Right | KC::Char('l') | KC::Down | KC::Char('j') => {
                                    cycle_type(form, true)
                                }
                                _ => {}
                            }
                            return true;
                        } else {
                            commit_master_field(form);
                            form.master_field_editing = false;
                            let enable_values =
                                app.port_mode == crate::protocol::status::PortMode::Master;
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
                    }
                    KC::Backspace => {
                        if !is_type {
                            form.master_input_buffer.pop();
                        }
                        return true;
                    }
                    KC::Char(c) => {
                        if !is_type && c.is_ascii_hexdigit() {
                            form.master_input_buffer.push(c.to_ascii_lowercase());
                        }
                        return true;
                    }
                    _ => return true,
                }
            } else if form.master_field_selected {
                match key.code {
                    KC::Esc => {
                        form.master_field_selected = false;
                        form.master_edit_field = None;
                        form.master_edit_index = None;
                        return true;
                    }
                    KC::Enter => {
                        if let Some(field) = &form.master_edit_field {
                            use crate::protocol::status::MasterEditField as MEF;
                            if matches!(field, MEF::Counter) {
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
                        if app.port_mode == crate::protocol::status::PortMode::SlaveStack {
                            app.pause_and_reset_slave_listen();
                        }
                        return true;
                    }
                    KC::Up | KC::Char('k') => {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Up, enable_values);
                        return true;
                    }
                    KC::Down | KC::Char('j') => {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Down, enable_values);
                        return true;
                    }
                    KC::Left | KC::Char('h') => {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Left, enable_values);
                        return true;
                    }
                    KC::Right | KC::Char('l') => {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Right, enable_values);
                        return true;
                    }
                    _ => {}
                }
            } else if key.code == KC::Enter {
                // Browsing layer: create or select
                if form.master_cursor == form.registers.len() {
                    let length = if app.port_mode == crate::protocol::status::PortMode::Master {
                        8
                    } else {
                        1
                    };
                    form.registers.push(crate::protocol::status::RegisterEntry {
                        slave_id: 1,
                        mode: crate::protocol::status::RegisterMode::Coils,
                        address: 0,
                        length,
                        values: vec![0u8; length as usize],
                        refresh_ms: 1000,
                        req_success: 0,
                        req_total: 0,
                        next_poll_at: std::time::Instant::now(),
                    });
                    form.master_cursor = form.registers.len() - 1;
                    form.master_field_selected = true;
                    form.master_edit_index = Some(form.master_cursor);
                    form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                    form.master_field_editing = true;
                    form.master_input_buffer.clear();
                    if app.port_mode == crate::protocol::status::PortMode::SlaveStack {
                        app.pause_and_reset_slave_listen();
                    }
                } else {
                    form.master_field_selected = true;
                    form.master_edit_index = Some(form.master_cursor);
                    form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                }
                return true;
            }
        }
    }
    match key.code {
        KC::Tab => {
            app.subpage_tab_index = (app.subpage_tab_index + 1) % 3;
            return true;
        }
        KC::BackTab => {
            let total = 3;
            if app.subpage_tab_index == 0 {
                app.subpage_tab_index = total - 1;
            } else {
                app.subpage_tab_index -= 1;
            }
            return true;
        }
        KC::Up | KC::Char('k') => {
            if master_tab {
                // Reorder (Ctrl+Up) feature removed
                if let Some(form) = app.subpage_form.as_mut() {
                    if !form.master_field_selected && !form.master_field_editing {
                        let total = form.registers.len() + 1;
                        if total > 0 {
                            if form.master_cursor == 0 {
                                form.master_cursor = total - 1;
                            } else {
                                form.master_cursor -= 1;
                            }
                        }
                    } else if form.master_field_selected {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Up, enable_values);
                    }
                }
                return true;
            }
            if app.subpage_tab_index == 2 {
                return false;
            }
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = if form.cursor == 0 {
                        total - 1
                    } else {
                        form.cursor - 1
                    };
                }
            }
            return true;
        }
        KC::Down | KC::Char('j') => {
            if master_tab {
                // Reorder (Ctrl+Down) feature removed
                if let Some(form) = app.subpage_form.as_mut() {
                    if !form.master_field_selected && !form.master_field_editing {
                        let total = form.registers.len() + 1;
                        if total > 0 {
                            form.master_cursor = (form.master_cursor + 1) % total;
                        }
                    } else if form.master_field_selected {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Down, enable_values);
                    }
                }
                return true;
            }
            if app.subpage_tab_index == 2 {
                return false;
            }
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = (form.cursor + 1) % total;
                }
            }
            return true;
        }
        KC::Left | KC::Char('h') => {
            if master_tab {
                if let Some(form) = app.subpage_form.as_mut() {
                    if form.master_field_selected {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Left, enable_values);
                        return true;
                    }
                }
            }
        }
        KC::Right | KC::Char('l') => {
            if master_tab {
                if let Some(form) = app.subpage_form.as_mut() {
                    if form.master_field_selected {
                        let enable_values =
                            app.port_mode == crate::protocol::status::PortMode::Master;
                        move_master_field_dir(form, Dir::Right, enable_values);
                        return true;
                    }
                }
            }
        }
        // Delete current register request (when not editing and not on the new-entry line)
        KC::Char('d') => {
            if master_tab {
                if app.port_mode == crate::protocol::status::PortMode::Master {
                    if let Some(form) = app.subpage_form.as_mut() {
                        if !form.master_field_selected && !form.master_field_editing {
                            if form.master_cursor < form.registers.len() {
                                form.registers.remove(form.master_cursor);
                                if form.master_cursor >= form.registers.len()
                                    && form.master_cursor > 0
                                {
                                    form.master_cursor -= 1;
                                }
                            }
                        }
                    }
                }
                return true;
            }
        }
        KC::Enter => {
            if master_tab {
                if let Some(form) = app.subpage_form.as_mut() {
                    // Field selection / edit already handled earlier; here we are in the browsing layer
                    if !form.master_field_selected && !form.master_field_editing {
                        if form.master_cursor == form.registers.len() {
                            let length =
                                if app.port_mode == crate::protocol::status::PortMode::Master {
                                    8
                                } else {
                                    1
                                };
                            form.registers.push(crate::protocol::status::RegisterEntry {
                                slave_id: 1,
                                mode: crate::protocol::status::RegisterMode::Coils,
                                address: 0,
                                length,
                                values: vec![0u8; length as usize],
                                refresh_ms: 1000,
                                req_success: 0,
                                req_total: 0,
                                next_poll_at: std::time::Instant::now(),
                            });
                            form.master_cursor = form.registers.len() - 1;
                        } else {
                            form.master_edit_index = Some(form.master_cursor);
                            form.master_edit_field =
                                Some(crate::protocol::status::MasterEditField::Id);
                            form.master_field_selected = true;
                        }
                    }
                }
                return true;
            }
            if let Some(form) = app.subpage_form.as_mut() {
                if !form.editing {
                    edit::begin_edit(form);
                } else {
                    edit::end_edit(form);
                }
            }
            return true;
        }
        KC::Char('n') => {
            if master_tab {
                if app.subpage_form.is_none() {
                    app.init_subpage_form();
                }
                if let Some(form) = app.subpage_form.as_mut() {
                    let len_val = if app.port_mode == crate::protocol::status::PortMode::Master {
                        8
                    } else {
                        1
                    };
                    form.registers.push(crate::protocol::status::RegisterEntry {
                        slave_id: 1,
                        mode: crate::protocol::status::RegisterMode::Coils,
                        address: 0,
                        length: len_val,
                        values: vec![0u8; len_val as usize],
                        refresh_ms: 1000,
                        req_success: 0,
                        req_total: 0,
                        next_poll_at: std::time::Instant::now(),
                    });
                }
                return true;
            }
        }
        _ => {}
    }
    false
}

/// Provide bottom hints when this page is active as a subpage.
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    if let Some(form) = &app.subpage_form {
        if form.editing {
            if !form.edit_confirmed {
                hints.push(lang().hotkeys.press_enter_select.as_str().to_string());
                hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
            } else {
                hints.push(lang().hotkeys.press_enter_submit.as_str().to_string());
                hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
            }
            return hints;
        }
    }
    // If current tab is the log tab, show log-input related hints
    // If current tab is the configuration tab, show config-specific hints
    if app.subpage_tab_index == 0 {
        // Show Enter to open the editor and movement hint (Up / Down or k / j)
        hints.push(lang().hotkeys.press_enter_confirm_edit.as_str().to_string());
        hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
        return hints;
    }

    // Legacy, partially corrupted block removed; new logic implemented below.

    if app.subpage_tab_index == 1 {
        // Middle list tab (master register requests) – mirror the logic in pull.rs
        if let Some(form) = &app.subpage_form {
            if form.master_field_editing {
                if let Some(field) = &form.master_edit_field {
                    use crate::protocol::status::MasterEditField as MEF;
                    if matches!(field, MEF::Type) {
                        hints.push(lang().hotkeys.hint_master_field_apply.as_str().to_string());
                        hints.push(
                            lang()
                                .hotkeys
                                .hint_master_field_cancel_edit
                                .as_str()
                                .to_string(),
                        );
                        hints.push(lang().hotkeys.hint_master_type_switch.as_str().to_string());
                    // Left/Right to cycle
                    } else if matches!(field, MEF::Id | MEF::Start | MEF::End | MEF::Refresh) {
                        hints.push(lang().hotkeys.hint_master_field_apply.as_str().to_string());
                        hints.push(
                            lang()
                                .hotkeys
                                .hint_master_field_cancel_edit
                                .as_str()
                                .to_string(),
                        );
                        hints.push(lang().hotkeys.hint_master_edit_hex.as_str().to_string());
                        hints.push(
                            lang()
                                .hotkeys
                                .hint_master_edit_backspace
                                .as_str()
                                .to_string(),
                        ); // Backspace editing
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
                ); // ESC to exit selection layer
            } else {
                hints.push(lang().hotkeys.hint_master_enter_edit.as_str().to_string());
                hints.push(lang().hotkeys.hint_master_delete.as_str().to_string());
                // 'd' to delete
            }
        }
        return hints;
    }

    if app.subpage_tab_index == 2 {
        if app.input_editing {
            hints.push(lang().hotkeys.press_enter_submit.as_str().to_string());
            hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
        } else {
            // Show concise kv-style hints (press i to edit, press m to toggle log input mode)
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "Enter / i",
                lang().input.hint_input_edit_short.as_str(),
            ));
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "m",
                lang().input.hint_input_mode_short.as_str(),
            ));
            // Show follow / track-latest toggle (p) — display the action inverse of the current state
            let action_label = if app.log_auto_scroll {
                lang().tabs.log.hint_follow_off.as_str()
            } else {
                lang().tabs.log.hint_follow_on.as_str()
            };
            hints.push(crate::tui::ui::bottom::format_kv_hint("p", action_label));
            // Mode description moved into the input placeholder; skip duplicating here
        }
        return hints;
    }

    // For all other fallback cases (rare), return empty list; the global hint line supplies navigation
    hints
}

/// Page-level key mapping: allow slave page to map keys to Actions (optional).
pub fn map_key(key: KeyEvent, _app: &Status) -> Option<Action> {
    use crossterm::event::KeyCode as KC;
    match key.code {
        KC::Tab => Some(Action::SwitchNext),
        KC::BackTab => Some(Action::SwitchPrev),
        KC::Enter => Some(Action::EditToggle), // Creation is handled in the event handler before toggling
        KC::Char('e') => Some(Action::EditToggle),
        KC::Char('n') => Some(Action::AddRegister),
        KC::Up | KC::Char('k') => Some(Action::MovePrev),
        KC::Down | KC::Char('j') => Some(Action::MoveNext),
        _ => None,
    }
}
// ---- Master list editing helper functions ----
#[derive(Clone, Copy)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}
fn commit_master_field(form: &mut crate::protocol::status::SubpageForm) {
    if let Some(idx) = form.master_edit_index {
        if let Some(field) = &form.master_edit_field {
            if let Some(master) = form.registers.get_mut(idx) {
                use crate::protocol::status::MasterEditField::*;
                if form.master_input_buffer.is_empty() {
                    match field {
                        // Empty input resets to 1; 0 is not allowed
                        Id => master.slave_id = 1,
                        Type => { /* keep original value (do not reset) */ }
                        Start => master.address = 0,
                        End => {
                            master.length = 1;
                            master.values.resize(master.length as usize, 0);
                        }
                        Value(a) => {
                            // Preserve toggled boolean for coils when buffer empty.
                            if master.mode != crate::protocol::status::RegisterMode::Coils {
                                let off = (*a as usize).saturating_sub(master.address as usize);
                                if off < master.values.len() {
                                    master.values[off] = 0;
                                }
                            }
                        }
                        Refresh => master.refresh_ms = 1000,
                        Counter => { /* pressing Enter on Counter does reset elsewhere; on commit do nothing */
                        }
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
                                let v = if v == 0 { 1 } else { v }; // Enforce non-zero
                                master.slave_id = v;
                            }
                        }
                        Type => {
                            if let Some(v) = parse_u8() {
                                master.mode = crate::protocol::status::RegisterMode::from_u8(v);
                            }
                        }
                        Start => {
                            if let Some(v) = parse_u16() {
                                master.address = v;
                            }
                        }
                        End => {
                            if let Some(v) = parse_u16() {
                                if v >= master.address {
                                    let new_len = v - master.address + 1;
                                    master.length = new_len;
                                    master.values.resize(new_len as usize, 0);
                                }
                            }
                        }
                        Value(a) => {
                            if let Some(v) = parse_u8() {
                                let off = (*a as usize).saturating_sub(master.address as usize);
                                if off < master.values.len() {
                                    master.values[off] = v;
                                }
                            }
                        }
                        Refresh => {
                            if let Ok(v) = buf.parse::<u32>() {
                                master.refresh_ms = v.max(10);
                            }
                        }
                        Counter => { /* not editable */ }
                    }
                }
                form.master_input_buffer.clear();
            }
        }
    }
}

fn cycle_type(form: &mut crate::protocol::status::SubpageForm, forward: bool) {
    use crate::protocol::status::RegisterMode;
    if let Some(idx) = form.master_edit_index {
        if let Some(crate::protocol::status::MasterEditField::Type) = form.master_edit_field {
            if let Some(master) = form.registers.get_mut(idx) {
                let all = RegisterMode::all();
                let cur_pos = all
                    .iter()
                    .position(|m| *m as u8 == master.mode as u8)
                    .unwrap_or(0);
                let len = all.len() as i32;
                let mut ni = cur_pos as i32 + if forward { 1 } else { -1 };
                if ni < 0 {
                    ni = len - 1;
                }
                if ni >= len {
                    ni = 0;
                }
                master.mode = all[ni as usize];
            }
        }
    }
}

fn move_master_field_dir(
    form: &mut crate::protocol::status::SubpageForm,
    dir: Dir,
    enable_values: bool,
) {
    use crate::protocol::status::MasterEditField as F;
    if let Some(idx) = form.master_edit_index {
        if let Some(master) = form.registers.get(idx) {
            // Build a grid: header row (Id, Type, Start, End, Refresh, Counter)
            let mut order: Vec<F> = vec![F::Id, F::Type, F::Start, F::End, F::Refresh, F::Counter];
            if enable_values {
                for off in 0..master.length {
                    order.push(F::Value(master.address + off));
                }
            }
            let cur = form.master_edit_field.clone().unwrap_or(F::Id);
            // Map each field to (row,col)
            let mut coords: Vec<(usize, usize, F)> = Vec::new();
            // Header row uses row=0, columns 0..4
            coords.push((0, 0, F::Id));
            coords.push((0, 1, F::Type));
            coords.push((0, 2, F::Start));
            coords.push((0, 3, F::End));
            coords.push((0, 4, F::Refresh));
            coords.push((0, 5, F::Counter));
            // Value grid starts at row=1; each row has up to 8 columns
            if enable_values {
                for off in 0..master.length {
                    let addr = master.address + off;
                    let row = 1 + (off / 8) as usize;
                    let col = (off % 8) as usize;
                    coords.push((row, col, F::Value(addr)));
                }
            }
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
                        // Move up one row (if above header, stay)
                        let nr = crow - 1; // Header row has only 4 cols
                        if nr == 0 {
                            // Clamp ccol to 0..3
                            (0, ccol.min(3))
                        } else {
                            (nr, ccol)
                        }
                    }
                }
                Dir::Down => {
                    // Attempt down
                    let max_row = coords.iter().map(|(r, _, _)| *r).max().unwrap_or(0);
                    if crow == max_row {
                        (crow, ccol)
                    } else {
                        let nr = crow + 1;
                        if nr == 1 {
                            // Leaving header to values
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
                    // Width depends on row
                    let row_width = if crow == 0 { 6 } else { 8 }; // header now has 6 columns including Counter
                    if ccol + 1 >= row_width {
                        (crow, ccol)
                    } else {
                        (crow, ccol + 1)
                    }
                }
            };
            // Find a field with those target coordinates (if none, stay in place)
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
