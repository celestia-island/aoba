use std::sync::{Arc, RwLock};

use crossterm::event::{KeyCode, MouseEventKind};

use crate::{
    protocol::status::{
        types::{self, Status},
        write_status,
    },
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
) -> Result<()> {
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
        KeyCode::Up | KeyCode::Char('k') => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
                    }
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::PageUp => {
            let page = 10usize;
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_sub(page);
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::PageDown => {
            let page = 10usize;
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(page);
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::Home => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = 0;
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        KeyCode::End | KeyCode::Esc => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = total.saturating_sub(1);
                }
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Handle mouse events for About page (scroll wheel). Return true when consumed.
pub fn handle_mouse(
    me: crossterm::event::MouseEvent,
    _bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
) -> Result<()> {
    match me.kind {
        MouseEventKind::ScrollUp => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
                    }
                }
                Ok(())
            })?;
            Ok(())
        }
        MouseEventKind::ScrollDown => {
            write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            })?;
            Ok(())
        }
        _ => Ok(()),
    }
}
