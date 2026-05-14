mod api_client;
mod llm_detector;
mod models;
mod settings;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use models::AIProvider;
use settings::Settings;
use std::sync::Arc;
use tokio::sync::RwLock;

const INDEX_HTML: &str = include_str!("../assets/index.html");

#[derive(Clone)]
struct AppState {
    settings:   Arc<RwLock<Settings>>,
    llm_status: Arc<RwLock<models::LLMStatus>>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let settings   = Arc::new(RwLock::new(Settings::load()));
    let llm_status = Arc::new(RwLock::new(models::LLMStatus::default()));

    // Background LLM status polling
    {
        let llm_status = llm_status.clone();
        tokio::spawn(async move {
            loop {
                let status = llm_detector::check_llm_status().await;
                *llm_status.write().await = status;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    let state = AppState { settings, llm_status };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/query", post(query_ai))
        .route("/api/llm-status", get(get_llm_status))
        .with_state(state);

    // Bind to random available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    eprintln!("Jarvis running at {}", url);

    // Open browser after short delay
    {
        let url = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = open::that(&url);
        });
    }

    axum::serve(listener, app).await.unwrap();
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn get_settings(State(s): State<AppState>) -> Json<serde_json::Value> {
    let settings = s.settings.read().await;
    Json(serde_json::to_value(&*settings).unwrap_or_default())
}

#[derive(serde::Deserialize)]
struct SaveSettingsBody {
    anthropic_key:             Option<String>,
    openai_key:                Option<String>,
    xai_key:                   Option<String>,
    auto_switch_to_local:      Option<bool>,
    notify_on_local_available: Option<bool>,
}

async fn save_settings(
    State(s): State<AppState>,
    Json(body): Json<SaveSettingsBody>,
) -> Json<serde_json::Value> {
    let mut settings = s.settings.write().await;
    if let Some(k) = body.anthropic_key { if !k.is_empty() { settings.set_key(AIProvider::Anthropic, k); } }
    if let Some(k) = body.openai_key    { if !k.is_empty() { settings.set_key(AIProvider::OpenAI, k); } }
    if let Some(k) = body.xai_key       { if !k.is_empty() { settings.set_key(AIProvider::XAI, k); } }
    if let Some(v) = body.auto_switch_to_local      { settings.auto_switch_to_local = v; }
    if let Some(v) = body.notify_on_local_available { settings.notify_on_local_available = v; }
    settings.save();
    Json(serde_json::json!({"ok": true}))
}

#[derive(serde::Deserialize)]
struct QueryBody {
    provider: AIProvider,
    messages: Vec<models::ChatMessage>,
}

async fn query_ai(
    State(s): State<AppState>,
    Json(body): Json<QueryBody>,
) -> impl IntoResponse {
    let key = {
        let settings = s.settings.read().await;
        settings.get_key(&body.provider).map(String::from)
    };
    match api_client::query(body.provider, body.messages, key).await {
        Ok(resp) => Json(serde_json::json!({
            "content": resp.content,
            "model": resp.model,
        })).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e).into_response(),
    }
}

async fn get_llm_status(State(s): State<AppState>) -> Json<models::LLMStatus> {
    Json(s.llm_status.read().await.clone())
}
