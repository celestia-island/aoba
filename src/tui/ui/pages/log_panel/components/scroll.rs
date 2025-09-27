use crate::protocol::status::{types, write_status};

/// Scroll the LogPanel view offset up by `amount` items (saturating at 0).
pub fn handle_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel {
            view_offset,
            selected_port,
            ..
        } = &mut status.page
        {
            // compute max index based on number of log items for selected port
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Ok(port_data) = port.read() {
                        let max_index = if port_data.logs.is_empty() {
                            0
                        } else {
                            port_data.logs.len().saturating_sub(1)
                        };
                        if *view_offset > 0 {
                            let new = view_offset.saturating_sub(amount);
                            *view_offset = std::cmp::min(new, max_index);
                        } else {
                            *view_offset = 0;
                        }
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the LogPanel view offset down by `amount` items.
pub fn handle_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel {
            view_offset,
            selected_port,
            ..
        } = &mut status.page
        {
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Ok(port_data) = port.read() {
                        let max_index = if port_data.logs.is_empty() {
                            0
                        } else {
                            port_data.logs.len().saturating_sub(1)
                        };
                        let new = view_offset.saturating_add(amount);
                        *view_offset = std::cmp::min(new, max_index);
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}
