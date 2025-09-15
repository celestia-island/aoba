use crate::protocol::status::types::{self, Status};
use crate::tui::utils::bus::Bus;
use anyhow::{anyhow, Result};
use crossterm::event::Event;
use std::sync::{Arc, RwLock};

pub fn handle_move_next(page: &mut types::Page, max: usize) {
    match page {
        types::Page::Entry { cursor } => {
            let idx = match cursor {
                Some(types::ui::EntryCursor::Com { idx }) => *idx,
                Some(types::ui::EntryCursor::Refresh) => max.saturating_sub(1),
                Some(types::ui::EntryCursor::CreateVirtual) => max.saturating_sub(1),
                Some(types::ui::EntryCursor::About) => max.saturating_sub(1),
                None => 0usize,
            };
            let next = idx.saturating_add(1).min(max);
            *cursor = Some(types::ui::EntryCursor::Com { idx: next });
        }
        _ => {}
    }
}

pub fn handle_move_prev(page: &mut types::Page) {
    match page {
        types::Page::Entry { cursor } => {
            let idx = match cursor {
                Some(types::ui::EntryCursor::Com { idx }) => *idx,
                Some(types::ui::EntryCursor::Refresh) => 0usize,
                Some(types::ui::EntryCursor::CreateVirtual) => 0usize,
                Some(types::ui::EntryCursor::About) => 0usize,
                None => 0usize,
            };
            let prev = idx.saturating_sub(1);
            *cursor = Some(types::ui::EntryCursor::Com { idx: prev });
        }
        _ => {}
    }
}

pub fn handle_enter_page(_page: &mut types::Page) {
    // placeholder: real implementation interacts with app state
}

pub fn handle_toggle_port(_page: &mut types::Page) {
    // placeholder: real implementation interacts with app ports
}

pub fn handle_leave_page(_page: &mut types::Page) {
    // placeholder
}

pub fn handle_input(event: &Event, page: &mut types::Page) {
    use crossterm::event::{KeyCode, KeyEvent};
    match event {
        Event::Key(KeyEvent { code, .. }) => match code {
            KeyCode::Char('j') | KeyCode::Down => handle_move_next(page, 0),
            KeyCode::Char('k') | KeyCode::Up => handle_move_prev(page),
            KeyCode::Enter => handle_enter_page(page),
            KeyCode::Char('q') => handle_leave_page(page),
            _ => {}
        },
        _ => {}
    }
}

/// Compatibility wrapper used by pages/mod.rs which expects signature:
/// fn handle_input(key: KeyEvent, app: &Status, bus: &Bus, app_arc: &Arc<RwLock<Status>>, snap: &types::ui::EntryStatus) -> bool
pub fn handle_input_dispatch(
    key: crossterm::event::KeyEvent,
    app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<Status>>,
    _snap: &types::ui::EntryStatus,
) -> Result<bool> {
    use crossterm::event::KeyCode as KC;

    match key.code {
        KC::Up | KC::Char('k') => {
            // move selection up in Entry page
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::Entry { cursor } = &mut s.page {
                    if let Some(types::ui::EntryCursor::Com { idx }) = cursor {
                        if *idx > 0 {
                            *idx = idx.saturating_sub(1);
                        }
                    } else {
                        *cursor = Some(types::ui::EntryCursor::Com { idx: 0 });
                    }
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::Down | KC::Char('j') => {
            // move selection down
            let max = app.ports.order.len().saturating_add(2);
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::Entry { cursor } = &mut s.page {
                    if let Some(types::ui::EntryCursor::Com { idx }) = cursor {
                        let next = (*idx).saturating_add(1).min(max);
                        *idx = next;
                    } else {
                        *cursor = Some(types::ui::EntryCursor::Com { idx: 0 });
                    }
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::Enter => {
            // Enter a page or take action depending on selection
            let sel = super::super::derive_selection(app);
            let special_base = app.ports.order.len();
            if sel < special_base {
                // Enter into ModbusDashboard for selected port
                let _ = crate::protocol::status::write_status(app_arc, |s| {
                    s.page = types::Page::ModbusDashboard {
                        selected_port: sel,
                        cursor: 0,
                        editing_field: None,
                        input_buffer: String::new(),
                        edit_choice_index: None,
                        edit_confirmed: false,
                        master_cursor: 0,
                        master_field_selected: false,
                        master_field_editing: false,
                        master_edit_field: None,
                        master_edit_index: None,
                        master_input_buffer: String::new(),
                        poll_round_index: 0,
                        in_flight_reg_index: None,
                    };
                    Ok(())
                });
                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
                Ok(true)
            } else {
                // Handle special entries
                let rel = sel - special_base;
                match rel {
                    0 => {
                        // Refresh action: request UI core to scan/refresh
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|e| anyhow!(e))?;
                        Ok(true)
                    }
                    1 => {
                        // Manual specify - open some dialog; for now no-op
                        Ok(true)
                    }
                    2 => {
                        // About: enter About page
                        let _ = crate::protocol::status::write_status(app_arc, |s| {
                            s.page = types::Page::About { view_offset: 0 };
                            Ok(())
                        });
                        bus.ui_tx
                            .send(crate::tui::utils::bus::UiToCore::Refresh)
                            .map_err(|e| anyhow!(e))?;
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
        }
        KC::Esc => {
            // Escape returns to top-level entry cursor cleared (or quit handled by caller)
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                s.page = types::Page::Entry { cursor: None };
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
