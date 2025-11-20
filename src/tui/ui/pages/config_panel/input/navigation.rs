use anyhow::Result;

use crate::tui::{
    status as types,
    status::{cursor::Cursor, write_status},
};

/// Ensure current cursor for ConfigPanel does not point to hidden items when
/// the selected port is not occupied by this instance or when the port is virtual.
/// This moves the cursor to a visible default and updates `view_offset` when needed.
pub fn sanitize_configpanel_cursor() -> Result<()> {
    write_status(|status| {
        if let crate::tui::status::Page::ConfigPanel {
            cursor,
            selected_port,
            view_offset,
            ..
        } = &mut status.page
        {
            let (occupied, is_virtual) = if let Some(port_name) = status.ports.order.get(*selected_port) {
                if let Some(port) = status.ports.map.get(port_name) {
                    (port.state.is_occupied_by_this(), port.port_type.is_virtual())
                } else {
                    (false, false)
                }
            } else {
                (false, false)
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
            } else if is_virtual {
                // For virtual ports, skip serial config fields (BaudRate, DataBits, Parity, StopBits)
                // If cursor lands on any of these, move to the next valid field
                match cursor {
                    types::cursor::ConfigPanelCursor::BaudRate
                    | types::cursor::ConfigPanelCursor::DataBits { .. }
                    | types::cursor::ConfigPanelCursor::Parity
                    | types::cursor::ConfigPanelCursor::StopBits => {
                        // Move to ViewCommunicationLog (last item in first group)
                        *cursor = types::cursor::ConfigPanelCursor::ViewCommunicationLog;
                        *view_offset = cursor.view_offset();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}
