mod init_font;

use std::sync::Arc;

use anyhow::{anyhow, Result};

use eframe::{self, egui};

use egui::IconData;
use init_font::replace_fonts;

pub fn start() -> Result<()> {
    log::info!("[GUI] aoba GUI starting...");

    let mut options = eframe::NativeOptions::default();
    options.viewport = options.viewport.with_icon(Arc::new({
        let data = include_bytes!("../../res/logo.png");
        let data = image::load_from_memory(data)?;
        IconData {
            rgba: data.to_rgba8().to_vec(),
            width: data.width(),
            height: data.height(),
        }
    }));

    eframe::run_native("aoba", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
        .map_err(|err| anyhow!("GUI has crashed: {}", err))
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Page {
    #[default]
    About,
}

pub struct App {
    pub selected_page: Page,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        replace_fonts(&cc.egui_ctx);
        Self {
            selected_page: Page::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Aoba - Multi-protocol Debug & Simulation Tool");
            ui.label("欢迎使用");
        });
    }
}
