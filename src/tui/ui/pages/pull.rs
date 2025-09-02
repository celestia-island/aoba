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
        master_list_panel::render_master_list_panel, pull_list_panel::render_pull_list_panel,
    },
    tui::{input::Action, utils::edit},
};

/// UI for configuring Modbus slave (pull) settings for the selected port.
pub fn render_pull(f: &mut Frame, area: Rect, app: &mut Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };

    // Listen mode should only have two tabs: [Config, Log]; other modes retain three tabs (including middle list)
    let (tabs, mid_is_list_panel) = match app.port_mode {
        crate::protocol::status::PortMode::Master => (
            vec![
                lang().tabs.tab_config.as_str(),
                lang().protocol.label_master_list.as_str(),
                lang().tabs.tab_log.as_str(),
            ],
            true,
        ),
        crate::protocol::status::PortMode::SlaveStack => (
            vec![
                lang().tabs.tab_config.as_str(),
                lang().protocol.label_slave_listen.as_str(),
                lang().tabs.tab_log.as_str(),
            ],
            true,
        ),
    };
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));

    // Use a single-line header so tabs sit directly above content
    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
    ])
    .areas(area);

    // Right-side mode label (port + mode text) to avoid sticking to last tab
    let mode_text = match app.port_mode {
        crate::protocol::status::PortMode::Master => lang().tabs.master_mode.as_str(),
        crate::protocol::status::PortMode::SlaveStack => lang().tabs.slave_mode.as_str(),
    };
    let right_label = format!("{} - {}", port_name, mode_text); // e.g. "COM1 - 模拟主"
    let right_width = UnicodeWidthStr::width(right_label.as_str());
    // Split header horizontally: tabs (remaining) | right label (exact width + 2 spaces padding)
    let h_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Min(5),
            ratatui::layout::Constraint::Length((right_width + 2) as u16),
        ])
        .split(header_area);

    // Tabs in left region
    let titles = tabs
        .iter()
        .map(|t| Line::from(Span::raw(format!("  {}  ", t))));
    let tabs_widget = Tabs::new(titles).select(tab_index);
    f.render_widget(tabs_widget, h_chunks[0]);

    // Right label with leading space to ensure clear separation
    let right_para = Paragraph::new(format!(" {}", right_label))
        .alignment(Alignment::Left)
        .style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(right_para, h_chunks[1]);

    // Three tabs for remaining modes
    match tab_index {
        0 => render_pull_config(f, content_area, app),
        1 => {
            if mid_is_list_panel {
                match app.port_mode {
                    crate::protocol::status::PortMode::Master => {
                        render_master_list_panel(f, content_area, app)
                    }
                    crate::protocol::status::PortMode::SlaveStack => {
                        render_pull_list_panel(f, content_area, app)
                    }
                }
            } else {
                render_pull_config(f, content_area, app)
            }
        }
        2 => render_pull_log(f, content_area, app),
        _ => render_pull_config(f, content_area, app),
    }
}

fn render_pull_config(f: &mut Frame, area: Rect, app: &mut Status) {
    // Delegate to shared component implementation
    render_config_panel(f, area, app, None);
}

// Registers rendering delegated to master_list_panel (listen panel removed)

fn render_pull_log(f: &mut Frame, area: Rect, _app: &mut Status) {
    // Delegate to shared component implementation
    render_log_panel(f, area, _app);
}

