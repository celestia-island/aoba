use anyhow::{anyhow, Result};

use crossterm::event::{KeyCode, MouseEventKind};

use crate::{
    protocol::status::{
        read_status,
        types::{self, ui::EntryCursor, Page, Status},
        write_status,
    },
    tui::utils::bus::Bus,
};

pub fn handle_move_prev(app: &Status, cursor: types::ui::EntryCursor) -> Result<()> {
    match cursor {
        types::ui::EntryCursor::Com { idx } => {
            let prev = idx.saturating_sub(1);
            write_status(|s| {
                s.page = Page::Entry {
                    cursor: Some(types::ui::EntryCursor::Com { idx: prev }),
                };
                Ok(())
            })?;
        }
        types::ui::EntryCursor::Refresh => {
            let prev = app.ports.map.len() - 1;
            if app.ports.map.is_empty() {
                write_status(|s| {
                    s.page = Page::Entry {
                        cursor: Some(types::ui::EntryCursor::Refresh),
                    };
                    Ok(())
                })?;
            } else {
                write_status(|s| {
                    s.page = Page::Entry {
                        cursor: Some(types::ui::EntryCursor::Com { idx: prev }),
                    };
                    Ok(())
                })?;
            }
        }
        types::ui::EntryCursor::CreateVirtual => {
            write_status(|s| {
                s.page = Page::Entry {
                    cursor: Some(types::ui::EntryCursor::Refresh),
                };
                Ok(())
            })?;
        }
        types::ui::EntryCursor::About => {
            write_status(|s| {
                s.page = Page::Entry {
                    cursor: Some(types::ui::EntryCursor::About),
                };
                Ok(())
            })?;
        }
    }

    Ok(())
}

pub fn handle_move_next(app: &Status, cursor: types::ui::EntryCursor) -> Result<()> {
    match cursor {
        types::ui::EntryCursor::Com { idx } => {
            let next = idx.saturating_add(1);
            if next >= app.ports.map.len() {
                write_status(|s| {
                    s.page = Page::Entry {
                        cursor: Some(types::ui::EntryCursor::Refresh),
                    };
                    Ok(())
                })?;
            } else {
                write_status(|s| {
                    s.page = Page::Entry {
                        cursor: Some(types::ui::EntryCursor::Com { idx: next }),
                    };
                    Ok(())
                })?;
            }
        }
        types::ui::EntryCursor::Refresh => {
            write_status(|s| {
                s.page = Page::Entry {
                    cursor: Some(types::ui::EntryCursor::CreateVirtual),
                };
                Ok(())
            })?;
        }
        types::ui::EntryCursor::CreateVirtual => {
            write_status(|s| {
                s.page = Page::Entry {
                    cursor: Some(types::ui::EntryCursor::About),
                };
                Ok(())
            })?;
        }
        types::ui::EntryCursor::About => {}
    }

    Ok(())
}

/// Compatibility wrapper used by pages/mod.rs which expects signature:
/// fn handle_input(key: KeyEvent, app: &Status, bus: &Bus, snap: &types::ui::EntryStatus) -> bool
pub fn handle_input(key: crossterm::event::KeyEvent, app: &Status, bus: &Bus) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            // move selection up in Entry page
            handle_move_prev(
                app,
                read_status(|s| {
                    if let types::Page::Entry { cursor } = &s.page {
                        Ok(cursor.unwrap_or(EntryCursor::Refresh))
                    } else {
                        Ok(EntryCursor::Refresh)
                    }
                })?,
            )?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // move selection down
            handle_move_next(
                app,
                read_status(|s| {
                    if let types::Page::Entry { cursor } = &s.page {
                        Ok(cursor.unwrap_or(EntryCursor::Refresh))
                    } else {
                        Ok(EntryCursor::Refresh)
                    }
                })?,
            )?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Enter => {
            // Enter a page or take action depending on selection

            let cursor = read_status(|s| {
                if let types::Page::Entry { cursor } = &s.page {
                    Ok(cursor.clone())
                } else {
                    Ok(None)
                }
            })?;

            if cursor.is_none() {
                // Give a default value for cursor
                if app.ports.map.is_empty() {
                    write_status(|s| {
                        s.page = Page::Entry {
                            cursor: Some(types::ui::EntryCursor::Refresh),
                        };
                        Ok(())
                    })?;
                } else {
                    write_status(|s| {
                        s.page = Page::Entry {
                            cursor: Some(types::ui::EntryCursor::Com { idx: 0 }),
                        };
                        Ok(())
                    })?;
                }
            }

            match cursor {
                Some(types::ui::EntryCursor::Com { idx }) => write_status(|s| {
                    s.page = Page::ModbusConfig {
                        selected_port: idx,
                        edit_active: false,
                        edit_port: None,
                        edit_field_index: 0,
                        edit_field_key: None,
                        edit_buffer: String::new(),
                        edit_cursor_pos: 0,
                    };
                    Ok(())
                })?,
                Some(types::ui::EntryCursor::Refresh) => {
                    bus.ui_tx
                        .send(crate::tui::utils::bus::UiToCore::Refresh)
                        .map_err(|e| anyhow!(e))?;
                }
                Some(types::ui::EntryCursor::CreateVirtual) => {
                    // TODO: implement virtual port creation
                }
                Some(types::ui::EntryCursor::About) => write_status(|s| {
                    s.page = Page::About { view_offset: 0 };
                    Ok(())
                })?,
                None => unreachable!(""),
            }
            Ok(())
        }
        KeyCode::Esc => {
            // Escape returns to top-level entry cursor cleared (or quit handled by caller)
            let _ = crate::protocol::status::write_status(|s| {
                s.page = types::Page::Entry { cursor: None };
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle mouse events for About page (scroll wheel). Return true when consumed.
pub fn handle_mouse(me: crossterm::event::MouseEvent, _bus: &Bus) -> Result<()> {
    match me.kind {
        MouseEventKind::ScrollUp => {
            handle_move_prev(
                &read_status(|s| Ok(s.clone()))?,
                types::ui::EntryCursor::Refresh,
            )?;
            Ok(())
        }
        MouseEventKind::ScrollDown => {
            handle_move_next(
                &read_status(|s| Ok(s.clone()))?,
                types::ui::EntryCursor::Refresh,
            )?;
            Ok(())
        }
        _ => Ok(()),
    }
}
