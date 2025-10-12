use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    protocol::status::{
        read_status,
        types::{self, cursor, Page},
        write_status,
    },
    tui::{
        ui::pages::entry::{calculate_special_items_offset, CONSERVATIVE_VIEWPORT_HEIGHT},
        utils::bus::Bus,
    },
};

pub fn handle_input(key: KeyEvent, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => {
            // Quit the application
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Quit)
                .map_err(|err| anyhow!(err))?;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            handle_move_prev(read_status(|status| {
                if let types::Page::Entry { cursor, .. } = &status.page {
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
                if let types::Page::Entry { cursor, .. } = &status.page {
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
                if let types::Page::Entry { cursor, .. } = &status.page {
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
                        .send(crate::tui::utils::bus::UiToCore::Refresh)
                        .map_err(|err| anyhow!(err))?;
                }
                Some(types::cursor::EntryCursor::CreateVirtual) => {
                    // TODO: implement virtual port creation
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
                status.page = types::Page::Entry {
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
