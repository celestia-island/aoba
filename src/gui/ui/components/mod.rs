pub mod config_panel;
pub mod log_input;
pub mod log_panel;
pub mod modbus_panel;
pub mod mode_selector;
pub mod mqtt_panel;

// The GUI components folder mirrors the TUI components structure. Files are UI-specific
// implementations for egui. Some files are initially created as lightweight stubs matching
// the TUI component names so callers can import the same module names.

// Forwarding helpers to keep previous call-sites working via a stable API at
// `crate::gui::ui::components::render_*`.
use crate::protocol::status::Status;
use crate::tui::utils::bus::{Bus, UiToCore};
use eframe::egui::{Checkbox, ScrollArea, Ui};
use std::sync::{Arc, Mutex};

/// Drawer implementation inlined so the adapter file can be removed safely.
pub fn render_drawer_ui(ui: &mut Ui, inner: &Arc<Mutex<Status>>, bus: &Bus) {
    // Snapshot from the Status guard
    let (ports, selected, auto_refresh) =
        crate::protocol::status::status_rw::read_status(inner, |g| {
            Ok((g.ports.clone(), g.ui.selected, g.ui.auto_refresh))
        })
        .unwrap_or_else(|_| {
            (
                crate::protocol::status::Status::default().ports,
                0usize,
                false,
            )
        });

    ui.horizontal(|ui| {
        if ui
            .button(crate::i18n::lang().index.refresh_action.as_str())
            .clicked()
        {
            let _ = bus.ui_tx.send(UiToCore::Refresh);
        }
        if ui
            .button(crate::i18n::lang().hotkeys.press_q_quit.as_str())
            .clicked()
        {
            let _ = bus.ui_tx.send(UiToCore::Quit);
        }
        let mut checkbox = auto_refresh;
        if ui
            .add(Checkbox::new(
                &mut checkbox,
                crate::i18n::lang().index.auto_off.as_str(),
            ))
            .changed()
        {
            let _ = crate::protocol::status::status_rw::write_status(inner, |g| {
                g.ui.auto_refresh = checkbox;
                // inline clear_error if needed (keep behaviour consistent)
                crate::protocol::status::ui::ui_error_set(g, None);
                Ok(())
            });
        }
    });

    ui.separator();

    ScrollArea::vertical().show(ui, |ui| {
        for (i, p) in ports.list.iter().enumerate() {
            let label = format!("{} - {:?}", p.port_name, p.port_type);
            let selected_bool = i == selected;
            if ui.selectable_label(selected_bool, label).clicked() {
                let _ = crate::protocol::status::status_rw::write_status(inner, |g| {
                    g.ui.selected = i;
                    crate::protocol::status::ui::ui_error_set(g, None); // inline clear_error
                    Ok(())
                });
            }
        }
    });
}

/// Logs panel inlined.
pub fn render_logs(ui: &mut Ui) {
    ui.vertical(|ui| {
        ui.heading(crate::i18n::lang().tabs.tab_log.as_str());
        ui.separator();
        ui.label("Logs panel placeholder (simplified)");
    });
}

/// Subpage placeholder inlined.
pub fn render_subpage(_ui: &mut Ui, _inner: &Arc<Mutex<Status>>) {
    // placeholder to keep API stable for pages that reuse a simple representation
}
