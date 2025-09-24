use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{
            self,
            cursor::Cursor,
            modbus::{ModbusConnectionMode, ModbusRegisterItem, RegisterMode},
        },
        with_port_write, write_status,
    },
    tui::{
        ui::components::input_span_handler,
        utils::bus::{Bus, UiToCore},
    },
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    let editing = read_status(|status| {
        Ok(!matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        ))
    })?;

    if editing {
        handle_editing_input(key, bus)?;
    } else {
        handle_navigation_input(key, bus)?;
    }
    Ok(())
}

fn handle_navigation_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            let new_cursor = current_cursor.prev();
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;
            let new_cursor = current_cursor.next();
            let new_offset = new_cursor.view_offset();
            write_status(|status| {
                if let types::Page::ModbusDashboard {
                    cursor,
                    view_offset,
                    ..
                } = &mut status.page
                {
                    *cursor = new_cursor;
                    *view_offset = new_offset;
                }
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Enter => {
            // Special handling: when cursor is on a Register and not editing,
            // allow immediate toggle for Coils/DiscreteInputs or start string
            // editing for numeric registers by pre-filling buffer with current value.
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            if let types::cursor::ModbusDashboardCursor::Register {
                slave_index,
                register_index,
            } = current_cursor
            {
                // locate the item and inspect its mode and value
                let port_name_opt = read_status(|status| {
                    if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                        Ok(status.ports.order.get(*selected_port).cloned())
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(port_name) = port_name_opt {
                    // To avoid deadlocks: do all reads under port read lock, collect
                    // the intended action, then perform writes after dropping the
                    // read guard.
                    enum PendingAction {
                        Toggle { new_value: u16 },
                        Prefill { value: u16 },
                        None,
                    }

                    let mut action = PendingAction::None;

                    if let Some(port_entry) =
                        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                    {
                        if let Ok(port_data_guard) = port_entry.read() {
                            let types::port::PortConfig::Modbus { masters, slaves } =
                                &port_data_guard.config;
                            let mut all_items = masters.clone();
                            all_items.extend(slaves.clone());
                            if let Some(item) = all_items.get(slave_index) {
                                match item.register_mode {
                                    types::modbus::RegisterMode::Coils
                                    | types::modbus::RegisterMode::DiscreteInputs => {
                                        let current_value =
                                            item.values.get(register_index).copied().unwrap_or(0);
                                        let new_value = if current_value == 0 { 1 } else { 0 };
                                        action = PendingAction::Toggle { new_value };
                                    }
                                    _ => {
                                        let current_value =
                                            item.values.get(register_index).copied().unwrap_or(0);
                                        let _hex = format!("0x{:04X}", current_value);
                                        let _bytes = _hex.clone().into_bytes();
                                        let _offset = _hex.chars().count() as isize;
                                        action = PendingAction::Prefill {
                                            value: current_value,
                                        };
                                    }
                                }
                            }
                            // port_data_guard dropped here when out of scope
                        }
                    }

                    // Execute action without holding the port read lock
                    match action {
                        PendingAction::Toggle { new_value } => {
                            if let Some(port) =
                                read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                            {
                                if with_port_write(&port, |port| {
                                    let types::port::PortConfig::Modbus { masters, slaves } =
                                        &mut port.config;
                                    let target = if slave_index < masters.len() {
                                        &mut masters[slave_index]
                                    } else {
                                        &mut slaves[slave_index - masters.len()]
                                    };
                                    if register_index < target.values.len() {
                                        target.values[register_index] = new_value;
                                    } else if register_index == target.values.len() {
                                        target.values.push(new_value);
                                    } else {
                                        while target.values.len() < register_index {
                                            target.values.push(0);
                                        }
                                        target.values.push(new_value);
                                    }
                                    Some(())
                                })
                                .is_none()
                                {
                                    log::warn!("failed to write toggle for {port_name}");
                                }
                            }
                            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                        }
                        PendingAction::Prefill { value } => {
                            write_status(|status| {
                                let hex = format!("0x{:04X}", value);
                                let bytes = hex.clone().into_bytes();
                                let offset = hex.chars().count() as isize;
                                status.temporarily.input_raw_buffer =
                                    types::ui::InputRawBuffer::String { bytes, offset };
                                Ok(())
                            })?;
                            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                        }
                        PendingAction::None => {}
                    }
                }
            } else {
                handle_enter_action(bus)?;
            }
            Ok(())
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
            // Only handle left/right when current cursor is a Register and not editing
            let current_cursor = read_status(|status| {
                if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(types::cursor::ModbusDashboardCursor::AddLine)
                }
            })?;

            if let types::cursor::ModbusDashboardCursor::Register {
                slave_index,
                register_index,
            } = current_cursor
            {
                // determine item and its total regs, allow left/right within [0, regs-1]
                let port_name_opt = read_status(|status| {
                    if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                        Ok(status.ports.order.get(*selected_port).cloned())
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(port_name) = port_name_opt {
                    if let Some(port_entry) =
                        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                    {
                        if let Ok(port_data_guard) = port_entry.read() {
                            let types::port::PortConfig::Modbus { masters, slaves } =
                                &port_data_guard.config;
                            let mut all_items = masters.clone();
                            all_items.extend(slaves.clone());
                            if let Some(item) = all_items.get(slave_index) {
                                let regs = item.register_length as usize;
                                let mut new_reg = register_index;
                                match key.code {
                                    KeyCode::Left | KeyCode::Char('h') => {
                                        if register_index > 0 {
                                            new_reg = register_index - 1;
                                        }
                                    }
                                    KeyCode::Right | KeyCode::Char('l') => {
                                        if register_index + 1 < regs {
                                            new_reg = register_index + 1;
                                        }
                                    }
                                    _ => {}
                                }

                                if new_reg != register_index {
                                    let new_cursor =
                                        types::cursor::ModbusDashboardCursor::Register {
                                            slave_index,
                                            register_index: new_reg,
                                        };
                                    let new_offset = new_cursor.view_offset();
                                    write_status(|status| {
                                        if let types::Page::ModbusDashboard {
                                            cursor,
                                            view_offset,
                                            ..
                                        } = &mut status.page
                                        {
                                            *cursor = new_cursor;
                                            *view_offset = new_offset;
                                        }
                                        Ok(())
                                    })?;
                                    bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        }
        KeyCode::Esc => {
            handle_leave_page(bus)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_enter_action(bus: &Bus) -> Result<()> {
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match current_cursor {
        types::cursor::ModbusDashboardCursor::AddLine => {
            create_new_modbus_entry()?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
            let mut sel_index: usize = 0;
            let port_name_opt = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    Ok(status.ports.order.get(*selected_port).cloned())
                } else {
                    Ok(None)
                }
            })?;

            if let Some(port_name) = port_name_opt {
                if let Some(port_entry) =
                    read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                {
                    if let Ok(port_data_guard) = port_entry.read() {
                        let types::port::PortConfig::Modbus { masters, slaves } =
                            &port_data_guard.config;
                        if let Some(item) = masters
                            .get(index)
                            .or_else(|| slaves.get(index.saturating_sub(masters.len())))
                        {
                            sel_index = item.connection_mode as usize;
                        }
                    }
                }
            }

            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(sel_index);
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
            let mut sel_index: usize = 2;
            let port_name_opt = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    Ok(status.ports.order.get(*selected_port).cloned())
                } else {
                    Ok(None)
                }
            })?;

            if let Some(port_name) = port_name_opt {
                if let Some(port_entry) =
                    read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                {
                    if let Ok(port_data_guard) = port_entry.read() {
                        let types::port::PortConfig::Modbus { masters, slaves } =
                            &port_data_guard.config;
                        if let Some(item) = masters
                            .get(index)
                            .or_else(|| slaves.get(index.saturating_sub(masters.len())))
                        {
                            // RegisterMode's discriminant is 1..4; map to 0..3
                            sel_index = (item.register_mode as u8 - 1u8) as usize;
                        }
                    }
                }
            }

            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::Index(sel_index);
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. } => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::String {
                    bytes: Vec::new(),
                    offset: 0,
                };
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
        types::cursor::ModbusDashboardCursor::Register {
            slave_index,
            register_index,
        } => {
            // When entering register edit from generic Enter, prefill buffer with current value
            let port_name_opt = read_status(|status| {
                if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
                    Ok(status.ports.order.get(*selected_port).cloned())
                } else {
                    Ok(None)
                }
            })?;

            if let Some(port_name) = port_name_opt {
                if let Some(port_entry) =
                    read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                {
                    if let Ok(port_data_guard) = port_entry.read() {
                        let types::port::PortConfig::Modbus { masters, slaves } =
                            &port_data_guard.config;
                        let mut all_items = masters.clone();
                        all_items.extend(slaves.clone());
                        if let Some(item) = all_items.get(slave_index) {
                            let current_value =
                                item.values.get(register_index).copied().unwrap_or(0);
                            let hex = format!("0x{:04X}", current_value);
                            write_status(|status| {
                                let bytes = hex.clone().into_bytes();
                                let offset = hex.chars().count() as isize;
                                status.temporarily.input_raw_buffer =
                                    types::ui::InputRawBuffer::String { bytes, offset };
                                Ok(())
                            })?;
                        }
                    }
                }
            }

            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
        }
    }

    Ok(())
}

fn handle_editing_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    let current_cursor = read_status(|status| {
        if let types::Page::ModbusDashboard { cursor, .. } = &status.page {
            Ok(*cursor)
        } else {
            Ok(types::cursor::ModbusDashboardCursor::AddLine)
        }
    })?;

    match key.code {
        KeyCode::Enter => {
            let buf = read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))?;
            match buf {
                types::ui::InputRawBuffer::String { bytes, .. } => {
                    if let Ok(s) = std::str::from_utf8(&bytes) {
                        commit_text_edit(current_cursor, s.trim().to_string())?;
                    } else {
                        commit_text_edit(current_cursor, String::new())?;
                    }
                }
                types::ui::InputRawBuffer::Index(_) => {
                    commit_selector_edit(current_cursor)?;
                }
                _ => {}
            }
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            return Ok(());
        }
        KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
                Ok(())
            })?;
            bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
            return Ok(());
        }
        _ => {}
    }

    match current_cursor {
        types::cursor::ModbusDashboardCursor::ModbusMode { .. } => {
            input_span_handler::handle_input_span(
                key,
                bus,
                Some(2),
                None,
                |_| true,
                |opt| {
                    if opt.is_none() {
                        commit_selector_edit(current_cursor)?;
                    }
                    Ok(())
                },
            )
        }
        types::cursor::ModbusDashboardCursor::RegisterMode { .. } => {
            input_span_handler::handle_input_span(
                key,
                bus,
                Some(4),
                None,
                |_| true,
                |opt| {
                    if opt.is_none() {
                        commit_selector_edit(current_cursor)?;
                    }
                    Ok(())
                },
            )
        }
        types::cursor::ModbusDashboardCursor::StationId { .. }
        | types::cursor::ModbusDashboardCursor::RegisterStartAddress { .. }
        | types::cursor::ModbusDashboardCursor::RegisterLength { .. } => {
            // Allow hexadecimal characters (0-9, a-f, A-F) and optional signs
            // Filtering is permissive; parsing/validation occurs on commit.
            // For StationId we restrict significant hex digits to 2 (max 255)
            input_span_handler::handle_input_span(
                key,
                bus,
                None,
                Some(2),
                |c: char| c.is_ascii_hexdigit() || c == '-' || c == '+' || c == 'x' || c == 'X',
                |opt| {
                    if let Some(s) = opt {
                        commit_text_edit(current_cursor, s)?;
                    } else {
                        commit_text_edit(current_cursor, String::new())?;
                    }
                    Ok(())
                },
            )
        }
        types::cursor::ModbusDashboardCursor::Register { .. } => {
            input_span_handler::handle_input_span(
                key,
                bus,
                None,
                None,
                |c: char| c.is_ascii_hexdigit() || c == '-',
                |opt| {
                    if let Some(s) = opt {
                        commit_text_edit(current_cursor, s)?;
                    } else {
                        commit_text_edit(current_cursor, String::new())?;
                    }
                    Ok(())
                },
            )
        }
        _ => Ok(()),
    }
}

