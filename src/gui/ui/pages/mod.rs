pub mod about;
pub mod entry;
pub mod modbus;

use crate::tui::utils::bus::{Bus, UiToCore};
use crate::{i18n::lang, protocol::status::Status};
use eframe::egui::CentralPanel;
use eframe::egui::Context;
use eframe::Frame;

/// Render top-level panels by delegating to per-page renderers.
pub fn render_panels(
    ctx: &Context,
    inner: &std::sync::Arc<std::sync::Mutex<Status>>,
    bus: &Bus,
    frame: &mut Frame,
) {
    // NOTE: drawer (left-side panel) was extracted to
    // `crate::gui::ui::components::drawer::render_drawer`.
    // It is intentionally NOT rendered here so that each page can choose
    // whether to render the drawer. Pages may call that function when
    // they want the drawer to appear.

    // Central area: delegate to per-page renderers
    let subpage_active = if let Ok(g) = inner.lock() {
        g.ui.subpage_active
    } else {
        false
    };
    if subpage_active {
        let tab = if let Ok(g) = inner.lock() {
            g.ui.subpage_tab_index
        } else {
            crate::protocol::status::SubpageTab::Config
        };
        match tab {
            crate::protocol::status::SubpageTab::Log => {
                // logs: render in a central area using simplified component API
                CentralPanel::default().show(ctx, |ui| {
                    crate::gui::ui::components::render_logs(ui);
                });
            }
            crate::protocol::status::SubpageTab::Body => {
                CentralPanel::default().show(ctx, |ui| {
                    crate::gui::ui::components::render_subpage(ui, inner);
                });
            }
            _ => {
                // For the default entry page, render the left drawer alongside the central panel
                egui::SidePanel::left("left_panel")
                    .resizable(true)
                    .default_width(360.0)
                    .show(ctx, |ui| {
                        crate::gui::ui::components::render_drawer_ui(ui, inner, bus);
                    });

                CentralPanel::default().show(ctx, |ui| {
                    entry::render_entry_ui(ui, inner, bus, frame);
                });
            }
        }
    } else {
        // On the home page (subpage not active), show the left drawer and the central entry UI
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                crate::gui::ui::components::render_drawer_ui(ui, inner, bus);
            });

        CentralPanel::default().show(ctx, |ui| {
            entry::render_entry_ui(ui, inner, bus, frame);
        });
    }
}
