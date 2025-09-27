use anyhow::Result;

use crate::protocol::status::{
    types::{self, cursor::Cursor},
    with_port_read, write_status,
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
                    if let Some(val) = with_port_read(port, |port| {
                        matches!(port.state, types::port::PortState::OccupiedByThis { .. })
                    }) {
                        val
                    } else {
                        log::warn!("Failed to acquire read lock for port {port_name} while sanitizing the config panel cursor");
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !occupied {
                // Port not occupied: allow movement within the first group of
                // config options but prevent entering the second group.
                // The group sizes are defined in the cursor types (first group
                // size is usually 4). If the current cursor index is inside
                // the second group, clamp it to the last item of the first
                // group and update view_offset so it stays visible.
                let first_group_size = types::cursor::CONFIG_PANEL_GROUP_SIZES
                    .first()
                    .copied()
                    .unwrap_or(4usize);

                let cur_index = cursor.to_index();
                if cur_index >= first_group_size {
                    *cursor = types::cursor::ConfigPanelCursor::from_index(first_group_size - 1);
                }

                // Ensure view offset follows the (possibly updated) cursor so
                // the selected item is visible.
                *view_offset = (*cursor).view_offset();
            }
        }
        Ok(())
    })?;
    Ok(())
}