fn commit_selector_edit(cursor: types::cursor::ModbusDashboardCursor) -> Result<()> {
    let selected_index = read_status(|status| {
        if let types::ui::InputRawBuffer::Index(i) = status.temporarily.input_raw_buffer {
            Ok(i)
        } else {
            Ok(0)
        }
    })?;
    let port_name_opt = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status.ports.order.get(*selected_port).cloned())
        } else {
            Ok(None)
        }
    })?;

    if let Some(port_name) = port_name_opt {
        if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            if with_port_write(&port, |port| {
                let types::port::PortConfig::Modbus { masters, slaves } = &mut port.config;
                match cursor {
                    types::cursor::ModbusDashboardCursor::ModbusMode { index } => {
                        if index < masters.len() {
                            if let Some(item) = masters.get_mut(index) {
                                item.connection_mode = if selected_index == 0 {
                                    ModbusConnectionMode::Master
                                } else {
                                    ModbusConnectionMode::Slave
                                };
                                return Some(());
                            }
                        } else if index - masters.len() < slaves.len() {
                            let si = index - masters.len();
                            if let Some(item) = slaves.get_mut(si) {
                                item.connection_mode = if selected_index == 0 {
                                    ModbusConnectionMode::Master
                                } else {
                                    ModbusConnectionMode::Slave
                                };
                                return Some(());
                            }
                        }
                    }
                    types::cursor::ModbusDashboardCursor::RegisterMode { index } => {
                        let target = if index < masters.len() {
                            &mut masters[index]
                        } else {
                            &mut slaves[index - masters.len()]
                        };
                        let rm = match selected_index {
                            0 => RegisterMode::Coils,
                            1 => RegisterMode::DiscreteInputs,
                            2 => RegisterMode::Holding,
                            3 => RegisterMode::Input,
                            _ => RegisterMode::Holding,
                        };
                        target.register_mode = rm;
                        return Some(());
                    }
                    _ => {}
                }
                None
            })
            .is_none()
            {
                log::warn!("commit_selector_edit: failed to acquire write lock for {port_name}");
            }
        }
    }

    write_status(|status| {
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
        Ok(())
    })?;
    Ok(())
}

