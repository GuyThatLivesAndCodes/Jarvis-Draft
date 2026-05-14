use crate::models::{AIProvider, ChatMessage, AIResponse};
use crate::tools;

pub async fn query(
    provider: AIProvider,
    mut messages: Vec<ChatMessage>,
    api_key:  Option<String>,
    model_override: Option<String>,
) -> Result<AIResponse, String> {
    // Extract system message if present
    let system_msg = if !messages.is_empty() && messages[0].role == "system" {
        Some(messages.remove(0).content)
    } else {
        None
    };

    match provider {
        AIProvider::Anthropic => anthropic(messages, system_msg, api_key, model_override).await,
        AIProvider::OpenAI    => openai(messages, api_key, model_override).await,
        AIProvider::XAI       => xai(messages, api_key, model_override).await,
        AIProvider::Ollama    => ollama(messages).await,
        AIProvider::LMStudio  => lm_studio(messages).await,
    }
}

async fn anthropic(mut msgs: Vec<ChatMessage>, system: Option<String>, key: Option<String>, model_override: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing Anthropic API key")?;
    let model = model_override.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());
    let tools_list = tools::get_tools();

    loop {
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
            "tools": tools_list.iter().map(|t| serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema
            })).collect::<Vec<_>>()
        });

        if let Some(ref sys) = system {
            body["system"] = serde_json::json!(sys);
        }

        let resp: serde_json::Value = reqwest::Client::new()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .json(&body).send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;

        if let Some(err) = resp.get("error") {
            return Err(format!("API Error: {}", err.to_string()));
        }

        let content_blocks = resp["content"].as_array().ok_or("Invalid response format")?;
        let mut has_tool_use = false;
        let mut final_text = String::new();
        let mut tool_results = Vec::new();

        for block in content_blocks {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                final_text = text.to_string();
            }

            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                has_tool_use = true;
                let tool_id = block.get("id").and_then(|id| id.as_str()).unwrap_or("");
                let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let tool_input = block.get("input").cloned().unwrap_or(serde_json::json!({}));

                match tools::execute_tool(tool_name, tool_input).await {
                    Ok(result) => {
                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result
                        }));
                    }
                    Err(e) => {
                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": format!("Error: {}", e),
                            "is_error": true
                        }));
                    }
                }
            }
        }

        if !has_tool_use {
            return Ok(AIResponse { content: final_text, provider: AIProvider::Anthropic, model });
        }

        // Add assistant response with tool use to messages
        let mut assistant_content = Vec::new();
        for block in content_blocks {
            assistant_content.push(block.clone());
        }
        msgs.push(ChatMessage {
            role: "assistant".to_string(),
            content: serde_json::to_string(&assistant_content).unwrap_or_default(),
        });

        // Add tool results
        for result in tool_results {
            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: result.to_string(),
            });
        }
    }
}

async fn openai(msgs: Vec<ChatMessage>, key: Option<String>, model_override: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing OpenAI API key")?;
    let model = model_override.unwrap_or_else(|| "gpt-4o-mini".to_string());
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

async fn xai(msgs: Vec<ChatMessage>, key: Option<String>, model_override: Option<String>) -> Result<AIResponse, String> {
    let key   = key.ok_or("Missing xAI API key")?;
    let model = model_override.unwrap_or_else(|| "grok-4.3".to_string());
    let tools_list = tools::get_tools();

    let mut body = serde_json::json!({
        "model": model,
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
        "tools": tools_list.iter().map(|t| serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.input_schema
            }
        })).collect::<Vec<_>>(),
        "tool_choice": "auto"
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

    // For simplicity, just get the text response (handle function calls if they appear)
    let message = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .ok_or("Invalid response format from xAI")?;

    // Check if there's a function call
    if let Some(fn_call) = message.get("tool_calls").and_then(|tc| tc.get(0)) {
        let fn_name = fn_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
        let fn_args_str = fn_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
        let fn_args: serde_json::Value = serde_json::from_str(fn_args_str).unwrap_or(serde_json::json!({}));

        match tools::execute_tool(fn_name, fn_args).await {
            Ok(result) => {
                return Ok(AIResponse {
                    content: format!("[Executed: {}]\n{}", fn_name, result),
                    provider: AIProvider::XAI,
                    model
                });
            }
            Err(e) => {
                return Ok(AIResponse {
                    content: format!("[Tool Error: {}]\n{}", fn_name, e),
                    provider: AIProvider::XAI,
                    model
                });
            }
        }
    }

    let content = message
        .get("content")
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
