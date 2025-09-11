use crate::tui::utils::bus::Bus;
use crate::{i18n::lang, protocol::status::Status};
use eframe::egui;
use eframe::Frame;
use egui::CentralPanel;

pub fn render_modbus(
    ctx: &egui::Context,
    inner: &std::sync::Arc<std::sync::Mutex<Status>>,
    _bus: &Bus,
    _frame: &mut Frame,
) {
    CentralPanel::default().show(ctx, |ui| {
        ui.heading(lang().protocol.modbus.label_modbus_settings.as_str());
        ui.separator();
        // Top controls: Edit toggle, Add / Delete register
        ui.horizontal(|ui| {
            if ui.button("Edit Toggle").clicked() {
                let _ = crate::protocol::status::status_rw::write_status(inner, |guard| {
                    // inline init_subpage_form
                    if guard.ui.subpage_form.is_none() {
                        guard.ui.subpage_form =
                            Some(crate::protocol::status::SubpageForm::default());
                    }
                    guard.ui.subpage_active = true;
                    let modbus_page = crate::protocol::status::Page::Modbus {
                        selected: guard.ui.selected,
                        subpage_active: guard.ui.subpage_active,
                        subpage_form: guard.ui.subpage_form.clone(),
                        subpage_tab_index: guard.ui.subpage_tab_index,
                        logs: crate::protocol::status::ui::ui_logs_get(guard),
                        log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                        log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                        log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                        log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(
                            guard,
                        ),
                        input_mode: guard.ui.input_mode,
                        input_editing: guard.ui.input_editing,
                        input_buffer: guard.ui.input_buffer.clone(),
                        app_mode: guard.ui.app_mode,
                    };
                    if guard.ui.pages.is_empty() {
                        guard.ui.pages.push(modbus_page);
                    } else {
                        *guard.ui.pages.last_mut().unwrap() = modbus_page;
                    }
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.editing = !form.editing;
                        if !form.editing {
                            form.editing_field = None;
                            form.input_buffer.clear();
                            form.edit_choice_index = None;
                            form.edit_confirmed = false;
                        }
                    }
                    Ok(())
                });
            }
            if ui.button("Add Register").clicked() {
                let _ = crate::protocol::status::status_rw::write_status(inner, |guard| {
                    // inline init_subpage_form
                    if guard.ui.subpage_form.is_none() {
                        guard.ui.subpage_form =
                            Some(crate::protocol::status::SubpageForm::default());
                    }
                    guard.ui.subpage_active = true;
                    let modbus_page = crate::protocol::status::Page::Modbus {
                        selected: guard.ui.selected,
                        subpage_active: guard.ui.subpage_active,
                        subpage_form: guard.ui.subpage_form.clone(),
                        subpage_tab_index: guard.ui.subpage_tab_index,
                        logs: crate::protocol::status::ui::ui_logs_get(guard),
                        log_selected: crate::protocol::status::ui::ui_log_selected_get(guard),
                        log_view_offset: crate::protocol::status::ui::ui_log_view_offset_get(guard),
                        log_auto_scroll: crate::protocol::status::ui::ui_log_auto_scroll_get(guard),
                        log_clear_pending: crate::protocol::status::ui::ui_log_clear_pending_get(
                            guard,
                        ),
                        input_mode: guard.ui.input_mode,
                        input_editing: guard.ui.input_editing,
                        input_buffer: guard.ui.input_buffer.clone(),
                        app_mode: guard.ui.app_mode,
                    };
                    if guard.ui.pages.is_empty() {
                        guard.ui.pages.push(modbus_page);
                    } else {
                        *guard.ui.pages.last_mut().unwrap() = modbus_page;
                    }
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.registers
                            .push(crate::protocol::status::RegisterEntry::default());
                    }
                    Ok(())
                });
            }
            if ui.button("Delete Register").clicked() {
                let _ = crate::protocol::status::status_rw::write_status(inner, |guard| {
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.registers.pop();
                    }
                    Ok(())
                });
            }
        });

        // reuse components subpage as a simple representation for modbus
        crate::gui::ui::components::render_subpage(ui, inner);
    });
}
