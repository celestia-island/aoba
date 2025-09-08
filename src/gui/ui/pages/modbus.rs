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
                if let Ok(mut guard) = inner.lock() {
                    if guard.ui.subpage_form.is_none() {
                        guard.init_subpage_form();
                    }
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.editing = !form.editing;
                        // clear editing helpers when toggled off
                        if !form.editing {
                            form.editing_field = None;
                            form.input_buffer.clear();
                            form.edit_choice_index = None;
                            form.edit_confirmed = false;
                        }
                    }
                }
            }
            if ui.button("Add Register").clicked() {
                if let Ok(mut guard) = inner.lock() {
                    if guard.ui.subpage_form.is_none() {
                        guard.init_subpage_form();
                    }
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.registers
                            .push(crate::protocol::status::RegisterEntry::default());
                    }
                }
            }
            if ui.button("Delete Register").clicked() {
                if let Ok(mut guard) = inner.lock() {
                    if let Some(form) = guard.ui.subpage_form.as_mut() {
                        form.registers.pop();
                    }
                }
            }
        });

        if let Ok(_guard) = inner.lock() {
            // reuse components subpage as a simple representation for modbus
            crate::gui::ui::components::render_subpage(ui, inner);
        }
    });
}
