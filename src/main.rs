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
use tokio::sync::RwLock;
use tao::event_loop::EventLoop;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const SYSTEM_PROMPT: &str = "You are Jarvis, a professional AI assistant integrated into a desktop application. \
Be concise, direct, and helpful. Respond with clarity and precision. Keep responses brief unless asked for details. \
You are intelligent, knowledgeable, and always act in the user's best interest. \
\
You have access to these tools — USE THEM when relevant. Do not say you cannot do something these tools cover:\
- get_weather(location): Retrieve current weather for a location\
- move_jarvis(position): Move yourself (the glowing Jarvis orb) on screen. Positions: center, top-left, top-center, top-right, middle-left, middle-right, bottom-left, bottom-center, bottom-right\
- show_location(location): Display a location on Google Maps\
\
Examples:\
- \"Move to the top right\" → call move_jarvis(\"top-right\")\
- \"Move yourself to the bottom left\" → call move_jarvis(\"bottom-left\")\
- \"What's the weather in Paris?\" → call get_weather(\"Paris\")\
- \"Show me Times Square\" → call show_location(\"Times Square\")\
\
Call tools directly — never refuse a request that one of these tools can fulfill.";

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

    let state = AppState {
        settings,
        llm_status,
    };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/query", post(query_ai))
        .route("/api/llm-status", get(get_llm_status))
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
    anthropic_model:           Option<String>,
    openai_key:                Option<String>,
    openai_model:              Option<String>,
    xai_key:                   Option<String>,
    xai_model:                 Option<String>,
    auto_switch_to_local:      Option<bool>,
    notify_on_local_available: Option<bool>,
}

async fn save_settings(
    State(s): State<AppState>,
    Json(body): Json<SaveSettingsBody>,
) -> Json<serde_json::Value> {
    let mut settings = s.settings.write().await;
    if let Some(k) = body.anthropic_key { if !k.is_empty() { settings.set_key(AIProvider::Anthropic, k); } }
    if let Some(m) = body.anthropic_model { if !m.is_empty() { settings.set_model(AIProvider::Anthropic, m); } }
    if let Some(k) = body.openai_key    { if !k.is_empty() { settings.set_key(AIProvider::OpenAI, k); } }
    if let Some(m) = body.openai_model { if !m.is_empty() { settings.set_model(AIProvider::OpenAI, m); } }
    if let Some(k) = body.xai_key       { if !k.is_empty() { settings.set_key(AIProvider::XAI, k); } }
    if let Some(m) = body.xai_model { if !m.is_empty() { settings.set_model(AIProvider::XAI, m); } }
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

    let (key, model) = {
        let settings = s.settings.read().await;
        let key = settings.get_key(&body.provider).map(String::from);
        let model = settings.get_model(&body.provider);
        (key, model)
    };

    match api_client::query(body.provider, body.messages, key, model).await {
        Ok(resp) => Json(serde_json::json!({
            "content":      resp.content,
            "model":        resp.model,
            "tool_actions": resp.tool_actions,
        })).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e).into_response(),
    }
}

async fn get_llm_status(State(s): State<AppState>) -> Json<models::LLMStatus> {
    Json(s.llm_status.read().await.clone())
}
