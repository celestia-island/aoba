use eframe::egui;
use egui::{Align, Align2, Button, ColorImage, FontId, Layout, TextureOptions, Ui, Vec2};
use image;

use crate::i18n::lang;
use crate::protocol::status::Status;
use std::sync::{Arc, Mutex};

/// Render a single-row breadcrumb (no big heading). Layout is left-to-right
/// with small gaps on left/right. Each level is styled as a button; only the
/// first level (Home / app name) is clickable. '>' is used as separator.
pub fn render_title_ui(ui: &mut Ui, inner: &Arc<Mutex<Status>>) {
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
            if let Ok(img) = image::load_from_memory(include_bytes!("../../../res/logo.png")) {
                let img = img.to_rgba8();
                let (w, h) = (img.width() as usize, img.height() as usize);
                let color_image = ColorImage::from_rgba_unmultiplied([w, h], &img);
                let tex =
                    ui.ctx()
                        .load_texture("aoba_logo", color_image, TextureOptions::default());
                ui.image((tex.id(), egui::vec2(24.0, 24.0)));
            } else {
                ui.add_space(4.0);
            }

            // Big app title (to the right of logo) - render with painter so it's not selectable
            let app_title = lang().index.title.as_str();
            ui.add_space(8.0);
            let app_w = (app_title.chars().count() * 10 + 8) as f32;
            let app_size = Vec2::new(app_w, 24.0);
            let app_rect = ui.allocate_exact_size(app_size, egui::Sense::hover()).0;
            ui.painter().text(
                app_rect.left_center(),
                Align2::LEFT_CENTER,
                app_title,
                FontId::proportional(18.0),
                ui.visuals().text_color(),
            );
            // small gap before breadcrumb
            ui.add_space(12.0);

            // Level 1: Home (clickable)
            // Use i18n text for the home label
            let home_label = lang().index.home.as_str();
            let home_w = (home_label.chars().count() * 8 + 8) as f32;
            if ui
                .add_sized(Vec2::new(home_w, 24.0), Button::new(home_label).small())
                .clicked()
            {
                if let Ok(mut g) = inner.lock() {
                    g.ui.subpage_active = false;
                    g.ui.subpage_tab_index = crate::protocol::status::SubpageTab::Config;
                }
            }

            // separator (non-selectable)
            ui.add_space(4.0);
            let sep_size = Vec2::new(12.0, 24.0);
            let sep_rect = ui.allocate_exact_size(sep_size, egui::Sense::hover()).0;
            ui.painter().text(
                sep_rect.center(),
                Align2::CENTER_CENTER,
                ">",
                FontId::proportional(14.0),
                ui.visuals().text_color(),
            );
            ui.add_space(4.0);

            // Level 2: Page name (auto-sized)
            if let Ok(g) = inner.lock() {
                let label = if g.ui.subpage_active {
                    match g.ui.subpage_tab_index {
                        crate::protocol::status::SubpageTab::Body => {
                            lang().protocol.modbus.label_modbus_settings.as_str()
                        }
                        crate::protocol::status::SubpageTab::Log => lang().tabs.tab_log.as_str(),
                        _ => lang().index.details.as_str(),
                    }
                } else {
                    lang().index.details.as_str()
                };

                let label_w = (label.chars().count() * 8 + 8) as f32;
                ui.add_sized(Vec2::new(label_w, 24.0), Button::new(label).small());

                // Level 3: optional port name
                if g.ui.subpage_active
                    && !g.ports.list.is_empty()
                    && g.ui.selected < g.ports.list.len()
                {
                    ui.add_space(4.0);
                    let sep_rect = ui
                        .allocate_exact_size(Vec2::new(12.0, 24.0), egui::Sense::hover())
                        .0;
                    ui.painter().text(
                        sep_rect.center(),
                        Align2::CENTER_CENTER,
                        ">",
                        FontId::proportional(14.0),
                        ui.visuals().text_color(),
                    );
                    ui.add_space(4.0);
                    let port_name = g.ports.list[g.ui.selected].port_name.clone();
                    let port_w = (port_name.chars().count() * 8 + 8) as f32;
                    ui.add_sized(Vec2::new(port_w, 24.0), Button::new(port_name).small());
                }
            } else {
                // fallback: show default second-level label
                let fallback = lang().index.details.as_str();
                let fw = (fallback.chars().count() * 8 + 8) as f32;
                ui.add_sized(Vec2::new(fw, 24.0), Button::new(fallback).small());
            }

            // right padding
            ui.add_space(8.0);
        });
    });
}
