use crate::models::{AIProvider, ChatMessage, AIResponse, ToolAction};
use crate::tools;

pub async fn query(
    provider: AIProvider,
    mut messages: Vec<ChatMessage>,
    api_key:  Option<String>,
    model_override: Option<String>,
) -> Result<AIResponse, String> {
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
    let mut tool_actions: Vec<ToolAction> = Vec::new();

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
                final_text.push_str(text);
            }

            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                has_tool_use = true;
                let tool_id = block.get("id").and_then(|id| id.as_str()).unwrap_or("");
                let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let tool_input = block.get("input").cloned().unwrap_or(serde_json::json!({}));

                tool_actions.push(ToolAction {
                    name:  tool_name.to_string(),
                    input: tool_input.clone(),
                });

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
            return Ok(AIResponse { content: final_text, provider: AIProvider::Anthropic, model, tool_actions });
        }

        let mut assistant_content = Vec::new();
        for block in content_blocks {
            assistant_content.push(block.clone());
        }
        msgs.push(ChatMessage {
            role: "assistant".to_string(),
            content: serde_json::to_string(&assistant_content).unwrap_or_default(),
        });

        for result in tool_results {
            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: result.to_string(),
            });
        }
    }
}

async fn openai(msgs: Vec<ChatMessage>, key: Option<String>, model_override: Option<String>) -> Result<AIResponse, String> {
    openai_compatible(
        msgs, key, model_override,
        "https://api.openai.com/v1/chat/completions",
        "gpt-4o-mini",
        AIProvider::OpenAI,
        "OpenAI",
    ).await
}

async fn xai(msgs: Vec<ChatMessage>, key: Option<String>, model_override: Option<String>) -> Result<AIResponse, String> {
    openai_compatible(
        msgs, key, model_override,
        "https://api.x.ai/v1/chat/completions",
        "grok-4.3",
        AIProvider::XAI,
        "xAI",
    ).await
}

async fn openai_compatible(
    msgs: Vec<ChatMessage>,
    key: Option<String>,
    model_override: Option<String>,
    url: &str,
    default_model: &str,
    provider: AIProvider,
    provider_label: &str,
) -> Result<AIResponse, String> {
    let key   = key.ok_or(format!("Missing {} API key", provider_label))?;
    let model = model_override.unwrap_or_else(|| default_model.to_string());
    let tools_list = tools::get_tools();
    let mut tool_actions: Vec<ToolAction> = Vec::new();

    let body = serde_json::json!({
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
        .post(url)
        .header("Authorization", format!("Bearer {key}"))
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    if let Some(err) = resp.get("error") {
        let err_msg = err.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_else(|| err.as_str().unwrap_or("Check API key and rate limits"));
        return Err(format!("{} Error: {}", provider_label, err_msg));
    }

    let message = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .ok_or(format!("Invalid response format from {}", provider_label))?;

    let text_content = message.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

    if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
        let mut results_text = Vec::new();
        for fn_call in tool_calls {
            let fn_name = fn_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
            let fn_args_str = fn_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
            let fn_args: serde_json::Value = serde_json::from_str(fn_args_str).unwrap_or(serde_json::json!({}));

            tool_actions.push(ToolAction {
                name:  fn_name.to_string(),
                input: fn_args.clone(),
            });

            match tools::execute_tool(fn_name, fn_args).await {
                Ok(result) => results_text.push(result),
                Err(e)     => results_text.push(format!("Error: {}", e)),
            }
        }

        let content = if text_content.is_empty() {
            results_text.join("\n")
        } else {
            format!("{}\n{}", text_content, results_text.join("\n"))
        };
        return Ok(AIResponse { content, provider, model, tool_actions });
    }

    if text_content.is_empty() {
        return Err(format!("Empty response from {}", provider_label));
    }
    Ok(AIResponse { content: text_content, provider, model, tool_actions })
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
    Ok(AIResponse { content, provider: AIProvider::Ollama, model, tool_actions: vec![] })
}

async fn lm_studio(msgs: Vec<ChatMessage>) -> Result<AIResponse, String> {
    let model = "local-model".to_string();
    let tools_list = tools::get_tools();
    let mut tool_actions: Vec<ToolAction> = Vec::new();

    let body  = serde_json::json!({
        "model": "",
        "messages": msgs.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
        "tools": tools_list.iter().map(|t| serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.input_schema
            }
        })).collect::<Vec<_>>(),
        "tool_choice": "auto",
        "temperature": 0.7
    });

    let resp: serde_json::Value = reqwest::Client::new()
        .post("http://localhost:1234/v1/chat/completions")
        .json(&body).send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    if let Some(err) = resp.get("error") {
        return Err(format!("LM Studio Error: {}", err.get("message").unwrap_or(&serde_json::json!("Unknown error"))));
    }

    let message = resp["choices"]
        .get(0)
        .and_then(|c| c.get("message"))
        .ok_or("Invalid response from LM Studio. Make sure a model is loaded and the server is running.")?;

    let text_content = message.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

    if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
        let mut results_text = Vec::new();
        for fn_call in tool_calls {
            let fn_name = fn_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
            let fn_args_str = fn_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
            let fn_args: serde_json::Value = serde_json::from_str(fn_args_str).unwrap_or(serde_json::json!({}));

            tool_actions.push(ToolAction {
                name:  fn_name.to_string(),
                input: fn_args.clone(),
            });

            match tools::execute_tool(fn_name, fn_args).await {
                Ok(result) => results_text.push(result),
                Err(e)     => results_text.push(format!("Error: {}", e)),
            }
        }

        let content = if text_content.is_empty() {
            results_text.join("\n")
        } else {
            format!("{}\n{}", text_content, results_text.join("\n"))
        };
        return Ok(AIResponse { content, provider: AIProvider::LMStudio, model, tool_actions });
    }

    if text_content.is_empty() {
        return Err("Empty response from LM Studio. Make sure a model is loaded.".to_string());
    }
    Ok(AIResponse { content: text_content, provider: AIProvider::LMStudio, model, tool_actions })
}
