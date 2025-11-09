use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{tui::status as types, tui::status::cursor};

// trait for cursor helper methods (view_offset, prev, next)
use crate::tui::status::cursor::Cursor;
use crate::tui::status::{read_status, write_status, Page};
use crate::tui::ui::pages::entry::{calculate_special_items_offset, CONSERVATIVE_VIEWPORT_HEIGHT};
use crate::tui::utils::bus::Bus;

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => {
            // Quit the application
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Quit)
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
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            handle_move_prev(read_status(|status| {
                if let crate::tui::status::Page::Entry { cursor, .. } = &status.page {
                    Ok(cursor.unwrap_or(cursor::EntryCursor::Refresh))
                } else {
                    Ok(cursor::EntryCursor::Refresh)
                }
            })?)?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let cursor_opt = read_status(|status| {
                if let crate::tui::status::Page::Entry { cursor, .. } = &status.page {
                    Ok(*cursor)
                } else {
                    Ok(None)
                }
            })?;

            if cursor_opt.is_none() {
                if read_status(|status| Ok(status.ports.map.len()))? >= 2 {
                    let new_cursor = types::cursor::EntryCursor::Com { index: 1 };
                    let offset = 1;
                    write_status(|status| {
                        status.page = Page::Entry {
                            cursor: Some(new_cursor),
                            view_offset: offset,
                        };
                        Ok(())
                    })?;
                } else {
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
                }
            } else {
                handle_move_next(cursor_opt.unwrap_or(types::cursor::EntryCursor::Refresh))?;
            }

            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
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
                Some(types::cursor::EntryCursor::Com { index }) => write_status(|status| {
                    status.page = Page::ConfigPanel {
                        selected_port: index,
                        view_offset: 0,
                        cursor: types::cursor::ConfigPanelCursor::EnablePort,
                    };
                    Ok(())
                })?,
                Some(types::cursor::EntryCursor::Refresh) => {
                    bus.ui_tx
                        .send(crate::tui::utils::bus::UiToCore::RescanPorts)
                        .map_err(|err| anyhow!(err))?;
                }
                Some(types::cursor::EntryCursor::CreateVirtual) => {
                    // Virtual port creation feature
                    // This would allow users to manually specify a port path to add
                    // For now, log the action as this feature requires:
                    // 1. Input dialog for port path
                    // 2. Validation of the port path
                    // 3. Platform-specific virtual port handling
                    log::info!(
                        "Virtual port creation requested - feature not yet fully implemented"
                    );

                    // Show a message in the error state to inform the user
                    write_status(|status| {
                        status.temporarily.error = Some(crate::tui::status::ErrorInfo {
                            message: "Virtual port creation is not yet implemented. Use system tools to create virtual ports.".to_string(),
                            timestamp: chrono::Local::now(),
                        });
                        Ok(())
                    })?;
                }
                Some(types::cursor::EntryCursor::About) => write_status(|status| {
                    status.page = Page::About { view_offset: 0 };
                    Ok(())
                })?,
                None => unreachable!(
                    "Entry cursor should have been initialized before reaching this point"
                ),
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
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
        }
        _ => {}
    }
    Ok(())
}

pub fn handle_move_prev(cursor: cursor::EntryCursor) -> Result<()> {
    match cursor {
        cursor::EntryCursor::Com { index } => {
            let prev = index.saturating_sub(1);
            let new_cursor = types::cursor::EntryCursor::Com { index: prev };
            let offset = prev; // For Com cursor, offset is just the index
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::Refresh => {
            let prev = read_status(|status| Ok(status.ports.map.len().saturating_sub(1)))?;
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
            } else {
                let new_cursor = types::cursor::EntryCursor::Com { index: prev };
                let offset = prev;
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: offset,
                    };
                    Ok(())
                })?;
            }
        }
        cursor::EntryCursor::CreateVirtual => {
            let new_cursor = types::cursor::EntryCursor::Refresh;
            let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
            let offset = calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::About => {
            let new_cursor = types::cursor::EntryCursor::CreateVirtual;
            let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
            let offset = calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
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
            let next = index.saturating_add(1);
            if next >= read_status(|status| Ok(status.ports.map.len()))? {
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
            } else {
                let new_cursor = types::cursor::EntryCursor::Com { index: next };
                let offset = next;
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(new_cursor),
                        view_offset: offset,
                    };
                    Ok(())
                })?;
            }
        }
        cursor::EntryCursor::Refresh => {
            let new_cursor = types::cursor::EntryCursor::CreateVirtual;
            let ports_count = read_status(|status| Ok(status.ports.order.len()))?;
            let offset = calculate_special_items_offset(ports_count, CONSERVATIVE_VIEWPORT_HEIGHT);
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(new_cursor),
                    view_offset: offset,
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::CreateVirtual => {
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
        }
        cursor::EntryCursor::About => {}
    }

    Ok(())
}
