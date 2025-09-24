use anyhow::{anyhow, Result};
use strum::IntoEnumIterator;

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::{
        runtime::RuntimeCommand,
        status::{
            read_status,
            types::{
                self,
                cursor::{Cursor, ModbusDashboardCursor},
            },
            with_port_read, with_port_write, write_status,
        },
    },
    tui::utils::bus::Bus,
};

/// Ensure current cursor for ConfigPanel does not point to hidden items when
/// the selected port is not occupied by this instance. This moves the cursor
/// to a visible default (`EnablePort`) and updates `view_offset` when needed.
fn sanitize_configpanel_cursor() -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel {
            cursor,
            selected_port,
            view_offset,
            ..
        } = &mut status.page
        {
            // Determine occupancy of selected port
            let occupied = if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Some(b) = with_port_read(port, |port| {
                        matches!(port.state, types::port::PortState::OccupiedByThis { .. })
                    }) {
                        b
                    } else {
                        log::warn!("sanitize_configpanel_cursor: failed to acquire read lock for {port_name}");
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !occupied {
                match cursor {
                    types::cursor::ConfigPanelCursor::EnablePort
                    | types::cursor::ConfigPanelCursor::ProtocolMode => {
                        // allowed
                    }
                    _ => {
                        *cursor = types::cursor::ConfigPanelCursor::EnablePort;
                    }
                }

                *view_offset = cursor.view_offset();
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ConfigPanel view offset up by `amount` (saturating at 0).
pub fn handle_scroll_up(amount: usize) -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel { view_offset, .. } = &mut status.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the ConfigPanel view offset down by `amount`.
pub fn handle_scroll_down(amount: usize) -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel { view_offset, .. } = &mut status.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    // Derive selected cursor in panel
    let selected_cursor = super::components::derive_selection()?;

    // Ensure cursor does not point to hidden items on entry
    sanitize_configpanel_cursor()?;

    // Check if we're in edit mode (simplified - using global buffer)
    let in_edit = read_status(|status| Ok(!status.temporarily.input_raw_buffer.is_empty()))?;

    if in_edit {
        // Determine index choice count for selector-style fields; None for free-form string edits
        let index_choices: Option<usize> = match selected_cursor {
            types::cursor::ConfigPanelCursor::BaudRate => {
                Some(types::modbus::BaudRateSelector::iter().count())
            }
            types::cursor::ConfigPanelCursor::DataBits { .. } => {
                Some(types::modbus::DataBitsOption::iter().count())
            }
            types::cursor::ConfigPanelCursor::StopBits => {
                Some(types::modbus::StopBitsOption::iter().count())
            }
            types::cursor::ConfigPanelCursor::Parity => {
                Some(types::modbus::ParityOption::iter().count())
            }
            _ => None,
        };

        // Delegate edit-mode key handling to the centralized handler.
        // The commit closure will be called when an Enter commit occurs with
        // an optional string payload (Some when committing a string), or None
        // when no string needs to be passed (index commit).
        crate::tui::ui::components::input_span_handler::handle_input_span(
            key,
            bus,
            index_choices,
            None,
            |maybe_string| -> Result<()> {
                // Helper: read selected port name and map to port data (if any)
                let port_name_opt = read_status(|status| {
                    if let types::Page::ConfigPanel { selected_port, .. } = status.page {
                        Ok(status.ports.order.get(selected_port).cloned())
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(port_name) = port_name_opt {
                    if let Some(port) =
                        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
                    {
                        // Handle string commit (custom input -- e.g. custom baud)
                        if let Some(s) = maybe_string {
                            match selected_cursor {
                                types::cursor::ConfigPanelCursor::BaudRate => {
                                    if let Ok(parsed) = s.trim().parse::<u32>() {
                                        // Accept reasonable range (1k .. 2_000_000)
                                        if parsed >= 1000 && parsed <= 2_000_000 {
                                            if with_port_write(&port, |port| {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    runtime.current_cfg.baud = parsed;
                                                    // request runtime reconfigure
                                                    let _ = runtime.cmd_tx.send(
                                                        RuntimeCommand::Reconfigure(
                                                            runtime.current_cfg.clone(),
                                                        ),
                                                    );
                                                    return Some(());
                                                }
                                                None
                                            })
                                            .is_none()
                                            {
                                                log::warn!("failed to apply custom baud: failed to acquire write lock");
                                            }
                                        } else {
                                            log::warn!(
                                                "custom baud out of allowed range: {}",
                                                parsed
                                            );
                                        }
                                    } else {
                                        log::warn!("failed to parse custom baud: {}", s);
                                    }
                                }
                                _ => {
                                    // For now, only BaudRate uses free-form string commits in this panel
                                }
                            }

                            // Clear buffer after string commit
                            write_status(|status| {
                                status.temporarily.input_raw_buffer.clear();
                                Ok(())
                            })?;
                        } else {
                            // Index commit: read index from global buffer and apply mapping
                            let idx = read_status(|status| {
                                Ok(status.temporarily.input_raw_buffer.clone())
                            })?;
                            match idx {
                                types::ui::InputRawBuffer::Index(i) => {
                                    match selected_cursor {
                                        types::cursor::ConfigPanelCursor::BaudRate => {
                                            // map index -> preset or Custom
                                            let sel =
                                                types::modbus::BaudRateSelector::from_index(i);
                                            if let types::modbus::BaudRateSelector::Custom {
                                                ..
                                            } = sel
                                            {
                                                // Switch to string editing mode with current runtime baud populated
                                                let runtime_baud = with_port_read(&port, |port| {
                                                    if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
                                                        runtime.current_cfg.baud
                                                    } else {
                                                        types::modbus::BaudRateSelector::B9600.as_u32()
                                                    }
                                                })
                                                .unwrap_or(types::modbus::BaudRateSelector::B9600.as_u32());

                                                write_status(|status| {
                                                    status
                                                        .temporarily
                                                        .input_raw_buffer
                                                        .set_string_and_place_cursor_at_end(
                                                            runtime_baud.to_string(),
                                                        );
                                                    Ok(())
                                                })?;
                                                // Do not clear buffer: allow user to edit the populated string
                                                return Ok(());
                                            } else {
                                                let val = sel.as_u32();
                                                if with_port_write(&port, |port| {
                                                    if let types::port::PortState::OccupiedByThis { runtime, .. } = &mut port.state {
                                                        runtime.current_cfg.baud = val;
                                                        let _ = runtime.cmd_tx.send(RuntimeCommand::Reconfigure(runtime.current_cfg.clone()));
                                                        return Some(());
                                                    }
                                                    None
                                                })
                                                .is_none()
                                                {
                                                    log::warn!("failed to apply baud preset: failed to acquire write lock");
                                                }
                                            }
                                        }
                                        types::cursor::ConfigPanelCursor::DataBits { .. } => {
                                            let val = types::modbus::DataBitsOption::from_index(i)
                                                .as_u8();
                                            if with_port_write(&port, |port| {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    runtime.current_cfg.data_bits = val;
                                                    let _ = runtime.cmd_tx.send(
                                                        RuntimeCommand::Reconfigure(
                                                            runtime.current_cfg.clone(),
                                                        ),
                                                    );
                                                    return Some(());
                                                }
                                                None
                                            })
                                            .is_none()
                                            {
                                                log::warn!("failed to apply data bits: failed to acquire write lock");
                                            }
                                        }
                                        types::cursor::ConfigPanelCursor::StopBits => {
                                            if with_port_write(&port, |port| {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    runtime.current_cfg.stop_bits =
                                                        types::modbus::StopBitsOption::from_index(
                                                            i,
                                                        )
                                                        .as_u8();
                                                    let _ = runtime.cmd_tx.send(
                                                        RuntimeCommand::Reconfigure(
                                                            runtime.current_cfg.clone(),
                                                        ),
                                                    );
                                                    return Some(());
                                                }
                                                None
                                            })
                                            .is_none()
                                            {
                                                log::warn!("failed to apply stop bits: failed to acquire write lock");
                                            }
                                        }
                                        types::cursor::ConfigPanelCursor::Parity => {
                                            let parity = match i {
                                                0 => serialport::Parity::None,
                                                1 => serialport::Parity::Odd,
                                                _ => serialport::Parity::Even,
                                            };
                                            if with_port_write(&port, |port| {
                                                if let types::port::PortState::OccupiedByThis {
                                                    runtime,
                                                    ..
                                                } = &mut port.state
                                                {
                                                    runtime.current_cfg.parity = parity;
                                                    let _ = runtime.cmd_tx.send(
                                                        RuntimeCommand::Reconfigure(
                                                            runtime.current_cfg.clone(),
                                                        ),
                                                    );
                                                    return Some(());
                                                }
                                                None
                                            })
                                            .is_none()
                                            {
                                                log::warn!("failed to apply parity: failed to acquire write lock");
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }

                            // Clear buffer after index commit (if we didn't early-return above)
                            write_status(|status| {
                                status.temporarily.input_raw_buffer.clear();
                                Ok(())
                            })?;
                        }
                    }
                }

                // Trigger UI refresh
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;

                Ok(())
            },
        )?;
        Ok(())
    } else {
        // Not in edit mode: handle navigation and actions
        match key.code {
            KeyCode::PageUp => {
                // Scroll up
                handle_scroll_up(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::PageDown => {
                // Scroll down
                handle_scroll_down(5)?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                // Navigate between fields using cursor system

                write_status(|status| {
                    if let types::Page::ConfigPanel {
                        cursor,
                        view_offset,
                        ..
                    } = &mut status.page
                    {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                *cursor = cursor.prev();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                *cursor = cursor.next();
                            }
                            _ => {}
                        }
                        // Recompute view offset using the cursor's index mapping
                        *view_offset = cursor.view_offset();
                    }
                    Ok(())
                })?;

                // After moving cursor, sanitize again to ensure we didn't land on a hidden item
                sanitize_configpanel_cursor()?;

                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                // No-op in navigation mode: left/right only switches options when
                // in editing mode (after Enter). This prevents accidental parity
                // changes without explicit edit entry.
                Ok(())
            }
            KeyCode::Enter => {
                // Handle Enter key for different cursor positions
                match selected_cursor {
                    types::cursor::ConfigPanelCursor::EnablePort => {
                        // Toggle port enable/disable: send a ToggleRuntime message to core
                        // Determine selected port name and request toggle
                        if let Some(port_name) = read_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = status.page {
                                Ok(status.ports.order.get(selected_port).cloned())
                            } else {
                                Ok(None)
                            }
                        })? {
                            bus.ui_tx
                                .send(crate::tui::utils::bus::UiToCore::ToggleRuntime(port_name))
                                .map_err(|err| anyhow!(err))?;
                        }
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::ProtocolConfig => {
                        // Navigate to modbus panel
                        write_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                                status.page = types::Page::ModbusDashboard {
                                    selected_port: *selected_port,
                                    view_offset: 0,
                                    cursor: ModbusDashboardCursor::AddLine,
                                };
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::ViewCommunicationLog => {
                        // Navigate to log panel
                        write_status(|status| {
                            if let types::Page::ConfigPanel { selected_port, .. } = &status.page {
                                status.page = types::Page::LogPanel {
                                    selected_port: *selected_port,
                                    input_mode: types::ui::InputMode::Ascii,
                                    view_offset: 0,
                                };
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                        Ok(())
                    }
                    types::cursor::ConfigPanelCursor::BaudRate
                    | types::cursor::ConfigPanelCursor::DataBits { .. }
                    | types::cursor::ConfigPanelCursor::StopBits
                    | types::cursor::ConfigPanelCursor::Parity => {
                        // Enter edit mode: initialize buffer with current value
                        // Before entering edit mode, clear any existing buffer to ensure a fresh start
                        write_status(|status| {
                            status.temporarily.input_raw_buffer.clear();
                            Ok(())
                        })?;

                        write_status(|status| {
                            if let types::Page::ConfigPanel {
                                selected_port,
                                cursor,
                                ..
                            } = &status.page
                            {
                                if let Some(port_name) = status.ports.order.get(*selected_port) {
                                    if let Some(port) = status.ports.map.get(port_name) {
                                        match cursor {
                                            types::cursor::ConfigPanelCursor::BaudRate => {
                                                // Selector-first: initialize an index buffer pointing
                                                // to the matching preset if any, otherwise point to Custom
                                                let index = with_port_read(port, |port| match &port.state {
                                                                            types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                                                crate::protocol::status::types::modbus::
                                                                                    BaudRateSelector::from_u32(
                                                                                        runtime.current_cfg.baud,
                                                                                    )
                                                                                    .to_index()
                                                                            }
                                                                            _ => crate::protocol::status::types::modbus::
                                                                                BaudRateSelector::B9600
                                                                                .to_index(),
                                                                        })
                                                                        .unwrap_or_default();

                                                status.temporarily.input_raw_buffer =
                                                    types::ui::InputRawBuffer::Index(index);
                                            }
                                            types::cursor::ConfigPanelCursor::DataBits {
                                                ..
                                            } => {
                                                // initialize index buffer using normalized enum
                                                let index = with_port_read(port, |port| match &port.state {
                                                    types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                        crate::protocol::status::types::modbus::
                                                            DataBitsOption::from_u8(runtime.current_cfg.data_bits)
                                                            .to_index()
                                                    }
                                                    _ => crate::protocol::status::types::modbus::DataBitsOption::Eight.to_index(),
                                                })
                                                .unwrap_or(crate::protocol::status::types::modbus::DataBitsOption::Eight.to_index());

                                                status.temporarily.input_raw_buffer =
                                                    types::ui::InputRawBuffer::Index(index);
                                            }
                                            types::cursor::ConfigPanelCursor::Parity => {
                                                // initialize index buffer for parity
                                                let index =
                                                    with_port_read(port, |port| {
                                                        match &port
                                                    .state
                                                {
                                                    types::port::PortState::OccupiedByThis {
                                                        runtime,
                                                        ..
                                                    } => match runtime.current_cfg.parity {
                                                        serialport::Parity::None => 0usize,
                                                        serialport::Parity::Odd => 1usize,
                                                        serialport::Parity::Even => 2usize,
                                                    },
                                                    _ => 0usize,
                                                }
                                                    })
                                                    .unwrap_or(0usize);

                                                status.temporarily.input_raw_buffer =
                                                    types::ui::InputRawBuffer::Index(index);
                                            }
                                            types::cursor::ConfigPanelCursor::StopBits => {
                                                let index = with_port_read(port, |port| match &port.state {
                                                    types::port::PortState::OccupiedByThis { runtime, .. } => {
                                                        crate::protocol::status::types::modbus::
                                                            StopBitsOption::from_u8(runtime.current_cfg.stop_bits)
                                                            .to_index()
                                                    }
                                                    _ => crate::protocol::status::types::modbus::StopBitsOption::One.to_index(),
                                                })
                                                .unwrap_or(crate::protocol::status::types::modbus::StopBitsOption::One.to_index());

                                                status.temporarily.input_raw_buffer =
                                                    types::ui::InputRawBuffer::Index(index);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })?;
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|err| anyhow!(err))?;
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
            KeyCode::Esc => {
                // Return to entry page
                let cursor = read_status(|status| {
                    if let types::Page::ConfigPanel { selected_port, .. } = status.page {
                        Ok(Some(types::cursor::EntryCursor::Com {
                            index: selected_port,
                        }))
                    } else {
                        Ok(None)
                    }
                })?;

                write_status(|status| {
                    status.page = types::Page::Entry { cursor };
                    Ok(())
                })?;
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
