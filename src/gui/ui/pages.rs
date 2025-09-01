use eframe::egui;
use egui::{CentralPanel, Checkbox, ScrollArea, SidePanel};

use crate::{i18n::lang, protocol::status::Status};

pub fn render_panels(ctx: &egui::Context, inner: &std::sync::Arc<std::sync::Mutex<Status>>) {
    // Snapshot state
    let (ports, selected, auto_refresh) = if let Ok(guard) = inner.lock() {
        (guard.ports.clone(), guard.selected, guard.auto_refresh)
    } else {
        (Vec::new(), 0usize, false)
    };

    // Left side panel: list of COM ports and controls
    SidePanel::left("left_panel")
        .resizable(true)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    if let Ok(mut guard) = inner.lock() {
                        guard.refresh();
                    }
                }
                let mut checkbox = auto_refresh;
                if ui.add(Checkbox::new(&mut checkbox, "Auto")).changed() {
                    if let Ok(mut guard) = inner.lock() {
                        guard.auto_refresh = checkbox;
                    }
                }
            });

            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                for (i, p) in ports.iter().enumerate() {
                    let label = format!("{} - {:?}", p.port_name, p.port_type);
                    let selected_bool = i == selected;
                    if ui.selectable_label(selected_bool, label).clicked() {
                        if let Ok(mut guard) = inner.lock() {
                            guard.selected = i;
                            guard.clear_error();
                        }
                    }
                }
            });
        });

    // Right / central panel for details (sibling to the SidePanel)
    CentralPanel::default().show(ctx, |ui| {
        ui.group(|ui| {
            ui.heading(lang().details.as_str());
            ui.separator();
            if ports.is_empty() {
                ui.label(lang().no_com_ports.as_str());
            } else {
                // Guard against out-of-bounds if selected is stale
                let idx = if selected >= ports.len() { 0 } else { selected };
                let p = &ports[idx];
                ui.label(format!("{} {}", lang().name_label, p.port_name));
                ui.label(format!("{} {:?}", lang().type_label, p.port_type));
                ui.separator();
                ui.label(lang().details_placeholder.as_str());
            }
        });
    });
}
