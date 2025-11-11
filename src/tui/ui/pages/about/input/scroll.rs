use anyhow::Result;

// removed unused import: protocol::status::types
use crate::tui::status::write_status;

/// Scroll the About page view offset up by `amount` (saturating at 0).
pub fn handle_scroll_up(amount: usize) -> Result<()> {
    write_status(|status| {
        if let crate::tui::status::Page::About { view_offset } = &mut status.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the About page view offset down by `amount`.
pub fn handle_scroll_down(amount: usize) -> Result<()> {
    write_status(|status| {
        if let crate::tui::status::Page::About { view_offset } = &mut status.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}
