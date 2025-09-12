use std::cmp::min;

use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::Line,
};

use crate::tui::utils::bus::Bus;
use crate::{
    i18n::lang,
    protocol::status::{EntryRole, MasterEditField, RegisterEntry, Status},
    tui::ui::components::{render_boxed_paragraph, styled_spans, StyledSpanKind, TextState},
};
use crossterm::event::{KeyCode as KC, KeyEvent};

/// Full subpage key handling migrated from `pages::modbus.rs`.
pub fn handle_subpage_key(key: KeyEvent, app: &mut Status, bus: &Bus) -> bool {
    // Perform any pending sync scheduled by UI actions in prior iterations
    if let Some(pn) = app.per_port.pending_sync_port.take() {
        app.sync_form_to_slave_context(pn.as_str());
    }
    // Ensure form exists when interacting with modbus settings tab
    if app.page.subpage_tab_index == crate::protocol::status::SubpageTab::Body
        && app.page.subpage_form.is_none()
    {
        app.init_subpage_form();
    }

    if app.page.subpage_tab_index == crate::protocol::status::SubpageTab::Body {
        // Cache selected index and port name to avoid additional immutable borrows
        // while we hold a mutable borrow to `app.ui.subpage_form` below.
        let selected_index = app.page.selected;
        let selected_port_name = if selected_index < app.ports.list.len() {
            Some(app.ports.list[selected_index].port_name.clone())
        } else {
            None
        };
        if let Some(form) = app.page.subpage_form.as_mut() {
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
                        if let Some(pname) = selected_port_name.clone() {
                            app.per_port.pending_sync_port = Some(pname);
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
                                    if (entry.mode == crate::protocol::status::RegisterMode::Coils
                                        || entry.mode == crate::protocol::status::RegisterMode::DiscreteInputs)
                                        && matches!(
                                            key.code,
                                            KC::Left | KC::Right | KC::Char('h') | KC::Char('l')
                                        )
                                    {
                                        let off = (addr - entry.address) as usize;
                                        if off < entry.values.len() {
                                            entry.values[off] = if entry.values[off] == 0 { 1 } else { 0 };
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
                        if let Some(pname) = selected_port_name.clone() {
                            app.per_port.pending_sync_port = Some(pname);
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
                                mode: crate::protocol::status::RegisterMode::Coils,
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
            // cycle forward: compute new usize, wrap by 3 and map back to enum
            let new_idx = (app.page.subpage_tab_index.as_usize() + 1) % 3;
            app.page.subpage_tab_index = match new_idx {
                0 => crate::protocol::status::SubpageTab::Config,
                1 => crate::protocol::status::SubpageTab::Body,
                2 => crate::protocol::status::SubpageTab::Log,
                _ => crate::protocol::status::SubpageTab::Config,
            };
            true
        }
        KC::BackTab => {
            let cur = app.page.subpage_tab_index.as_usize();
            let new_idx = if cur == 0 { 2 } else { cur - 1 };
            app.page.subpage_tab_index = match new_idx {
                0 => crate::protocol::status::SubpageTab::Config,
                1 => crate::protocol::status::SubpageTab::Body,
                2 => crate::protocol::status::SubpageTab::Log,
                _ => crate::protocol::status::SubpageTab::Config,
            };
            true
        }
        KC::Enter => {
            if app.page.subpage_tab_index == crate::protocol::status::SubpageTab::Config {
                // Cache selected index and port name before taking a mutable borrow to `app.ui.subpage_form`.
                let __selected_index = app.page.selected;
                let __selected_port_name = if __selected_index < app.ports.list.len() {
                    Some(app.ports.list[__selected_index].port_name.clone())
                } else {
                    None
                };
                if let Some(form) = app.page.subpage_form.as_mut() {
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
                        let derived_default_passive = form
                            .registers
                            .iter()
                            .any(|r| r.role == crate::protocol::status::EntryRole::Master);
                        // effective current: None -> default (passive if derived_default_passive), Some(v) -> v
                        let effective_passive =
                            form.master_passive.unwrap_or(derived_default_passive);
                        // Toggle and store explicit choice
                        form.master_passive = Some(!effective_passive);
                        // schedule sync to avoid mutably borrowing app while still borrowed
                        if let Some(pname) = __selected_port_name.clone() {
                            app.per_port.pending_sync_port = Some(pname);
                        }
                        return true;
                    }
                    if !form.editing {
                        crate::tui::utils::edit::begin_edit(form);
                    } else {
                        crate::tui::utils::edit::end_edit(form);
                    }
                }
                return true;
            } else if app.page.subpage_tab_index == crate::protocol::status::SubpageTab::Log {
                // Let global mapping handle input editing
                return false;
            }
            false
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn commit_master_field(form: &mut crate::protocol::status::SubpageForm) {
    use crate::protocol::status::{MasterEditField, RegisterMode};
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

pub(crate) fn cycle_type(form: &mut crate::protocol::status::SubpageForm, forward: bool) {
    if let Some(idx) = form.master_edit_index {
        if let Some(crate::protocol::status::MasterEditField::Type) = form.master_edit_field {
            if let Some(entry) = form.registers.get_mut(idx) {
                let all = crate::protocol::status::RegisterMode::all();
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

pub(crate) fn cycle_role(form: &mut crate::protocol::status::SubpageForm, forward: bool) {
    if let Some(idx) = form.master_edit_index {
        if let Some(crate::protocol::status::MasterEditField::Role) = form.master_edit_field {
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

pub(crate) fn move_master_field_dir(
    form: &mut crate::protocol::status::SubpageForm,
    dir: Dir,
    enable_values: bool,
) {
    use crate::protocol::status::MasterEditField as F;
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

pub(crate) fn current_entry_is_master(form: &crate::protocol::status::SubpageForm) -> bool {
    if let Some(idx) = form.master_edit_index {
        if let Some(entry) = form.registers.get(idx) {
            return entry.role == EntryRole::Master;
        }
    }
    false
}

/// Unified ModBus list panel: each entry has a Role (Master / Slave) + fields previously split.
/// Added first editable field: Role, allowing switching between Master and Slave behaviors.
pub fn render_modbus_panel(f: &mut Frame, area: Rect, app: &mut Status) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(form) = app.page.subpage_form.as_ref() {
        if form.registers.is_empty() {
            let sel = form.master_cursor == 0;
            let prefix = if sel { "> " } else { "  " };
            let line = format!("{}[+] {}", prefix, lang().protocol.modbus.new_entry);
            if sel {
                lines.push(Line::styled(line, Style::default().fg(Color::Green)));
            } else {
                lines.push(Line::from(line));
            }
        } else {
            for (i, r) in form.registers.iter().enumerate() {
                render_entry_header(&mut lines, i, r, form);
                render_entry_values(&mut lines, i, r, form);
                lines.push(Line::from(""));
            }
            let new_sel = form.master_cursor == form.registers.len();
            let new_line = format!(
                "{}[+] {}",
                if new_sel { "> " } else { "  " },
                lang().protocol.modbus.new_entry
            );
            if new_sel {
                lines.push(Line::styled(new_line, Style::default().fg(Color::Green)));
            } else {
                lines.push(Line::from(new_line));
            }
        }
    } else {
        lines.push(Line::from(lang().index.details_placeholder.as_str()));
        lines.push(Line::from(format!(
            "[+] {}",
            lang().protocol.modbus.new_entry
        )));
    }

    // Scroll logic copied from master / pull panels
    let inner_height = area.height.saturating_sub(2) as usize;
    let mut first_visible = 0usize;
    if let Some(form) = app.page.subpage_form.as_ref() {
        let mut cursor_line = 0usize;
        let mut accum = 0usize;
        for (i, r) in form.registers.iter().enumerate() {
            if i == form.master_cursor {
                cursor_line = accum;
                break;
            }
            let val_lines = if r.length == 0 {
                0
            } else {
                (r.length as usize).div_ceil(8)
            };
            accum += 1 + val_lines + 1;
        }
        if form.master_cursor == form.registers.len() {
            cursor_line = accum;
        }
        if form.master_field_selected {
            if let Some(idx) = form.master_edit_index {
                if idx < form.registers.len() {
                    if let Some(MasterEditField::Value(a)) = form.master_edit_field.as_ref() {
                        let r_cur = &form.registers[idx];
                        if *a >= r_cur.address && *a < r_cur.address + r_cur.length {
                            let mut line_no = 0usize;
                            for (i2, r2) in form.registers.iter().enumerate() {
                                let lines2 = if r2.length == 0 {
                                    0
                                } else {
                                    (r2.length as usize).div_ceil(8)
                                };
                                if i2 == idx {
                                    line_no += 1;
                                    if r2.length > 0 {
                                        let off = *a as usize - r2.address as usize;
                                        line_no += off / 8;
                                    }
                                    if line_no < first_visible {
                                        first_visible = line_no;
                                    } else if line_no >= first_visible + inner_height {
                                        first_visible = line_no + 1 - inner_height;
                                    }
                                    break;
                                } else {
                                    line_no += 1 + lines2 + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        if cursor_line >= first_visible + inner_height {
            first_visible = cursor_line + 1 - inner_height;
        }
    }
    let total = lines.len();
    let last_start = total.saturating_sub(inner_height);
    if first_visible > last_start {
        first_visible = last_start;
    }
    let end = min(total, first_visible + inner_height);
    render_boxed_paragraph(f, area, lines[first_visible..end].to_vec(), None);
    if total > inner_height && inner_height > 0 {
        render_scroll_bar(f, area, first_visible, inner_height, total);
    }
}

fn render_entry_header(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
) {
    use MasterEditField as F;
    let selected = form.master_cursor == idx;
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };
    // Removed unused style bindings
    let mut spans: Vec<Span> = Vec::new();
    // Use helper to render prefix and index
    spans.extend(styled_spans(StyledSpanKind::PrefixIndex {
        idx,
        selected,
        chosen,
    }));
    spans.push(Span::raw(", "));
    // Role field (new first editable)
    let role_active = matches!(cur_field, Some(F::Role)) && editing;
    // Role style computed inline where used; removed unused binding
    let role_label = match r.role {
        EntryRole::Master => lang().protocol.modbus.role_master.as_str(),
        EntryRole::Slave => lang().protocol.modbus.role_slave.as_str(),
    };
    if role_active {
        spans.extend(styled_spans(StyledSpanKind::Selector {
            base_prefix: "",
            label: role_label,
            state: TextState::Editing,
        }));
    } else if chosen && matches!(cur_field, Some(F::Role)) {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("[{role_label}]").as_str(),
            state: TextState::Chosen,
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: role_label.to_string().as_str(),
            state: if selected && !chosen {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    spans.push(Span::raw(", "));
    // ID
    let id_active = matches!(cur_field, Some(F::Id)) && editing;
    // Id style computed inline where used; removed unused binding
    if id_active {
        let content = if form.master_input_buffer.is_empty() {
            "_".to_string()
        } else {
            form.master_input_buffer.clone()
        };
        spans.push(Span::raw("ID = "));
        spans.extend(styled_spans(StyledSpanKind::Input {
            base_prefix: "",
            buffer: content.as_str(),
            hovered: chosen,
            editing: true,
            with_prefix: true,
        }));
    } else if chosen && matches!(cur_field, Some(F::Id)) {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("ID = [{:02X}]", r.slave_id).as_str(),
            state: TextState::Chosen,
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("ID = {:02X}", r.slave_id).as_str(),
            state: if selected && !chosen {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    spans.push(Span::raw(", "));
    // Type
    let types = [
        lang().protocol.modbus.reg_type_coils.as_str(),
        lang().protocol.modbus.reg_type_discrete_inputs.as_str(),
        lang().protocol.modbus.reg_type_holding.as_str(),
        lang().protocol.modbus.reg_type_input.as_str(),
    ];
    let type_idx = ((r.mode as u8 as usize).saturating_sub(1)).min(3);
    if matches!(cur_field, Some(F::Type)) && editing {
        spans.extend(styled_spans(StyledSpanKind::Selector {
            base_prefix: "",
            label: types[type_idx],
            state: TextState::Editing,
        }));
    } else if chosen && matches!(cur_field, Some(F::Type)) {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("[{}]", types[type_idx]).as_str(),
            state: TextState::Chosen,
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: types[type_idx].to_string().as_str(),
            state: if selected && !chosen {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    spans.push(Span::raw(", "));
    // Address range
    let start = r.address as u32;
    let end_inclusive = start + r.length as u32 - 1;
    // Keep the address-range label in default style (no green), per UI spec.
    spans.extend(styled_spans(StyledSpanKind::Text {
        text: format!("{} = ", lang().protocol.modbus.label_address_range).as_str(),
        state: TextState::Normal,
        bold: false,
    }));
    let start_active = matches!(cur_field, Some(F::Start)) && editing;
    // Start style computed inline where used; removed unused binding
    if start_active {
        let content = if form.master_input_buffer.is_empty() {
            "_".to_string()
        } else {
            form.master_input_buffer.clone()
        };
        spans.extend(styled_spans(StyledSpanKind::Input {
            base_prefix: "",
            buffer: content.as_str(),
            hovered: chosen,
            editing: true,
            with_prefix: true,
        }));
    } else if chosen && matches!(cur_field, Some(F::Start)) {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("0x[{start:04X}]").as_str(),
            state: TextState::Chosen,
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("0x{start:04X}").as_str(),
            state: if selected && !chosen {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    spans.push(Span::raw(" - "));
    let end_active = matches!(cur_field, Some(F::End)) && editing;
    // End style computed inline where used; removed unused binding
    if end_active {
        let content = if form.master_input_buffer.is_empty() {
            "_".to_string()
        } else {
            form.master_input_buffer.clone()
        };
        spans.extend(styled_spans(StyledSpanKind::Input {
            base_prefix: "",
            buffer: content.as_str(),
            hovered: chosen,
            editing: true,
            with_prefix: true,
        }));
    } else if chosen && matches!(cur_field, Some(F::End)) {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("0x[{end_inclusive:04X}]").as_str(),
            state: TextState::Chosen,
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("0x{end_inclusive:04X}").as_str(),
            state: if selected && !chosen {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    // Counter (refresh removed; global interval applies)
    spans.push(Span::raw(", "));
    let counter_label = lang().protocol.modbus.label_req_counter.as_str();
    let counter_selected = chosen && matches!(cur_field, Some(F::Counter));
    // Counter style computed inline where used; removed unused binding
    if counter_selected {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("{} = [{} / {}]", counter_label, r.req_success, r.req_total).as_str(),
            state: if counter_selected {
                TextState::Chosen
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    } else {
        spans.extend(styled_spans(StyledSpanKind::Text {
            text: format!("{} = {} / {}", counter_label, r.req_success, r.req_total).as_str(),
            state: if counter_selected {
                TextState::Chosen
            } else {
                TextState::Normal
            },
            bold: false,
        }));
    }
    out.push(Line::from(spans));
}

fn render_entry_values(
    out: &mut Vec<Line>,
    idx: usize,
    r: &RegisterEntry,
    form: &crate::protocol::status::SubpageForm,
) {
    use MasterEditField as F;
    if r.length == 0 {
        return;
    }
    let chosen = form.master_field_selected && form.master_edit_index == Some(idx);
    let editing = form.master_field_editing && form.master_edit_index == Some(idx);
    let cur_field = if chosen {
        form.master_edit_field.clone()
    } else {
        None
    };
    let start = r.address as usize;
    let end_exclusive = start + r.length as usize;
    // Removed unused style bindings
    let mut addr = start;
    while addr < end_exclusive {
        let line_base = (addr / 8) * 8;
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(format!("      0x{line_base:04X}: ")));
        for col in 0..8 {
            if col > 0 {
                spans.push(Span::raw(" "));
            }
            let cur = line_base + col;
            if cur < start || cur >= end_exclusive {
                spans.push(Span::raw("__"));
                continue;
            }
            let offset = cur - start;
            let raw_val = r.values.get(offset).cloned().unwrap_or(0u16);
            // Style computed inline where used; removed unused binding
            if let Some(F::Value(a)) = &cur_field {
                if *a as usize == cur && editing {
                    // For Coils and DiscreteInputs show boolean editor; otherwise show numeric editor
                    if r.mode == crate::protocol::status::RegisterMode::Coils
                        || r.mode == crate::protocol::status::RegisterMode::DiscreteInputs
                    {
                        let lbl = if raw_val != 0 {
                            lang().protocol.modbus.value_true.as_str()
                        } else {
                            lang().protocol.modbus.value_false.as_str()
                        };
                        spans.extend(styled_spans(StyledSpanKind::Selector {
                            base_prefix: "",
                            label: lbl,
                            state: TextState::Editing,
                        }));
                    } else {
                        let content = if form.master_input_buffer.is_empty() {
                            "_".to_string()
                        } else {
                            form.master_input_buffer.clone()
                        };
                        spans.extend(styled_spans(StyledSpanKind::Input {
                            base_prefix: "",
                            buffer: content.as_str(),
                            hovered: chosen,
                            editing: true,
                            with_prefix: true,
                        }));
                    }
                    continue;
                }
            }
            let is_chosen_value = if let Some(F::Value(a)) = &cur_field {
                !editing && *a as usize == cur
            } else {
                false
            };
            if r.mode == crate::protocol::status::RegisterMode::Coils
                || r.mode == crate::protocol::status::RegisterMode::DiscreteInputs
            {
                let lbl = if raw_val != 0 {
                    lang().protocol.modbus.value_true.as_str()
                } else {
                    lang().protocol.modbus.value_false.as_str()
                };
                if is_chosen_value {
                    spans.extend(styled_spans(StyledSpanKind::Text {
                        text: format!("[{lbl}]").as_str(),
                        state: if is_chosen_value {
                            TextState::Chosen
                        } else {
                            TextState::Normal
                        },
                        bold: false,
                    }));
                } else {
                    spans.extend(styled_spans(StyledSpanKind::Text {
                        text: lbl.to_string().as_str(),
                        state: if chosen {
                            TextState::Chosen
                        } else {
                            TextState::Normal
                        },
                        bold: false,
                    }));
                }
            } else if is_chosen_value {
                spans.extend(styled_spans(StyledSpanKind::Text {
                    text: format!("[{raw_val:04X}]").as_str(),
                    state: if is_chosen_value {
                        TextState::Chosen
                    } else {
                        TextState::Normal
                    },
                    bold: false,
                }));
            } else {
                spans.extend(styled_spans(StyledSpanKind::Text {
                    text: format!("{raw_val:04X}").as_str(),
                    state: if chosen {
                        TextState::Chosen
                    } else {
                        TextState::Normal
                    },
                    bold: false,
                }));
            }
        }
        out.push(Line::from(spans));
        addr = line_base + 8;
    }
}

fn render_scroll_bar(
    f: &mut Frame,
    area: Rect,
    first_visible: usize,
    inner_height: usize,
    total: usize,
) {
    let bar_x = area.x + area.width.saturating_sub(1);
    let bar_y = area.y + 1;
    let bar_h = area.height.saturating_sub(2);
    let denom = (total.saturating_sub(inner_height)) as f32;
    let ratio = if denom > 0. {
        first_visible as f32 / denom
    } else {
        0.
    };
    let thumb = bar_y + ((ratio * (bar_h.saturating_sub(1) as f32)).round() as u16);
    for i in 0..bar_h {
        let ch = if bar_y + i == thumb { '█' } else { '│' };
        let p = ratatui::widgets::Paragraph::new(ch.to_string())
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, Rect::new(bar_x, bar_y + i, 1, 1));
    }
}

pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    // Reuse previous modbus page hints logic
    let mut hints: Vec<String> = Vec::new();
    if let Some(form) = app.page.subpage_form.as_ref() {
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
                    }
                    MasterEditField::Id
                    | MasterEditField::Start
                    | MasterEditField::End
                    | MasterEditField::Counter
                    | MasterEditField::Value(_) => {
                        if !matches!(field, MasterEditField::Counter) {
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
    hints
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}
