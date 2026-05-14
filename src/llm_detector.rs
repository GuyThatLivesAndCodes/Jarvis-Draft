use crate::models::LLMStatus;

pub async fn check_llm_status() -> LLMStatus {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    let (ollama_ok, ollama_models) = check_ollama(&client).await;
    let (lm_ok,     lm_models)    = check_lm_studio(&client).await;

    LLMStatus {
        ollama_available:    ollama_ok,
        lm_studio_available: lm_ok,
        ollama_models,
        lm_studio_models:    lm_models,
    }
}

async fn check_ollama(client: &reqwest::Client) -> (bool, Vec<String>) {
    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) if r.status().is_success() => {
            let models = r.json::<serde_json::Value>().await
                .ok()
                .and_then(|v| v.get("models")?.as_array().cloned())
                .map(|arr| arr.iter()
                    .filter_map(|m| m.get("name")?.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();
            (true, models)
        }
        _ => (false, vec![]),
    }
}

async fn check_lm_studio(client: &reqwest::Client) -> (bool, Vec<String>) {
    match client.get("http://localhost:1234/api/models").send().await {
        Ok(r) if r.status().is_success() => {
            let models = r.json::<serde_json::Value>().await
                .ok()
                .and_then(|v| v.get("data")?.as_array().cloned())
                .map(|arr| arr.iter()
                    .filter_map(|m| m.get("id")?.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();
            (true, models)
        }
        _ => (false, vec![]),
    }
}
