use crate::{
    api_client::APIClient, llm_detector::LLMDetector, models::*, settings::Settings,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn toggle_fullscreen(window: tauri::Window) {
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    window.set_fullscreen(!fullscreen).ok();
}

#[tauri::command]
pub async fn get_settings(settings: State<'_, Arc<tokio::sync::Mutex<Settings>>>) -> Result<Settings, String> {
    Ok(settings.lock().await.clone())
}

#[tauri::command]
pub async fn update_settings(
    new_settings: Settings,
    settings: State<'_, Arc<tokio::sync::Mutex<Settings>>>,
) -> Result<(), String> {
    let mut s = settings.lock().await;
    *s = new_settings;
    s.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_ollama() -> Result<bool, String> {
    let mut detector = LLMDetector::new();
    Ok(detector.check_available().await.ollama_available)
}

#[tauri::command]
pub async fn test_lm_studio() -> Result<bool, String> {
    let mut detector = LLMDetector::new();
    Ok(detector.check_available().await.lm_studio_available)
}

#[tauri::command]
pub async fn test_anthropic(api_key: String) -> Result<bool, String> {
    let query = AIQuery {
        provider: AIProvider::Anthropic,
        messages: vec![Message {
            role: "user".to_string(),
            content: "Say 'test' briefly".to_string(),
        }],
        model: Some("claude-3-5-sonnet-20241022".to_string()),
    };

    match APIClient::query(query, Some(api_key)).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("Anthropic test error: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn test_openai(api_key: String) -> Result<bool, String> {
    let query = AIQuery {
        provider: AIProvider::OpenAI,
        messages: vec![Message {
            role: "user".to_string(),
            content: "Say 'test' briefly".to_string(),
        }],
        model: Some("gpt-4o-mini".to_string()),
    };

    match APIClient::query(query, Some(api_key)).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("OpenAI test error: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn test_xai(api_key: String) -> Result<bool, String> {
    let query = AIQuery {
        provider: AIProvider::XAI,
        messages: vec![Message {
            role: "user".to_string(),
            content: "Say 'test' briefly".to_string(),
        }],
        model: Some("grok-2".to_string()),
    };

    match APIClient::query(query, Some(api_key)).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("xAI test error: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn get_available_llms(
    detector: State<'_, Arc<tokio::sync::Mutex<LLMDetector>>>,
) -> Result<LLMStatus, String> {
    let mut d = detector.lock().await;
    let status = d.check_available().await;
    Ok(status)
}

#[tauri::command]
pub async fn query_ai(
    query: AIQuery,
    api_key: Option<String>,
) -> Result<AIResponse, String> {
    APIClient::query(query, api_key)
        .await
        .map_err(|e| e.to_string())
}
