mod init_font;
mod ui;
use flume;

use anyhow::{anyhow, Result};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use eframe::{self, egui};
use egui::{vec2, IconData};

use crate::{
    protocol::status::types::Status,
    tui::utils::bus::{Bus, CoreToUi, UiToCore},
};

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
    status: Arc<RwLock<Status>>,
    bus: Bus,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        init_font::replace_fonts(&cc.egui_ctx);

        let status = Arc::new(RwLock::new(Status::default()));

        // Create channels and a simple core-like thread for demo handling of Refresh/Quit.
        let (core_tx, core_rx) = flume::unbounded::<CoreToUi>();
        let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>();
        let bus = Bus::new(core_rx, ui_tx.clone());

        // Spawn a demo core thread that listens for UI commands and replies.
        thread::spawn(move || loop {
            if let Ok(msg) = ui_rx.recv() {
                match msg {
                    UiToCore::Refresh => {
                        log::info!("[GUI-core-demo] received Refresh");
                        let _ = core_tx.send(CoreToUi::Refreshed);
                    }
                    UiToCore::Quit => {
                        log::info!("[GUI-core-demo] received Quit, exiting demo core");
                        let _ = core_tx.send(CoreToUi::Refreshed);
                        break;
                    }
                    UiToCore::PausePolling => {
                        let _ = core_tx.send(CoreToUi::Refreshed);
                    }
                    UiToCore::ResumePolling => {
                        let _ = core_tx.send(CoreToUi::Refreshed);
                    }
                    UiToCore::ToggleRuntime(_) => {
                        // Demo GUI core ignores ToggleRuntime
                        let _ = core_tx.send(CoreToUi::Refreshed);
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        });

        Self { status, bus }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Delegate to centralized UI renderer (title + breadcrumb + pages + status)
        crate::gui::ui::render_ui(ctx, frame, &self.status, &self.bus);
    }
}