/// Handle key events when pull page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // If editing, let the global editing handler take care of chars / backspace / enter
    if let Some(form) = app.subpage_form.as_ref() {
        if form.editing {
            return true;
        }
    }
    // Tab / arrow navigation is handled by the parent input handler; fall through to allow
    // Parent to process Tab / BackTab / Right / Left / Up / Down when appropriate.

    // Mid tab (index 1) advanced editing for register list (master or slave stack)
    if app.subpage_tab_index == 1 {
        if let Some(form) = app.subpage_form.as_mut() {
            // If in field editing
            if form.master_field_editing {
                let is_type = matches!(
                    form.master_edit_field,
                    Some(crate::protocol::status::MasterEditField::Type)
                );
                match key.code {
                    KC::Esc => {
                        // Cancel editing: discard buffer and stay in selection layer
                        form.master_field_editing = false;
                        form.master_input_buffer.clear();
                        return true;
                    }
                    KC::Enter => {
                        // Commit current field
                        commit_master_field(form);
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
                        // Special toggle for coil value editing: left/right flips boolean without hex input
                        let mut handled_move = false;
                        if let Some(crate::protocol::status::MasterEditField::Value(addr)) =
                            form.master_edit_field.clone()
                        {
                            // Determine if current entry is coils
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
                        if !handled_move {
                            if is_type {
                                // Cycle type when editing Type
                                let forward = matches!(
                                    key.code,
                                    KC::Right | KC::Char('l') | KC::Down | KC::Char('j')
                                );
                                cycle_type(form, forward);
                            } else {
                                // Commit then move to another field entering editing again
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
                            }
                        }
                        return true;
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
                // In selection layer
                match key.code {
                    KC::Esc => {
                        form.master_field_selected = false;
                        form.master_edit_field = None;
                        form.master_edit_index = None;
                        return true;
                    }
                    KC::Enter => {
                        // If selecting Counter field, reset counts instead of entering editing
                        if matches!(
                            form.master_edit_field,
                            Some(crate::protocol::status::MasterEditField::Counter)
                        ) {
                            if let Some(idx) = form.master_edit_index {
                                if let Some(entry) = form.registers.get_mut(idx) {
                                    entry.req_success = 0;
                                    entry.req_total = 0;
                                }
                            }
                            return true;
                        } else {
                            form.master_field_editing = true;
                            form.master_input_buffer.clear();
                            return true;
                        }
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
            } else {
                // Browse layer
                if key.code == KC::Enter {
                    if form.master_cursor == form.registers.len() {
                        // Create new
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
                    }
                    form.master_field_selected = true;
                    form.master_edit_index = Some(form.master_cursor);
                    form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                    form.master_field_editing = true; // Jump straight into editing Id
                    form.master_input_buffer.clear();
                    return true;
                }
                match key.code {
                    // Vertical movement over headers/new line
                    KC::Up | KC::Char('k') => {
                        let total = form.registers.len() + 1; // Includes new line
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
                        // Delete current (not new line)
                        if form.master_cursor < form.registers.len()
                            && !form.master_field_selected
                            && !form.master_field_editing
                        {
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

    match key.code {
        // Allow local Tab-based subpage tab switching (3 tabs: config, list / log)
        KC::Tab => {
            app.subpage_tab_index = (app.subpage_tab_index + 1) % 3;
            return true;
        }
        KC::BackTab => {
            if app.subpage_tab_index == 0 {
                app.subpage_tab_index = 2;
            } else {
                app.subpage_tab_index -= 1;
            }
            return true;
        }
        KC::Enter => {
            // If we are in the middle tab (list / listen panel) and cursor is on the trailing new entry, create a new register entry
            if app.subpage_tab_index == 1 {
                if app.subpage_form.is_none() {
                    app.init_subpage_form();
                }
                if let Some(form) = app.subpage_form.as_mut() {
                    if form.master_cursor == form.registers.len() {
                        form.registers.push(crate::protocol::status::RegisterEntry {
                            slave_id: 1,
                            mode: crate::protocol::status::RegisterMode::Coils,
                            address: 0,
                            length: 1,
                            values: vec![0u8; 1],
                            refresh_ms: 1000,
                            req_success: 0,
                            req_total: 0,
                            next_poll_at: std::time::Instant::now(),
                        });
                        // Move cursor to new entry index (last one)
                        if !form.registers.is_empty() {
                            form.master_cursor = form.registers.len() - 1;
                        }
                        // Enter field selection state
                        form.master_field_selected = true;
                        form.master_edit_index = Some(form.master_cursor);
                        form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                        form.master_field_editing = true;
                        form.master_input_buffer.clear();
                        return true;
                    }
                }
            }
            // Otherwise treat Enter as toggle edit for config tab or submit in editing
            if let Some(form) = app.subpage_form.as_mut() {
                if !form.editing {
                    edit::begin_edit(form);
                } else {
                    edit::end_edit(form);
                }
            }
            return true;
        }
        KC::Char('e') => {
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
            // Add register
            if app.subpage_form.is_none() {
                app.init_subpage_form();
            }
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.push(crate::protocol::status::RegisterEntry {
                    slave_id: 1,
                    mode: crate::protocol::status::RegisterMode::Coils,
                    address: 0,
                    length: 1,
                    values: vec![0u8; 1],
                    refresh_ms: 1000,
                    req_success: 0,
                    req_total: 0,
                    next_poll_at: std::time::Instant::now(),
                });
            }
            return true;
        }
        KC::Char('d') => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.pop();
            }
            return true;
        }
        // Form cursor movement delegated to parent (MoveNext / MovePrev) when subpage is active
        _ => {}
    }
    false
}

/// Provide bottom hints when this page is active as a subpage.
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    if let Some(form) = &app.subpage_form {
        if form.editing {
            if let Some(crate::protocol::status::EditingField::Baud) = &form.editing_field {
                // Distinguish between selecting Custom (not yet confirmed) and deeper editing
                let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
                let custom_idx = presets.len();
                let cur = form.edit_choice_index.unwrap_or_else(|| {
                    presets
                        .iter()
                        .position(|&p| p == form.baud)
                        .unwrap_or(custom_idx)
                });
                if cur == custom_idx && !form.edit_confirmed {
                    hints.push(lang().hotkeys.press_enter_select.as_str().to_string());
                    hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
                } else {
                    hints.push(lang().hotkeys.press_enter_submit.as_str().to_string());
                    hints.push(lang().hotkeys.press_esc_cancel.as_str().to_string());
                }
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
        // Show Enter to open editor and movement hint (Up / Down or k / j)
        hints.push(lang().hotkeys.press_enter_confirm_edit.as_str().to_string());
        hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
        return hints;
    }

    // If current tab is the middle list tab, do not show page-specific hints (leave blank)
    if app.subpage_tab_index == 1 {
        if let Some(form) = &app.subpage_form {
            if form.master_field_editing {
                if let Some(field) = &form.master_edit_field {
                    if matches!(field, crate::protocol::status::MasterEditField::Type) {
                        hints.push(lang().hotkeys.hint_master_field_apply.as_str().to_string());
                        hints.push(
                            lang()
                                .hotkeys
                                .hint_master_field_cancel_edit
                                .as_str()
                                .to_string(),
                        );
                        hints.push(lang().hotkeys.hint_master_type_switch.as_str().to_string());
                    } else if matches!(
                        field,
                        crate::protocol::status::MasterEditField::Id
                            | crate::protocol::status::MasterEditField::Start
                            | crate::protocol::status::MasterEditField::End
                            | crate::protocol::status::MasterEditField::Refresh
                            | crate::protocol::status::MasterEditField::Counter
                    ) {
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
                        );
                    }
                }
            } else if form.master_field_selected {
                if matches!(
                    form.master_edit_field,
                    Some(crate::protocol::status::MasterEditField::Counter)
                ) {
                    hints.push(lang().hotkeys.hint_reset_req_counter.as_str().to_string());
                } else {
                    hints.push(lang().hotkeys.hint_master_field_select.as_str().to_string());
                }
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
            // Show short kv-style hints (press i to edit, press m to toggle mode) in the bottom bar.
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "Enter / i",
                lang().input.hint_input_edit_short.as_str(),
            ));
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "m",
                lang().input.hint_input_mode_short.as_str(),
            ));
            // Show follow / track latest toggle (p) -- show the action (inverse of current state)
            let action_label = if app.log_auto_scroll {
                // Currently following -> hint to stop following
                lang().tabs.log.hint_follow_off.as_str()
            } else {
                // Currently not following -> hint to start following
                lang().tabs.log.hint_follow_on.as_str()
            };
            hints.push(crate::tui::ui::bottom::format_kv_hint("p", action_label));
            // (mode description moved into the input placeholder; skip duplicate)
        }
        return hints;
    }

    // Fallback: no page-specific hints; global line provides navigation
    hints
}

/// Page-level key mapping: allow pull page to map keys to Actions (optional).
pub fn map_key(key: KeyEvent, _app: &Status) -> Option<Action> {
    use crossterm::event::KeyCode as KC;
    match key.code {
        KC::Tab => Some(Action::SwitchNext),
        KC::BackTab => Some(Action::SwitchPrev),
        KC::Enter => Some(Action::EditToggle),
        KC::Char('e') => Some(Action::EditToggle),
        KC::Char('n') => Some(Action::AddRegister),
        KC::Char('d') => Some(Action::DeleteRegister),
        KC::Char('p') => Some(Action::ToggleFollow),
        _ => None,
    }
}

// ---- Shared master/slave list editing helpers (subset) ----
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
            if let Some(entry) = form.registers.get_mut(idx) {
                use crate::protocol::status::MasterEditField::*;
                if form.master_input_buffer.is_empty() {
                    match field {
                        // Disallow 0; empty input resets to 1
                        Id => entry.slave_id = 1,
                        Type => {}
                        Start => entry.address = 0,
                        End => {
                            entry.length = 1;
                            entry.values.resize(entry.length as usize, 0);
                        }
                        Value(a) => {
                            // For coils boolean editing we toggle via arrow keys without buffer; don't overwrite.
                            if entry.mode != crate::protocol::status::RegisterMode::Coils {
                                let off = (*a as usize).saturating_sub(entry.address as usize);
                                if off < entry.values.len() {
                                    entry.values[off] = 0;
                                }
                            }
                        }
                        Refresh => entry.refresh_ms = 1000,
                        Counter => { /* not editable */ }
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
                                entry.slave_id = v;
                            }
                        }
                        Type => {
                            if let Some(v) = parse_u8() {
                                entry.mode = crate::protocol::status::RegisterMode::from_u8(v);
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
                                    entry.values.resize(new_len as usize, 0);
                                }
                            }
                        }
                        Value(a) => {
                            if let Some(v) = parse_u8() {
                                let off = (*a as usize).saturating_sub(entry.address as usize);
                                if off < entry.values.len() {
                                    entry.values[off] = v;
                                }
                            }
                        }
                        Refresh => {
                            if let Ok(v) = buf.parse::<u32>() {
                                entry.refresh_ms = v.max(10);
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

fn move_master_field_dir(
    form: &mut crate::protocol::status::SubpageForm,
    dir: Dir,
    enable_values: bool,
) {
    use crate::protocol::status::MasterEditField as F;
    if let Some(idx) = form.master_edit_index {
        if let Some(entry) = form.registers.get(idx) {
            let mut order: Vec<F> = vec![F::Id, F::Type, F::Start, F::End, F::Refresh, F::Counter];
            if enable_values {
                for off in 0..entry.length {
                    order.push(F::Value(entry.address + off));
                }
            }
            let cur = form.master_edit_field.clone().unwrap_or(F::Id);
            let mut coords: Vec<(usize, usize, F)> = Vec::new();
            coords.push((0, 0, F::Id));
            coords.push((0, 1, F::Type));
            coords.push((0, 2, F::Start));
            coords.push((0, 3, F::End));
            coords.push((0, 4, F::Refresh));
            coords.push((0, 5, F::Counter));
            if enable_values {
                for off in 0..entry.length {
                    let addr = entry.address + off;
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
                        let nr = crow - 1;
                        if nr == 0 {
                            (0, ccol.min(3))
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
                    let row_width = if crow == 0 { 6 } else { 8 };
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
