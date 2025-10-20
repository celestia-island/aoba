use crate::tui::status::write_status;

/// Scroll up in the LogPanel by moving the selected_item up by `amount` items.
pub fn handle_scroll_up(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let crate::tui::status::Page::LogPanel {
            selected_item,
            selected_port,
            ..
        } = &mut status.page
        {
            // Get the log count for selected port
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    let port_data = port.read();
                    let log_count = port_data.logs.len();

                    if log_count == 0 {
                        return Ok(());
                    }

                    // Any scroll action switches to manual mode
                    match selected_item {
                        None => {
                            // Auto-follow mode: switch to manual mode at second-to-last item
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
        Ok(())
    })?;
    Ok(())
}

/// Scroll down in the LogPanel by moving the selected_item down by `amount` items.
pub fn handle_scroll_down(amount: usize) -> anyhow::Result<()> {
    write_status(|status| {
        if let crate::tui::status::Page::LogPanel {
            selected_item,
            selected_port,
            ..
        } = &mut status.page
        {
            // Get the log count for selected port
            if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    let port_data = port.read();
                    let log_count = port_data.logs.len();

                    if log_count == 0 {
                        return Ok(());
                    }

                    // Any scroll action switches to manual mode
                    match selected_item {
                        None => {
                            // Auto-follow mode: switch to manual mode at first item
                            *selected_item = Some(0);
                        }
                        Some(current_idx) => {
                            // Manual mode: move down by amount, but stay in manual mode
                            let new_idx = current_idx.saturating_add(amount);
                            *selected_item =
                                Some(std::cmp::min(new_idx, log_count.saturating_sub(1)));
                        }
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}
