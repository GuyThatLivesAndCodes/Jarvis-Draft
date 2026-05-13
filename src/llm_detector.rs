use crate::models::LLMStatus;
use chrono::Local;

pub struct LLMDetector {
    ollama_available: bool,
    lm_studio_available: bool,
}

impl LLMDetector {
    pub fn new() -> Self {
        LLMDetector {
            ollama_available: false,
            lm_studio_available: false,
        }
    }

    pub async fn check_available(&mut self) -> LLMStatus {
        self.ollama_available = self.check_ollama().await;
        self.lm_studio_available = self.check_lm_studio().await;

        let ollama_models = if self.ollama_available {
            self.get_ollama_models().await.unwrap_or_default()
        } else {
            vec![]
        };

        let lm_studio_models = if self.lm_studio_available {
            self.get_lm_studio_models().await.unwrap_or_default()
        } else {
            vec![]
        };

        LLMStatus {
            ollama_available: self.ollama_available,
            lm_studio_available: self.lm_studio_available,
            ollama_models,
            lm_studio_models,
            last_checked: Local::now().to_rfc3339(),
        }
    }

    async fn check_ollama(&self) -> bool {
        match reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn check_lm_studio(&self) -> bool {
        match reqwest::Client::new()
            .get("http://localhost:1234/api/models")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn get_ollama_models(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let response = reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let models: Vec<String> = body
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    async fn get_lm_studio_models(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let response = reqwest::Client::new()
            .get("http://localhost:1234/api/models")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let models: Vec<String> = body
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}
