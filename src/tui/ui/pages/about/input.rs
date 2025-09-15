use std::sync::{Arc, RwLock};

use crossterm::event::KeyCode as KC;

use crate::{
    protocol::status::types::{self, Status},
    tui::utils::bus::Bus,
};

use anyhow::{anyhow, Result};

/// Handle input for about page. Sends navigation commands via UiToCore.
pub fn handle_input(
    key: crossterm::event::KeyEvent,
    _app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
    _snap: &types::ui::AboutStatus,
) -> Result<bool> {
    // Build the full lines snapshot to determine bounds for scrolling.
    let mut full_lines: Vec<ratatui::text::Line> = Vec::new();
    let h = crate::tui::ui::pages::about::render::init_about_cache();
    if let Ok(g) = h.lock() {
        full_lines = crate::tui::ui::pages::about::render::render_about_details(g.clone());
        if let Some(e) = crate::tui::ui::pages::about::render::about_cache_error(&h) {
            full_lines.push(ratatui::text::Line::from(format!("Note: {e}")));
        }
    }

    let total = full_lines.len();

    match key.code {
        KC::Up | KC::Char('k') => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
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
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::PageUp => {
            let page = 10usize;
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_sub(page);
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::PageDown => {
            let page = 10usize;
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(page);
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::Home => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = 0;
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::End => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = total.saturating_sub(1);
                }
                Ok(())
            });
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(true)
        }
        KC::Esc => {
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

/// Handle mouse events for About page (scroll wheel). Return true when consumed.
pub fn handle_mouse(
    me: crossterm::event::MouseEvent,
    _app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
    _snap: &types::ui::AboutStatus,
) -> bool {
    use crossterm::event::MouseEventKind as MEK;

    match me.kind {
        MEK::ScrollUp => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
                    }
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        MEK::ScrollDown => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false,
    }
}
