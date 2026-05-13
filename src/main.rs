#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod llm_detector;
mod api_client;
mod commands;
mod models;

use settings::Settings;
use llm_detector::LLMDetector;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .manage(Arc::new(tokio::sync::Mutex::new(Settings::load().unwrap_or_default())))
        .manage(Arc::new(tokio::sync::Mutex::new(LLMDetector::new())))
        .setup(|app| {
            let window = app.get_window("main").unwrap();

            window.set_fullscreen(true).ok();

            let window_clone = window.clone();
            let detector_clone = app.state::<Arc<tokio::sync::Mutex<LLMDetector>>>().inner().clone();

            std::thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    loop {
                        let available = detector_clone.lock().await.check_available().await;
                        let _ = window_clone.emit("llm_status", available);
                        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    }
                });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::toggle_fullscreen,
            commands::get_settings,
            commands::update_settings,
            commands::test_ollama,
            commands::test_lm_studio,
            commands::test_anthropic,
            commands::test_openai,
            commands::test_xai,
            commands::get_available_llms,
            commands::query_ai,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
