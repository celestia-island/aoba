use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::{status as types, status::cursor};

// trait for cursor helper methods (view_offset, prev, next)
use crate::core::bus::{self, Bus};
use crate::tui::status::cursor::Cursor;
use crate::tui::status::{read_status, write_status, Page};
use crate::tui::ui::pages::entry::{calculate_special_items_offset, CONSERVATIVE_VIEWPORT_HEIGHT};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => {
            // Quit the application
            bus.ui_tx
                .send(crate::core::bus::UiToCore::Quit)
                .map_err(|err| anyhow!(err))?;
        }
        KeyCode::PageUp => {
            // Jump to first cursor position
            let ports_count = read_status(|status| Ok(status.ports.map.len()))?;
            let new_cursor = if ports_count > 0 {
                types::cursor::EntryCursor::Com { index: 0 }
            } else {
                types::cursor::EntryCursor::Refresh
            };
            let offset = new_cursor.view_offset();
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
                };
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        KeyCode::PageDown => {
            // Jump to last cursor position (About)
            let new_cursor = types::cursor::EntryCursor::About;
            let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
            let offset = calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
                };
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            handle_move_prev(read_status(|status| {
                if let crate::tui::status::Page::Entry { cursor, .. } = &status.page {
                    Ok(cursor.unwrap_or(cursor::EntryCursor::Com { index: 0 }))
                } else {
                    Ok(cursor::EntryCursor::Com { index: 0 })
                }
            })?)?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let cursor_opt = read_status(|status| {
                if let crate::tui::status::Page::Entry { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(None)
                }
            })?;

            if cursor_opt.is_none() {
                // Initialize cursor to first port
                let new_cursor = types::cursor::EntryCursor::Com { index: 0 };
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: 0,
                    };
                    Ok(())
                })?;
            } else {
                handle_move_next(cursor_opt.unwrap_or(types::cursor::EntryCursor::Com { index: 0 }))?;
            }

            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        // Support for About (A key)
        KeyCode::Char('a') | KeyCode::Char('A') => {
            write_status(|status| {
                status.page = Page::About { view_offset: 0 };
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        // Support for New (N key) - placeholder for port creation
        KeyCode::Char('n') | KeyCode::Char('N') => {
            log::info!("New port creation requested - feature not yet fully implemented");
            write_status(|status| {
                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                    message: "Port creation is not yet implemented. Use system tools or --check-port to add ports.".to_string(),
                    timestamp: chrono::Local::now(),
                });
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        // Support for Delete (D key) - placeholder for port deletion
        KeyCode::Char('d') | KeyCode::Char('D') => {
            log::info!("Port deletion requested - feature not yet fully implemented");
            write_status(|status| {
                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                    message: "Port deletion is not yet implemented. Ports are managed by the system.".to_string(),
                    timestamp: chrono::Local::now(),
                });
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        KeyCode::Enter => {
            let cursor = read_status(|status| {
                if let crate::tui::status::Page::Entry { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(None)
                }
            })?;

            let final_cursor = if cursor.is_none() {
                if read_status(|status| Ok(status.ports.map.is_empty()))? {
                    let new_cursor = types::cursor::EntryCursor::Refresh;
                    let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
                    let offset =
                        calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
                    write_status(|status| {
                        status.page = Page::Entry {
                            cursor: Some(new_cursor),
                            view_offset: offset,
                        };
                        Ok(())
                    })?;
                    Some(new_cursor)
                } else {
                    let new_cursor = types::cursor::EntryCursor::Com { index: 0 };
                    let offset = 0;
                    write_status(|status| {
                        status.page = Page::Entry {
                            cursor: Some(new_cursor),
                            view_offset: offset,
                        };
                        Ok(())
                    })?;
                    Some(new_cursor)
                }
            } else {
                cursor
            };

            match final_cursor {
                Some(types::cursor::EntryCursor::Com { index }) => {
                    // Check if port is occupied by another program before allowing access
                    let port_state = read_status(|status| {
                        Ok(status
                            .ports
                            .order
                            .get(index)
                            .and_then(|name| status.ports.map.get(name))
                            .map(|port| port.state.clone()))
                    })?;

                    if let Some(state) = port_state {
                        if state.is_occupied_by_other() {
                            // Port is occupied by another program, show error and prevent access
                            write_status(|status| {
                                status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                                    message: crate::protocol::i18n::lang()
                                        .index
                                        .port_occupied_error
                                        .clone(),
                                    timestamp: chrono::Local::now(),
                                });
                                Ok(())
                            })?;
                            log::warn!(
                                "Cannot access port at index {index}: occupied by another program"
                            );
                            return Ok(());
                        }
                    }

                    // Port is free, allow access to config panel
                    write_status(|status| {
                        status.page = Page::ConfigPanel {
                            selected_port: index,
                            view_offset: 0,
                            cursor: types::cursor::ConfigPanelCursor::EnablePort,
                        };
                        Ok(())
                    })?;
                }
                _ => {
                    // For any other cursor type, do nothing (shouldn't happen in new layout)
                    log::warn!("Unexpected cursor type on Enter in entry page");
                }
            }
        }
        KeyCode::Esc => {
            write_status(|status| {
                status.page = crate::tui::status::Page::Entry {
                    cursor: None,
                    view_offset: 0,
                };
                Ok(())
            })?;
            bus::request_refresh(&bus.ui_tx).map_err(|err| anyhow!(err))?;
        }
        _ => {}
    }
    Ok(())
}

pub fn handle_move_prev(cursor: cursor::EntryCursor) -> Result<()> {
    match cursor {
        cursor::EntryCursor::Com { index } => {
            if index > 0 {
                let prev = index - 1;
                let new_cursor = types::cursor::EntryCursor::Com { index: prev };
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: prev,
                    };
                    Ok(())
                })?;
            }
            // If already at index 0, stay there (no wrap-around in grid layout)
        }
        _ => {
            // For other cursor types, move to last port
            let last_index = read_status(|status| Ok(status.ports.order.len().saturating_sub(1)))?;
            let new_cursor = types::cursor::EntryCursor::Com { index: last_index };
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: last_index,
                };
                Ok(())
            })?;
        }
    }

    Ok(())
}

pub fn handle_move_next(cursor: cursor::EntryCursor) -> Result<()> {
    match cursor {
        cursor::EntryCursor::Com { index } => {
            let max_index = read_status(|status| Ok(status.ports.order.len().saturating_sub(1)))?;
            if index < max_index {
                let next = index + 1;
                let new_cursor = types::cursor::EntryCursor::Com { index: next };
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: next,
                    };
                    Ok(())
                })?;
            }
            // If already at last index, stay there (no wrap-around in grid layout)
        }
        _ => {
            // For other cursor types, move to first port
            let new_cursor = types::cursor::EntryCursor::Com { index: 0 };
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: 0,
                };
                Ok(())
            })?;
        }
    }

    Ok(())
}
