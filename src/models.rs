use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AIProvider {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "xai")]
    XAI,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "lm_studio")]
    LMStudio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APICredentials {
    pub provider: AIProvider,
    pub api_key: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMStatus {
    pub ollama_available: bool,
    pub lm_studio_available: bool,
    pub ollama_models: Vec<String>,
    pub lm_studio_models: Vec<String>,
    pub last_checked: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIQuery {
    pub provider: AIProvider,
    pub messages: Vec<Message>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub provider: AIProvider,
    pub model: String,
}
