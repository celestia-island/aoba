//! 表单编辑相关的公共辅助函数，消除多处重复代码。
use crate::protocol::status::{EditingField, SubpageForm};

/// 根据 `cursor` 位置选择正在编辑的字段，并为 Baud 字段初始化预设索引与输入缓冲。
pub fn select_field_by_cursor(form: &mut SubpageForm) {
    form.editing_field = Some(match form.cursor {
        0 => EditingField::Baud,
        1 => EditingField::Parity,
        2 => EditingField::DataBits,
        3 => EditingField::StopBits,
        n => {
            let ridx = n.saturating_sub(4);
            EditingField::RegisterField { idx: ridx, field: crate::protocol::status::RegisterField::SlaveId }
        }
    });
    form.input_buffer.clear();
    if matches!(form.editing_field, Some(EditingField::Baud)) {
        let presets: [u32; 8] = [1200,2400,4800,9600,19200,38400,57600,115200];
        let custom_idx = presets.len();
        let idx = presets.iter().position(|&p| p == form.baud).unwrap_or(custom_idx);
        form.edit_choice_index = Some(idx);
        if idx == presets.len() { form.input_buffer = form.baud.to_string(); }
        form.edit_confirmed = false;
    } else {
        form.edit_choice_index = None;
        form.edit_confirmed = false;
    }
}

/// 开始编辑：设置 `editing=true` 并调用 `select_field_by_cursor`。
pub fn begin_edit(form: &mut SubpageForm) { form.editing = true; select_field_by_cursor(form); }

/// 结束编辑：清理状态。
pub fn end_edit(form: &mut SubpageForm) {
    form.editing = false;
    form.editing_field = None;
    form.input_buffer.clear();
    form.edit_choice_index = None;
    form.edit_confirmed = false;
}