mod init_font;
mod ui;

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

use eframe::{self, egui};
use egui::{vec2, IconData};

use crate::protocol::status::Status;

pub fn start() -> Result<()> {
    log::info!("[GUI] aoba GUI starting...");

    let mut options = eframe::NativeOptions::default();
    options.viewport = options
        .viewport
        .with_icon(Arc::new({
            let data = include_bytes!("../../res/logo.png");
            let data = image::load_from_memory(data)?;
            IconData {
                rgba: data.to_rgba8().to_vec(),
                width: data.width(),
                height: data.height(),
            }
        }))
        .with_inner_size(vec2(900., 600.));
    options.centered = true;

    eframe::run_native(
        "aoba",
        options,
        Box::new(|cc| Ok(Box::new(GuiApp::new(cc)))),
    )
    .map_err(|err| anyhow!("GUI has crashed: {}", err))
}

pub struct GuiApp {
    status: Arc<Mutex<Status>>,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        init_font::replace_fonts(&cc.egui_ctx);

        let status = Arc::new(Mutex::new(Status::new()));

        Self { status }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Snapshot state quickly to avoid holding lock while drawing
        let (_ports, _selected, auto_refresh, last_refresh, _error) = {
            if let Ok(guard) = self.status.lock() {
                (
                    guard.ports.clone(),
                    guard.ui.selected,
                    guard.ui.auto_refresh,
                    guard.ui.last_refresh,
                    guard.ui.error.clone(),
                )
            } else {
                (
                    crate::protocol::status::Status::default().ports,
                    0usize,
                    false,
                    None,
                    None,
                )
            }
        };

        // Use modular renderers
        ui::title::render_title(ctx);
        ui::pages::render_panels(ctx, &self.status);
        // Wrap internal state error into GUI-friendly (message, timestamp) format
        let err_ts: Option<(String, chrono::DateTime<chrono::Local>)> =
            if let Ok(g) = self.status.lock() {
                g.ui.error.clone()
            } else {
                None
            };
        ui::status::render_status(ctx, &last_refresh, auto_refresh, &err_ts, &self.status);
    }
}
