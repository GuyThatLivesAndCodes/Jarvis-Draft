use crate::models::{AIProvider, ChatMessage, AIResponse};

pub async fn query(
    provider: AIProvider,
    mut messages: Vec<ChatMessage>,
    api_key:  Option<String>,
) -> Result<AIResponse, String> {
    // Extract system message if present
    let system_msg = if !messages.is_empty() && messages[0].role == "system" {
        Some(messages.remove(0).content)
    } else {
        None
    };

    match provider {
        AIProvider::Anthropic => anthropic(messages, system_msg, api_key).await,
        AIProvider::OpenAI    => openai(messages, api_key).await,
        AIProvider::XAI       => xai(messages, api_key).await,
        AIProvider::Ollama    => ollama(messages).await,
        AIProvider::LMStudio  => lm_studio(messages).await,
    }
}

async fn anthropic(msgs: Vec<ChatMessage>, system: Option<String>, key: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing Anthropic API key")?;
    let model = "claude-3-5-sonnet-20241022".to_string();

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>()
    });

    if let Some(sys) = system {
        body["system"] = serde_json::json!(sys);
    }

    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    // Check for API error
    if let Some(err) = resp.get("error") {
        return Err(format!("API Error: {}", err.to_string()));
    }

    let content = resp["content"]
        .get(0)
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .ok_or("Invalid response format from Anthropic")?
        .to_string();
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

    if let Some(err) = resp.get("error") {
        return Err(format!("OpenAI Error: {}", err.get("message").unwrap_or(&serde_json::json!("Unknown error"))));
    }

    let content = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or("Invalid response format from OpenAI")?
        .to_string();
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

    if let Some(err) = resp.get("error") {
        let err_msg = err.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_else(|| err.as_str().unwrap_or("Check API key and rate limits"));
        return Err(format!("xAI Error: {}", err_msg));
    }

    let content = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or("Invalid response format from xAI")?
        .to_string();
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

    if let Some(err) = resp.get("error") {
        return Err(format!("Ollama Error: {}", err));
    }

    let content = resp.get("response")
        .and_then(|r| r.as_str())
        .ok_or("Invalid response format from Ollama")?
        .to_string();
    Ok(AIResponse { content, provider: AIProvider::Ollama, model })
}

async fn lm_studio(msgs: Vec<ChatMessage>) -> Result<AIResponse, String> {
    let model = "local-model".to_string();
    let body  = serde_json::json!({
        "model": "",
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
        "temperature": 0.7
    });

    let resp: serde_json::Value = reqwest::Client::new()
        .post("http://localhost:1234/v1/chat/completions")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    if let Some(err) = resp.get("error") {
        return Err(format!("LM Studio Error: {}", err.get("message").unwrap_or(&serde_json::json!("Unknown error"))));
    }

    let content = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or("Invalid response format from LM Studio. Make sure a model is loaded and the server is running.")?
        .to_string();
    Ok(AIResponse { content, provider: AIProvider::LMStudio, model })
}
