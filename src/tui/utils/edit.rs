//! Shared helper functions for form editing to eliminate repeated code.
use crate::protocol::status::{EditingField, SubpageForm};

/// Select the field being edited based on the current `cursor` position and
/// Initialize the preset index and input buffer for the Baud field.
pub fn select_field_by_cursor(form: &mut SubpageForm) {
    form.editing_field = Some(match form.cursor {
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
            let ridx = n.saturating_sub(8);
            EditingField::RegisterField {
                idx: ridx,
                field: crate::protocol::status::RegisterField::SlaveId,
            }
        }
    });
    form.input_buffer.clear();
    if matches!(form.editing_field, Some(EditingField::Baud)) {
        let presets: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
        let custom_idx = presets.len();
        let idx = presets
            .iter()
            .position(|&p| p == form.baud)
            .unwrap_or(custom_idx);
        form.edit_choice_index = Some(idx);
        if idx == presets.len() {
            form.input_buffer = form.baud.to_string();
        }
        form.edit_confirmed = false;
    } else {
        form.edit_choice_index = None;
        form.edit_confirmed = false;
    }
}

/// Begin editing: set `editing = true` and invoke `select_field_by_cursor`.
pub fn begin_edit(form: &mut SubpageForm) {
    form.editing = true;
    select_field_by_cursor(form);
}

/// End editing: reset editing state.
pub fn end_edit(form: &mut SubpageForm) {
    form.editing = false;
    form.editing_field = None;
    form.input_buffer.clear();
    form.edit_choice_index = None;
    form.edit_confirmed = false;
}
