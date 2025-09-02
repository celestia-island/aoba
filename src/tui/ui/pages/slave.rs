use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

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

    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            port_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    Span::raw(format!(" - {}", lang().tabs.tab_master.as_str())),
    ]);

    // Middle tab label varies by mode:
    // Master => editable simulated masters list
    // SlaveStack / Listen => passive listen list (registers not directly editable except header fields)
    let middle_tab = match app.port_mode {
        crate::protocol::status::PortMode::Master => lang().protocol.label_master_list.as_str(),
        crate::protocol::status::PortMode::SlaveStack => lang().protocol.label_slave_listen.as_str(),
    };
    let tabs = vec![
        lang().tabs.tab_config.as_str(),
        middle_tab,
        lang().tabs.tab_log.as_str(),
    ];
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));

    // Use a single-line header so tabs sit directly above content (no extra empty row)
    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Min(0),
    ])
    .areas(area);

    // Header: title and tabs
    let [tabs_area, title_area] = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(20),
    ])
    .areas(header_area);

    // Title
    f.render_widget(Paragraph::new(title_line), title_area);

    // Tabs
    let titles = tabs
        .iter()
        .map(|t| Line::from(Span::raw(format!("  {}  ", t))));
    let tabs_widget = Tabs::new(titles).select(tab_index);
    f.render_widget(tabs_widget, tabs_area);

    match tab_index {
        0 => render_slave_config(f, content_area, app),
        1 => match app.port_mode {
            crate::protocol::status::PortMode::Master => {
                render_master_list_panel(f, content_area, app)
            }
            _ => render_master_list_panel(f, content_area, app),
        },
        2 => render_slave_log(f, content_area, app),
        _ => render_slave_config(f, content_area, app),
    }
}

fn render_slave_config(f: &mut Frame, area: Rect, app: &mut Status) {
    // Delegate to shared component implementation
    render_config_panel(f, area, app, None);
}

// Registers rendering delegated to master_list_panel (listen panel removed)

fn render_slave_log(f: &mut Frame, area: Rect, _app: &mut Status) {
    // Delegate to shared component implementation
    render_log_panel(f, area, _app);
}

