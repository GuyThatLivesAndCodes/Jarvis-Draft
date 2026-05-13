use crate::models::{AIProvider, AIQuery, AIResponse};

pub struct APIClient;

impl APIClient {
    pub async fn query(
        query: AIQuery,
        api_key: Option<String>,
    ) -> Result<AIResponse, Box<dyn std::error::Error>> {
        match query.provider {
            AIProvider::Anthropic => Self::query_anthropic(query, api_key).await,
            AIProvider::OpenAI => Self::query_openai(query, api_key).await,
            AIProvider::XAI => Self::query_xai(query, api_key).await,
            AIProvider::Ollama => Self::query_ollama(query).await,
            AIProvider::LMStudio => Self::query_lm_studio(query).await,
        }
    }

    async fn query_anthropic(
        query: AIQuery,
        api_key: Option<String>,
    ) -> Result<AIResponse, Box<dyn std::error::Error>> {
        let api_key = api_key.ok_or("Missing Anthropic API key")?;
        let model = query.model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

        let body = serde_json::json!({
            "model": model,
            "max_tokens": 1024,
            "messages": query.messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>()
        });

        let response = reqwest::Client::new()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let content = body
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(AIResponse {
            content,
            provider: AIProvider::Anthropic,
            model,
        })
    }

    async fn query_openai(
        query: AIQuery,
        api_key: Option<String>,
    ) -> Result<AIResponse, Box<dyn std::error::Error>> {
        let api_key = api_key.ok_or("Missing OpenAI API key")?;
        let model = query.model.unwrap_or_else(|| "gpt-4o-mini".to_string());

        let body = serde_json::json!({
            "model": model,
            "messages": query.messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>()
        });

        let response = reqwest::Client::new()
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let content = body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(AIResponse {
            content,
            provider: AIProvider::OpenAI,
            model,
        })
    }

    async fn query_xai(
        query: AIQuery,
        api_key: Option<String>,
    ) -> Result<AIResponse, Box<dyn std::error::Error>> {
        let api_key = api_key.ok_or("Missing xAI API key")?;
        let model = query.model.unwrap_or_else(|| "grok-2".to_string());

        let body = serde_json::json!({
            "model": model,
            "messages": query.messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>()
        });

        let response = reqwest::Client::new()
            .post("https://api.x.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let content = body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(AIResponse {
            content,
            provider: AIProvider::XAI,
            model,
        })
    }

    async fn query_ollama(query: AIQuery) -> Result<AIResponse, Box<dyn std::error::Error>> {
        let model = query.model.unwrap_or_else(|| "qwen2".to_string());

        let messages_str = query
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let body = serde_json::json!({
            "model": model,
            "prompt": messages_str,
            "stream": false
        });

        let response = reqwest::Client::new()
            .post("http://localhost:11434/api/generate")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let content = body
            .get("response")
            .and_then(|r| r.as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(AIResponse {
            content,
            provider: AIProvider::Ollama,
            model,
        })
    }

    async fn query_lm_studio(query: AIQuery) -> Result<AIResponse, Box<dyn std::error::Error>> {
        let model = query.model.unwrap_or_else(|| "default".to_string());

        let body = serde_json::json!({
            "model": model,
            "messages": query.messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>(),
            "temperature": 0.7
        });

        let response = reqwest::Client::new()
            .post("http://localhost:1234/v1/chat/completions")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let content = body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(AIResponse {
            content,
            provider: AIProvider::LMStudio,
            model,
        })
    }
}
