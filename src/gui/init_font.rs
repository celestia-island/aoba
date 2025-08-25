pub fn replace_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "更纱黑体".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../res/mono-sc-nerd.ttf"
        ))),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "更纱黑体".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("更纱黑体".to_owned());

    ctx.set_fonts(fonts);
}
