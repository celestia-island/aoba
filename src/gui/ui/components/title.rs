use anyhow::Result;

use eframe::egui;
use egui::{Align, Align2, Button, FontId, Layout, Ui, Vec2};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, write_status},
};

/// Render a single-row breadcrumb layout. Layout is left-to-right
/// with small gaps on left/right. Each level is styled as a button; only the
/// first level (app name) is clickable. '>' is used as separator.
pub fn render_title_ui(ui: &mut Ui) -> Result<()> {
    let desired_h = 32.0f32;
    let avail_w = ui.available_width();
    let size = Vec2::new(avail_w, desired_h);

    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    // Use the existing allocate_ui_at_rect (allowed temporarily) so child
    // widgets do not change the top panel height. We'll migrate to a
    // non-deprecated API later.
    #[allow(deprecated)]
    ui.allocate_ui_at_rect(rect, |child| {
        // left-to-right layout with vertical centering
        child.with_layout(Layout::left_to_right(Align::Center), |ui| {
            // Start with 2 spaces from left (space for loading spinner)
            ui.add_space(8.);

            // Loading spinner at left using ◜◝◞◟ rotation
            if let Ok(is_busy) = read_status(|g| Ok(g.temporarily.busy.busy)) {
                if is_busy {
                    if let Ok(frame) = read_status(|g| Ok(g.temporarily.busy.spinner_frame)) {
                        let spinner_chars = ['◜', '◝', '◞', '◟'];
                        let ch = spinner_chars[(frame as usize) % spinner_chars.len()];
                        let spinner_rect = ui
                            .allocate_exact_size(Vec2::new(16., 24.), egui::Sense::hover())
                            .0;
                        ui.painter().text(
                            spinner_rect.center(),
                            Align2::CENTER_CENTER,
                            &ch.to_string(),
                            FontId::proportional(14.),
                            ui.visuals().text_color(),
                        );
                        ui.add_space(4.);
                    }
                }
            }

            // Breadcrumb implementation
            // Level 1: AOBA title (clickable, goes to home)
            let app_title = lang().index.title.as_str();
            let app_w = (app_title.chars().count() * 8 + 8) as f32;
            if ui
                .add_sized(Vec2::new(app_w, 24.), Button::new(app_title).small())
                .clicked()
            {
                write_status(|g| {
                    g.page = types::Page::Entry { cursor: None };
                    Ok(())
                })
                .unwrap_or(());
            }

            // Build breadcrumb path based on current page
            if let Ok(breadcrumb_info) = read_status(|g| {
                match &g.page {
                    // Entry page: only show AOBA title (already shown)
                    types::Page::Entry { .. } => {
                        Ok((None::<String>, None::<String>, None::<String>))
                    }

                    // Port configuration page: AOBA title > COMx
                    types::Page::ModbusConfig { selected_port, .. } => {
                        let port_name = if *selected_port < g.ports.order.len() {
                            let name = &g.ports.order[*selected_port];
                            g.ports.map.get(name).map(|p| p.port_name.clone())
                        } else {
                            None
                        };
                        Ok((port_name, None::<String>, None::<String>))
                    }

                    // Modbus master/slave configuration: AOBA title > COMx > Modbus
                    types::Page::ModbusDashboard { selected_port, .. } => {
                        let port_name = if *selected_port < g.ports.order.len() {
                            let name = &g.ports.order[*selected_port];
                            g.ports.map.get(name).map(|p| p.port_name.clone())
                        } else {
                            None
                        };
                        let modbus_label = lang()
                            .protocol
                            .modbus
                            .label_modbus_settings
                            .as_str()
                            .to_string();
                        Ok((port_name, Some(modbus_label), None::<String>))
                    }

                    // Manual debug log: AOBA title > COMx > Communication Log
                    types::Page::ModbusLog { selected_port, .. } => {
                        let port_name = if *selected_port < g.ports.order.len() {
                            let name = &g.ports.order[*selected_port];
                            g.ports.map.get(name).map(|p| p.port_name.clone())
                        } else {
                            None
                        };
                        let log_label = lang().tabs.tab_log.as_str().to_string();
                        Ok((port_name, Some(log_label), None::<String>))
                    }

                    // About page: AOBA title > About
                    types::Page::About { .. } => {
                        let about_label = lang().index.about_label.as_str().to_string();
                        Ok((None::<String>, Some(about_label), None::<String>))
                    }
                }
            }) {
                let (port_name, secondary_label, _tertiary_label) = breadcrumb_info;

                // Level 2: COMx (port name) if present
                if let Some(port) = port_name {
                    ui.add_space(4.);
                    let sep_rect = ui
                        .allocate_exact_size(Vec2::new(12., 24.), egui::Sense::hover())
                        .0;
                    ui.painter().text(
                        sep_rect.center(),
                        Align2::CENTER_CENTER,
                        ">",
                        FontId::proportional(14.),
                        ui.visuals().text_color(),
                    );
                    ui.add_space(4.);
                    let port_w = (port.chars().count() * 8 + 8) as f32;
                    ui.add_sized(Vec2::new(port_w, 24.), Button::new(port).small());
                }

                // Level 3: Secondary label (Modbus, communication log, etc.) if present
                if let Some(label) = secondary_label {
                    ui.add_space(4.);
                    let sep_rect = ui
                        .allocate_exact_size(Vec2::new(12., 24.), egui::Sense::hover())
                        .0;
                    ui.painter().text(
                        sep_rect.center(),
                        Align2::CENTER_CENTER,
                        ">",
                        FontId::proportional(14.),
                        ui.visuals().text_color(),
                    );
                    ui.add_space(4.);
                    let label_w = (label.chars().count() * 8 + 8) as f32;
                    ui.add_sized(Vec2::new(label_w, 24.), Button::new(label).small());
                }
            }

            // right padding
            ui.add_space(8.);
        });
    });

    Ok(())
}
