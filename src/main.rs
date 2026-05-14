#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod api_client;
mod llm_detector;
mod models;
mod settings;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Jarvis")
            .with_fullscreen(true)
            .with_decorations(false)
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Jarvis",
        options,
        Box::new(|cc| Ok(Box::new(app::JarvisApp::new(cc)))),
    )
}

fn load_icon() -> std::sync::Arc<egui::viewport::IconData> {
    // 32×32 RGBA blue icon embedded as bytes
    let rgba: Vec<u8> = (0..32 * 32)
        .flat_map(|_| [30u8, 100, 180, 255])
        .collect();
    std::sync::Arc::new(egui::viewport::IconData { rgba, width: 32, height: 32 })
}
