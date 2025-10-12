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
    protocol::status::{init_status, types::Status},
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
    .map_err(|err| anyhow!("GUI has crashed: {err}"))
}

pub struct GuiApp {
    bus: Bus,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        init_font::replace_fonts(&cc.egui_ctx);

        let status = Arc::new(RwLock::new(Status::default()));

        // Initialize the global status
        if let Err(err) = init_status(status.clone()) {
            log::error!("Failed to initialize global status: {err}");
        }

        // Create channels and a simple core-like thread for demo handling of Refresh/Quit.
        let (core_tx, core_rx) = flume::unbounded::<CoreToUi>();
        let (ui_tx, ui_rx) = flume::unbounded::<UiToCore>();
        let bus = Bus::new(core_rx, ui_tx.clone());

        // Spawn a demo core thread that listens for UI commands and replies.
        thread::spawn(move || {
            if let Err(err) = run_demo_core(ui_rx, core_tx) {
                log::error!("Demo core thread exited with error: {err}");
            }
        });

        Self { bus }
    }
}

// Extracted demo core loop so it can return a Result and use `?` on sends.
fn run_demo_core(ui_rx: flume::Receiver<UiToCore>, core_tx: flume::Sender<CoreToUi>) -> Result<()> {
    loop {
        match ui_rx.recv() {
            Ok(msg) => match msg {
                UiToCore::Refresh => {
                    log::info!("Received Refresh");
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                }
                UiToCore::Quit => {
                    log::info!("Received Quit, exiting demo core");
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                    break;
                }
                UiToCore::PausePolling => {
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                }
                UiToCore::ResumePolling => {
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                }
                UiToCore::ToggleRuntime(_) => {
                    // Demo GUI core ignores ToggleRuntime
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
                }
                UiToCore::SendRegisterUpdate { .. } => {
                    // Demo GUI core ignores SendRegisterUpdate (no subprocess support)
                    log::debug!("Received SendRegisterUpdate (ignored in demo GUI)");
                }
            },
            Err(_) => {
                // Channel closed; exit gracefully
                log::info!("`ui_rx` closed, exiting demo core");
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Delegate to centralized UI renderer (title + breadcrumb + pages + status)
        crate::gui::ui::render_ui(ctx, frame, &self.bus);
    }
}
