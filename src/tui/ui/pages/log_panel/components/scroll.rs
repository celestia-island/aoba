use crate::protocol::status::{types, write_status};

/// Scroll the LogPanel view offset up by `amount` (saturating at 0).
pub fn log_panel_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel { view_offset, .. } = &mut status.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the LogPanel view offset down by `amount`.
pub fn log_panel_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel { view_offset, .. } = &mut status.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}