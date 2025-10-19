use anyhow::Result;

use ratatui::prelude::*;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types},
    tui::ui::{
        components::boxed_paragraph::render_boxed_paragraph,
        pages::modbus_panel::components::render_modbus_status_lines,
    },
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    // Check if we're in hex editing mode for registers
    let is_hex_editing = read_status(|status| {
        let is_editing = !matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::None
        );
        let is_register = matches!(
            status.page,
            types::Page::ModbusDashboard {
                cursor: types::cursor::ModbusDashboardCursor::Register { .. },
                ..
            }
        );
        let is_string_input = matches!(
            status.temporarily.input_raw_buffer,
            types::ui::InputRawBuffer::String { .. }
        );
        Ok(is_editing && is_register && is_string_input)
    })?;

    if is_hex_editing {
        Ok(vec![
            vec![
                lang().hotkeys.hint_hex_input_mode.as_str().to_string(),
                lang().hotkeys.hint_hex_enter_save.as_str().to_string(),
            ],
            vec![lang().hotkeys.hint_hex_esc_exit.as_str().to_string()],
        ])
    } else {
        // Check if the port has modifications
        let has_modifications = read_status(|status| {
            let port_name_opt = match &status.page {
                types::Page::ModbusDashboard { selected_port, .. } => {
                    status.ports.order.get(*selected_port).cloned()
                }
                _ => None,
            };
            if let Some(port_name) = port_name_opt {
                if let Some(port_entry) = status.ports.map.get(&port_name) {
                    let port_guard = port_entry.read();
                    return Ok(port_guard.config_modified);
                }
            }
            Ok(false)
        })?;

        if has_modifications {
            // Show Ctrl+S to save and Ctrl+Esc to discard
            Ok(vec![
                vec![
                    lang().hotkeys.hint_move_vertical.as_str().to_string(),
                    lang().hotkeys.hint_master_enter_edit.as_str().to_string(),
                ],
                vec![
                    lang().hotkeys.hint_master_delete.as_str().to_string(),
                    lang().hotkeys.press_ctrl_s_save_config.as_str().to_string(),
                    lang()
                        .hotkeys
                        .press_ctrl_esc_discard_return
                        .as_str()
                        .to_string(),
                ],
            ])
        } else {
            // No modifications - just show Esc to return
            Ok(vec![
                vec![
                    lang().hotkeys.hint_move_vertical.as_str().to_string(),
                    lang().hotkeys.hint_master_enter_edit.as_str().to_string(),
                ],
                vec![
                    lang().hotkeys.hint_master_delete.as_str().to_string(),
                    lang().hotkeys.press_esc_return.as_str().to_string(),
                ],
            ])
        }
    }
}

/// Render the ModBus panel. Only reads from Status, does not mutate.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let view_offset = read_status(|status| {
        if let types::Page::ModbusDashboard { view_offset, .. } = &status.page {
            Ok(*view_offset)
        } else {
            Ok(0)
        }
    })?;

    let lines = render_modbus_status_lines()?;
    render_boxed_paragraph(frame, area, lines, view_offset, None, false, true);

    Ok(())
}
