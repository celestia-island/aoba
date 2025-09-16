use anyhow::Result;

use eframe::egui;
use egui::{Align, Align2, Button, ColorImage, FontId, Layout, TextureOptions, Ui, Vec2};

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, write_status},
};

/// Render a single-row breadcrumb (no big heading). Layout is left-to-right
/// with small gaps on left/right. Each level is styled as a button; only the
/// first level (Home / app name) is clickable. '>' is used as separator.
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
            // Logo at left
            if let Ok(img) = image::load_from_memory(include_bytes!("../../../../res/logo.png")) {
                let img = img.to_rgba8();
                let (w, h) = (img.width() as usize, img.height() as usize);
                let color_image = ColorImage::from_rgba_unmultiplied([w, h], &img);
                let tex =
                    ui.ctx()
                        .load_texture("aoba_logo", color_image, TextureOptions::default());
                ui.image((tex.id(), egui::vec2(24., 24.)));
            } else {
                ui.add_space(4.);
            }

            // Big app title (to the right of logo) - render with painter so it's not selectable
            let app_title = lang().index.title.as_str();
            ui.add_space(8.);
            let app_w = (app_title.chars().count() * 10 + 8) as f32;
            let app_size = Vec2::new(app_w, 24.);
            let app_rect = ui.allocate_exact_size(app_size, egui::Sense::hover()).0;
            ui.painter().text(
                app_rect.left_center(),
                Align2::LEFT_CENTER,
                app_title,
                FontId::proportional(18.),
                ui.visuals().text_color(),
            );
            // small gap before breadcrumb
            ui.add_space(12.);

            // Level 1: Home (clickable)
            // Use i18n text for the home label
            let home_label = lang().index.home.as_str();
            let home_w = (home_label.chars().count() * 8 + 8) as f32;
            if ui
                .add_sized(Vec2::new(home_w, 24.), Button::new(home_label).small())
                .clicked()
            {
                write_status(|g| {
                    g.page = types::Page::Entry { cursor: None };
                    Ok(())
                })
                .unwrap_or(());
            }

            // separator (non-selectable)
            ui.add_space(4.);
            let sep_size = Vec2::new(12., 24.);
            let sep_rect = ui.allocate_exact_size(sep_size, egui::Sense::hover()).0;
            ui.painter().text(
                sep_rect.center(),
                Align2::CENTER_CENTER,
                ">",
                FontId::proportional(14.),
                ui.visuals().text_color(),
            );
            ui.add_space(4.);

            // Level 2: Page name (auto-sized)
            if let Ok((label, maybe_port)) = read_status(|g| {
                let subpage_active = matches!(
                    g.page,
                    types::Page::ModbusConfig { .. }
                        | types::Page::ModbusDashboard { .. }
                        | types::Page::ModbusLog { .. }
                        | types::Page::About { .. }
                );
                let label = if subpage_active {
                    // Try to infer a tab label from current_page; default to details.
                    match g.page {
                        types::Page::ModbusDashboard { .. } => lang()
                            .protocol
                            .modbus
                            .label_modbus_settings
                            .as_str()
                            .to_string(),
                        types::Page::ModbusLog { .. } => lang().tabs.tab_log.as_str().to_string(),
                        _ => lang().index.details.as_str().to_string(),
                    }
                } else {
                    lang().index.details.as_str().to_string()
                };

                let maybe_port = if subpage_active && !g.ports.order.is_empty() {
                    // derive selection from page
                    let sel = match &g.page {
                        types::Page::Entry { cursor } => match cursor {
                            Some(types::ui::EntryCursor::Com { idx }) => *idx,
                            Some(types::ui::EntryCursor::About) => {
                                g.ports.order.len().saturating_add(2)
                            }
                            Some(types::ui::EntryCursor::Refresh) => g.ports.order.len(),
                            Some(types::ui::EntryCursor::CreateVirtual) => {
                                g.ports.order.len().saturating_add(1)
                            }
                            None => 0usize,
                        },
                        types::Page::ModbusDashboard { selected_port, .. }
                        | types::Page::ModbusConfig { selected_port, .. }
                        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
                        _ => 0usize,
                    };
                    if sel < g.ports.order.len() {
                        let name = &g.ports.order[sel];
                        Some(
                            g.ports
                                .map
                                .get(name)
                                .map(|p| p.port_name.clone())
                                .unwrap_or_default(),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };
                Ok((label, maybe_port))
            }) {
                let label_w = (label.chars().count() * 8 + 8) as f32;
                ui.add_sized(Vec2::new(label_w, 24.), Button::new(label).small());

                if let Some(port_name) = maybe_port {
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
                    let port_w = (port_name.chars().count() * 8 + 8) as f32;
                    ui.add_sized(Vec2::new(port_w, 24.), Button::new(port_name).small());
                }
            } else {
                // fallback: show default second-level label
                let fallback = lang().index.details.as_str();
                let fw = (fallback.chars().count() * 8 + 8) as f32;
                ui.add_sized(Vec2::new(fw, 24.), Button::new(fallback).small());
            }

            // right padding
            ui.add_space(8.);
        });
    });

    Ok(())
}
