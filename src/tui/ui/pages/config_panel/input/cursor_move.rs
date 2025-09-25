use anyhow::Result;

use crate::{
    protocol::status::{
        types::{
            self,
        },
        with_port_read, write_status,
    },
};

/// Ensure current cursor for ConfigPanel does not point to hidden items when
/// the selected port is not occupied by this instance. This moves the cursor
/// to a visible default (`EnablePort`) and updates `view_offset` when needed.
pub fn sanitize_configpanel_cursor() -> Result<()> {
    write_status(|status| {
        if let types::Page::ConfigPanel {
            cursor,
            selected_port,
            view_offset,
            ..
        } = &mut status.page
        {
            let occupied = if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    if let Some(b) = with_port_read(port, |port| {
                        matches!(port.state, types::port::PortState::OccupiedByThis { .. })
                    }) {
                        b
                    } else {
                        log::warn!("sanitize_configpanel_cursor: failed to acquire read lock for {port_name}");
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !occupied {
                // Port not occupied, limit cursor to visible items
                *cursor = types::cursor::ConfigPanelCursor::EnablePort;
                *view_offset = 0;
            }
        }
        Ok(())
    })?;
    Ok(())
}