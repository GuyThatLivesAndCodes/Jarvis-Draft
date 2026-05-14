use crate::models::{AIProvider, ChatMessage, AIResponse};

pub async fn query(
    provider: AIProvider,
    messages: Vec<ChatMessage>,
    api_key:  Option<String>,
) -> Result<AIResponse, String> {
    match provider {
        AIProvider::Anthropic => anthropic(messages, api_key).await,
        AIProvider::OpenAI    => openai(messages, api_key).await,
        AIProvider::XAI       => xai(messages, api_key).await,
        AIProvider::Ollama    => ollama(messages).await,
        AIProvider::LMStudio  => lm_studio(messages).await,
    }
}

async fn anthropic(msgs: Vec<ChatMessage>, key: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing Anthropic API key")?;
    let model = "claude-3-5-sonnet-20241022".to_string();
    let body  = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>()
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let content = resp["content"][0]["text"].as_str().unwrap_or("No response").to_string();
    Ok(AIResponse { content, provider: AIProvider::Anthropic, model })
}

async fn openai(msgs: Vec<ChatMessage>, key: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing OpenAI API key")?;
    let model = "gpt-4o-mini".to_string();
    let body  = serde_json::json!({
        "model": model,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>()
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {key}"))
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let content = resp["choices"][0]["message"]["content"].as_str().unwrap_or("No response").to_string();
    Ok(AIResponse { content, provider: AIProvider::OpenAI, model })
}

async fn xai(msgs: Vec<ChatMessage>, key: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing xAI API key")?;
    let model = "grok-2".to_string();
    let body  = serde_json::json!({
        "model": model,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>()
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.x.ai/v1/chat/completions")
        .header("Authorization", format!("Bearer {key}"))
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let content = resp["choices"][0]["message"]["content"].as_str().unwrap_or("No response").to_string();
    Ok(AIResponse { content, provider: AIProvider::XAI, model })
}

async fn ollama(msgs: Vec<ChatMessage>) -> Result<AIResponse, String> {
    let model  = "qwen2".to_string();
    let prompt = msgs.iter().map(|m| format!("{}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n");
    let body   = serde_json::json!({"model": model, "prompt": prompt, "stream": false});
    let resp: serde_json::Value = reqwest::Client::new()
        .post("http://localhost:11434/api/generate")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let content = resp["response"].as_str().unwrap_or("No response").to_string();
    Ok(AIResponse { content, provider: AIProvider::Ollama, model })
}

async fn lm_studio(msgs: Vec<ChatMessage>) -> Result<AIResponse, String> {
    let model = "default".to_string();
    let body  = serde_json::json!({
        "model": model,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
        "temperature": 0.7
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post("http://localhost:1234/v1/chat/completions")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let content = resp["choices"][0]["message"]["content"].as_str().unwrap_or("No response").to_string();
    Ok(AIResponse { content, provider: AIProvider::LMStudio, model })
}
