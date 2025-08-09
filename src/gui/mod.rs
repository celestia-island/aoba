use eframe::{self, egui};

pub fn start() {
    log::info!("[GUI] aoba GUI starting...");
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "aoba",
        options,
        Box::new(|_cc| Ok(Box::new(AobaApp::default()))),
    );
}

#[derive(Default)]
struct AobaApp;

impl eframe::App for AobaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Aoba - Multi-protocol Debug & Simulation Tool");
            ui.label("Welcome to the GUI mode!");
        });
    }
}
