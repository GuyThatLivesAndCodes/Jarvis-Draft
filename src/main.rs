mod api_client;
mod llm_detector;
mod models;
mod settings;
mod tools;

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
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use tao::event_loop::EventLoop;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const SYSTEM_PROMPT: &str = "You are Jarvis, a professional AI assistant integrated into a desktop application. \
Be concise, direct, and helpful. Respond with clarity and precision. Keep responses brief unless asked for details. \
You are intelligent, knowledgeable, and always act in the user's best interest.";

#[derive(Clone)]
struct AppState {
    settings:      Arc<RwLock<Settings>>,
    llm_status:    Arc<RwLock<models::LLMStatus>>,
    is_locked:     Arc<AtomicBool>,
    lock_password: Arc<RwLock<String>>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let settings       = Arc::new(RwLock::new(Settings::load()));
    let llm_status     = Arc::new(RwLock::new(models::LLMStatus::default()));
    let is_locked      = Arc::new(AtomicBool::new(false));
    let lock_password  = Arc::new(RwLock::new(String::new()));

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

    let state = AppState {
        settings,
        llm_status,
        is_locked,
        lock_password,
    };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/query", post(query_ai))
        .route("/api/llm-status", get(get_llm_status))
        .route("/api/lock", get(get_lock_status).post(set_lock))
        .route("/api/unlock", post(unlock))
        .with_state(state);

    // Start Axum server in background
    let app_addr = "127.0.0.1:0";
    let listener = tokio::net::TcpListener::bind(app_addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    eprintln!("Jarvis running at {}", url);

    // Start server in background
    let url_clone = url.clone();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Open native window with WebView
    open_native_window(&url_clone);
}

fn open_native_window(url: &str) {
    use tao::event_loop::ControlFlow;
    use tao::event::{Event, WindowEvent};
    use tao::window::Fullscreen;

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Jarvis")
        .with_fullscreen(Some(Fullscreen::Borderless(None)))
        .with_decorations(false)
        .build(&event_loop)
        .unwrap();

    let _webview = WebViewBuilder::new(&window)
        .with_url(url)
        .with_devtools(false)
        .build()
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
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
    Json(mut body): Json<QueryBody>,
) -> impl IntoResponse {
    // Prepend system prompt
    body.messages.insert(0, models::ChatMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
    });

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

async fn get_lock_status(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "locked": s.is_locked.load(Ordering::Relaxed),
    }))
}

#[derive(serde::Deserialize)]
struct LockBody {
    password: Option<String>,
}

async fn set_lock(State(s): State<AppState>, Json(body): Json<LockBody>) -> Json<serde_json::Value> {
    if let Some(pwd) = body.password {
        let mut lock_pwd = s.lock_password.write().await;
        *lock_pwd = pwd;
    }
    s.is_locked.store(true, Ordering::Relaxed);
    Json(serde_json::json!({"ok": true, "locked": true}))
}

#[derive(serde::Deserialize)]
struct UnlockBody {
    password: String,
}

async fn unlock(
    State(s): State<AppState>,
    Json(body): Json<UnlockBody>,
) -> impl IntoResponse {
    let lock_pwd = s.lock_password.read().await;
    if lock_pwd.as_str() == body.password.as_str() || body.password.is_empty() {
        s.is_locked.store(false, Ordering::Relaxed);
        Json(serde_json::json!({"ok": true, "locked": false})).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid password"}))).into_response()
    }
}