fn commit_text_edit(cursor: types::cursor::ModbusDashboardCursor, value: String) -> Result<()> {
    fn parse_int(s: &str) -> Option<i64> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // Accept optional leading +/-, optional 0x prefix. If no prefix,
        // treat the value as hex by default per UI requirement.
        if let Some(rest) = s.strip_prefix("-0x") {
            i64::from_str_radix(rest, 16).ok().map(|v| -(v as i64))
        } else if let Some(rest) = s.strip_prefix("0x") {
            i64::from_str_radix(rest, 16).ok()
        } else if let Some(rest) = s.strip_prefix('-') {
            // negative without 0x prefix: treat as hex
            i64::from_str_radix(rest, 16).ok().map(|v| -(v as i64))
        } else {
            // No prefix: parse as hex
            i64::from_str_radix(s, 16).ok()
        }
    }

    let port_name_opt = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status.ports.order.get(*selected_port).cloned())
        } else {
            Ok(None)
        }
    })?;

    if let Some(port_name) = port_name_opt {
        if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            if with_port_write(&port, |port| {
                let types::port::PortConfig::Modbus { masters, slaves } = &mut port.config;
                match cursor {
                    types::cursor::ModbusDashboardCursor::StationId { index } => {
                        let item_opt = if index < masters.len() {
                            masters.get_mut(index)
                        } else {
                            slaves.get_mut(index - masters.len())
                        };
                        if let Some(item) = item_opt {
                            if let Some(parsed) = parse_int(&value) {
                                if (0..=255).contains(&parsed) {
                                    item.station_id = parsed as u8;
                                    return Some(());
                                } else {
                                    log::warn!("station id out of range: {}", parsed);
                                }
                            }
                        }
                    }
                    types::cursor::ModbusDashboardCursor::RegisterStartAddress { index } => {
                        let item_opt = if index < masters.len() {
                            masters.get_mut(index)
                        } else {
                            slaves.get_mut(index - masters.len())
                        };
                        if let Some(item) = item_opt {
                            if let Some(parsed) = parse_int(&value) {
                                if (0..=u16::MAX as i64).contains(&parsed) {
                                    item.register_address = parsed as u16;
                                    return Some(());
                                } else {
                                    log::warn!("register start address out of range: {}", parsed);
                                }
                            }
                        }
                    }
                    types::cursor::ModbusDashboardCursor::RegisterLength { index } => {
                        let item_opt = if index < masters.len() {
                            masters.get_mut(index)
                        } else {
                            slaves.get_mut(index - masters.len())
                        };
                        if let Some(item) = item_opt {
                            if let Some(parsed) = parse_int(&value) {
                                if parsed > 0 && parsed <= u16::MAX as i64 {
                                    item.register_length = parsed as u16;
                                    return Some(());
                                } else {
                                    log::warn!("register length out of range: {}", parsed);
                                }
                            }
                        }
                    }
                    types::cursor::ModbusDashboardCursor::Register {
                        slave_index,
                        register_index,
                    } => {
                        let item_opt = if slave_index < masters.len() {
                            masters.get_mut(slave_index)
                        } else {
                            slaves.get_mut(slave_index - masters.len())
                        };
                        if let Some(item) = item_opt {
                            if let Some(parsed) = parse_int(&value) {
                                if (0..=u16::MAX as i64).contains(&parsed) {
                                    let v = parsed as u16;
                                    if register_index < item.values.len() {
                                        item.values[register_index] = v;
                                    } else if register_index == item.values.len() {
                                        item.values.push(v);
                                    } else {
                                        while item.values.len() < register_index {
                                            item.values.push(0);
                                        }
                                        item.values.push(v);
                                    }
                                    return Some(());
                                } else {
                                    log::warn!("register value out of range: {}", parsed);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                None
            })
            .is_none()
            {
                log::warn!("commit_text_edit: failed to acquire write lock for {port_name}");
            }
        }
    }

    write_status(|status| {
        status.temporarily.input_raw_buffer = types::ui::InputRawBuffer::None;
        Ok(())
    })?;
    Ok(())
}

fn create_new_modbus_entry() -> Result<()> {
    let port_name_opt = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(status.ports.order.get(*selected_port).cloned())
        } else {
            Ok(None)
        }
    })?;

    if let Some(port_name) = port_name_opt {
        // Get a clone of the port Arc without taking the global write lock
        if let Some(port) = read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))? {
            if with_port_write(&port, |port| {
                let types::port::PortConfig::Modbus { masters: _, slaves } = &mut port.config;
                let item = ModbusRegisterItem {
                    connection_mode: ModbusConnectionMode::Slave,
                    station_id: 1,
                    register_mode: RegisterMode::Coils,
                    register_address: 0,
                    register_length: 8,
                    req_success: 0,
                    req_total: 0,
                    next_poll_at: std::time::Instant::now(),
                    pending_requests: Vec::new(),
                    values: Vec::new(),
                };
                slaves.push(item);
                Some(())
            })
            .is_none()
            {
                log::warn!("create_new_modbus_entry: failed to acquire write lock for {port_name}");
            }
        }
    }

    Ok(())
}

fn handle_leave_page(bus: &Bus) -> Result<()> {
    let selected_port = read_status(|status| {
        if let types::Page::ModbusDashboard { selected_port, .. } = &status.page {
            Ok(*selected_port)
        } else {
            Ok(0)
        }
    })?;
    write_status(|status| {
        status.page = types::Page::ConfigPanel {
            selected_port,
            view_offset: 0,
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
        };
        Ok(())
    })?;
    bus.ui_tx.send(UiToCore::Refresh).map_err(|e| anyhow!(e))?;
    Ok(())
}
