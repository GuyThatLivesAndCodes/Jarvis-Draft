use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AIProvider {
    #[serde(rename = "anthropic")] Anthropic,
    #[serde(rename = "openai")]    OpenAI,
    #[serde(rename = "xai")]       XAI,
    #[serde(rename = "ollama")]    Ollama,
    #[serde(rename = "lm_studio")] LMStudio,
}

impl AIProvider {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAI    => "OpenAI",
            Self::XAI       => "xAI",
            Self::Ollama    => "Ollama",
            Self::LMStudio  => "LM Studio",
        }
    }
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Anthropic => "claude-3-5-sonnet-20241022",
            Self::OpenAI    => "gpt-4o-mini",
            Self::XAI       => "grok-2",
            Self::Ollama    => "qwen2",
            Self::LMStudio  => "default",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMStatus {
    pub ollama_available:    bool,
    pub lm_studio_available: bool,
    pub ollama_models:       Vec<String>,
    pub lm_studio_models:    Vec<String>,
}

impl Default for LLMStatus {
    fn default() -> Self {
        Self {
            ollama_available:    false,
            lm_studio_available: false,
            ollama_models:       vec![],
            lm_studio_models:    vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role:    String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub content:  String,
    pub provider: AIProvider,
    pub model:    String,
}
