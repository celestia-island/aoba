//! Shared helper functions for form editing to eliminate repeated code.
use crate::tui::status::types::ui::{EditingField, RegisterField};

/// Select the field being edited based on the current `cursor` position.
///
/// This helper operates on minimal pieces of state; callers should pass
/// mutable references for fields that need updating.
pub fn select_field_by_cursor(
    cursor: usize,
    editing_field: &mut Option<EditingField>,
    baud: u32,
    input_buffer: &mut String,
    edit_choice_index: &mut Option<usize>,
    edit_confirmed: &mut bool,
) {
    *editing_field = Some(match cursor {
        0 => EditingField::Loop,
        // 1 is master_passive toggle and is not editable via the editing_field; skip it
        1 => return,
        2 => EditingField::Baud,
        3 => EditingField::Parity,
        4 => EditingField::DataBits,
        5 => EditingField::StopBits,
        6 => EditingField::GlobalInterval,
        7 => EditingField::GlobalTimeout,
        n => {
            let rindex = n.saturating_sub(8);
            EditingField::RegisterField {
                index: rindex,
                field: RegisterField::SlaveId,
            }
        }
    });

    input_buffer.clear();
    if matches!(editing_field, Some(EditingField::Baud)) {
        let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
        let custom_index = presets.len();
        let index = presets
            .iter()
            .position(|&p| p == baud)
            .unwrap_or(custom_index);
        *edit_choice_index = Some(index);
        if index == presets.len() {
            *input_buffer = baud.to_string();
        }
        *edit_confirmed = false;
    } else {
        *edit_choice_index = None;
        *edit_confirmed = false;
    }
}

/// Begin editing: set `editing = true` and invoke `select_field_by_cursor`.
pub fn begin_edit(editing: &mut bool) {
    *editing = true;
}

/// End editing: reset editing state.
pub fn end_edit(
    editing: &mut bool,
    editing_field: &mut Option<EditingField>,
    input_buffer: &mut String,
    edit_choice_index: &mut Option<usize>,
    edit_confirmed: &mut bool,
) {
    *editing = false;
    *editing_field = None;
    input_buffer.clear();
    *edit_choice_index = None;
    *edit_confirmed = false;
}
