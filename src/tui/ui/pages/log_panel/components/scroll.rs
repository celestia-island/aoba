use crate::protocol::status::{types, write_status};

/// Scroll up in the LogPanel by moving the selected_item up by `amount` items.
pub fn handle_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel {
            selected_item,
            selected_port,
            ..
        } = &mut status.page
        {
            // Get the log count for selected port
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Ok(port_data) = port.read() {
                        let log_count = port_data.logs.len();
                        
                        if log_count == 0 {
                            return Ok(());
                        }

                        match selected_item {
                            None => {
                                // Auto-follow mode: move to second-to-last item
                                *selected_item = Some(log_count.saturating_sub(2));
                            }
                            Some(current_idx) => {
                                // Manual mode: move up by amount
                                *selected_item = Some(current_idx.saturating_sub(amount));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll down in the LogPanel by moving the selected_item down by `amount` items.
pub fn handle_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let types::Page::LogPanel {
            selected_item,
            selected_port,
            ..
        } = &mut status.page
        {
            // Get the log count for selected port
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Ok(port_data) = port.read() {
                        let log_count = port_data.logs.len();
                        
                        if log_count == 0 {
                            return Ok(());
                        }

                        match selected_item {
                            None => {
                                // Auto-follow mode: stay at the last item (no change)
                                // selected_item remains None
                            }
                            Some(current_idx) => {
                                // Manual mode: move down by amount
                                let new_idx = current_idx.saturating_add(amount);
                                if new_idx >= log_count.saturating_sub(1) {
                                    // If we reach or go past the last item, return to auto-follow mode
                                    *selected_item = None;
                                } else {
                                    *selected_item = Some(new_idx);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}