/// Handle key events when slave page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    let master_tab = app.subpage_tab_index == 1;
    let _is_listen_like = false; // Removed Listen reference
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
            // Field editing stage
            if form.master_field_editing {
                // Special: the Type field skips input buffer; cycle with left/right or up/down
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
                        } // Type commits live (no buffer)
                        form.master_field_editing = false;
                        return true;
                    }
                    // While editing a non-Type field, allow movement keys to auto-commit and move to adjacent field
                    KC::Left
                    | KC::Char('h')
                    | KC::Right
                    | KC::Char('l')
                    | KC::Up
                    | KC::Char('k')
                    | KC::Down
                    | KC::Char('j') => {
                        if is_type {
                            // Keep previous behavior for Type spinner
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
                            // Commit current field then move focus (stay in selection layer)
                            commit_master_field(form);
                            form.master_field_editing = false; // Exit editing
                                                               // After exiting editing we are in field-selected layer; perform movement
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
                        // Exit field selection layer
                        form.master_field_selected = false;
                        form.master_edit_field = None;
                        form.master_edit_index = None;
                        return true;
                    }
                    KC::Enter => {
                        // Enter field editing (Type supports immediate left/right cycling)
                        form.master_field_editing = true;
                        form.master_input_buffer.clear();
                        return true;
                    }
                    KC::Up | KC::Char('k') => {
                        // In listen-like modes, disable value cell navigation (only header + Refresh)
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
                // Not in field selection or editing; allow Enter on trailing new line to create entry
                if key.code == KC::Enter {
                    if form.master_cursor == form.registers.len() {
                        // Creation allowed in all modes; default length differs for Master vs others
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
                        });
                        form.master_cursor = form.registers.len() - 1;
                        form.master_field_selected = true;
                        form.master_edit_index = Some(form.master_cursor);
                        form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                        form.master_field_editing = true;
                        form.master_input_buffer.clear();
                        return true;
                    } else {
                        form.master_field_selected = true;
                        form.master_edit_index = Some(form.master_cursor);
                        form.master_edit_field = Some(crate::protocol::status::MasterEditField::Id);
                        return true;
                    }
                }
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
        // Delete current master (not editing and not on the new entry line)
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
                    // Field selection/edit already handled earlier; here we are in browse layer
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
                if app.subpage_form.is_none() { app.init_subpage_form(); }
                if let Some(form) = app.subpage_form.as_mut() {
                    form.registers.push(crate::protocol::status::RegisterEntry {
                        slave_id: 1,
                        mode: crate::protocol::status::RegisterMode::Coils,
                        address: 0,
                        length: if app.port_mode == crate::protocol::status::PortMode::Master { 8 } else { 1 },
                        values: vec![0u8; if app.port_mode == crate::protocol::status::PortMode::Master { 8 } else { 1 }],
                        refresh_ms: 1000,
                        req_success: 0,
                        req_total: 0,
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
        // Show Enter to open editor and movement hint (Up / Down or k / j)
    hints.push(lang().hotkeys.press_enter_confirm_edit.as_str().to_string());
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
        return hints;
    }

    // (Corrupted legacy block removed; new logic implemented below.)

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
                lang().tabs.log.hint_follow_off.as_str()
            } else {
                lang().tabs.log.hint_follow_on.as_str()
            };
            hints.push(crate::tui::ui::bottom::format_kv_hint("p", action_label));
            // (mode description moved into the input placeholder; skip duplicate)
        }
        return hints;
    }

    hints.push(lang().hotkeys.hint_back_list.as_str().to_string());
    hints.push(lang().hotkeys.hint_switch_tab.as_str().to_string());
    hints
}

/// Page-level key mapping: allow slave page to map keys to Actions (optional).
pub fn map_key(key: KeyEvent, _app: &Status) -> Option<Action> {
    use crossterm::event::KeyCode as KC;
    match key.code {
        KC::Tab => Some(Action::SwitchNext),
        KC::BackTab => Some(Action::SwitchPrev),
        KC::Enter => Some(Action::EditToggle), // Creation handled in handler before toggle
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
                        Id => master.slave_id = 0,
                        Type => { /* keep original value (do not reset) */ }
                        Start => master.address = 0,
                        End => {
                            master.length = 1;
                            master.values.resize(master.length as usize, 0);
                        }
                        Value(a) => {
                            let off = (*a as usize).saturating_sub(master.address as usize);
                            if off < master.values.len() {
                                master.values[off] = 0;
                            }
                        }
                        Refresh => master.refresh_ms = 1000,
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
            // Build grid: header row (Id, Type, Start, End, Refresh) then value cells (value navigation allowed only in Master mode)
            let mut order: Vec<F> = vec![F::Id, F::Type, F::Start, F::End, F::Refresh];
            if enable_values {
                for off in 0..master.length {
                    order.push(F::Value(master.address + off));
                }
            }
            let cur = form.master_edit_field.clone().unwrap_or(F::Id);
            // Map each field to (row,col)
            let mut coords: Vec<(usize, usize, F)> = Vec::new();
            // Header row row=0, columns 0..4
            coords.push((0, 0, F::Id));
            coords.push((0, 1, F::Type));
            coords.push((0, 2, F::Start));
            coords.push((0, 3, F::End));
            coords.push((0, 4, F::Refresh));
            // Value grid starts at row=1; each row has 8 columns
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
                    let row_width = if crow == 0 { 5 } else { 8 };
                    if ccol + 1 >= row_width {
                        (crow, ccol)
                    } else {
                        (crow, ccol + 1)
                    }
                }
            };
            // Find nearest field with those coords (if none, stay)
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
