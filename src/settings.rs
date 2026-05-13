use crate::models::{APICredentials, AIProvider};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub api_credentials: Vec<APICredentials>,
    pub enable_admin_mode: bool,
    pub admin_password: String,
    pub preferred_local_llm: Option<String>,
    pub auto_switch_to_local: bool,
    pub notification_on_local_available: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_credentials: vec![],
            enable_admin_mode: false,
            admin_password: String::new(),
            preferred_local_llm: None,
            auto_switch_to_local: true,
            notification_on_local_available: true,
        }
    }
}

impl Settings {
    fn config_dir() -> PathBuf {
        let base = if cfg!(target_os = "macos") {
            dirs::home_dir()
                .map(|h| h.join("Library/Application Support"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        } else if cfg!(target_os = "windows") {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from(""))
        } else {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
        };

        let config = base.join("jarvis");
        let _ = fs::create_dir_all(&config);
        config
    }

    fn settings_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::settings_path();
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let settings: Settings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(Settings::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::settings_path();
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn set_api_key(&mut self, provider: AIProvider, api_key: String) {
        if let Some(cred) = self
            .api_credentials
            .iter_mut()
            .find(|c| std::mem::discriminant(&c.provider) == std::mem::discriminant(&provider))
        {
            cred.api_key = api_key;
        } else {
            self.api_credentials.push(APICredentials {
                provider,
                api_key,
                enabled: true,
            });
        }
    }

    pub fn get_api_key(&self, provider: &AIProvider) -> Option<String> {
        self.api_credentials
            .iter()
            .find(|c| std::mem::discriminant(&c.provider) == std::mem::discriminant(provider))
            .and_then(|c| if c.enabled { Some(c.api_key.clone()) } else { None })
    }

    pub fn set_enabled(&mut self, provider: &AIProvider, enabled: bool) {
        if let Some(cred) = self
            .api_credentials
            .iter_mut()
            .find(|c| std::mem::discriminant(&c.provider) == std::mem::discriminant(provider))
        {
            cred.enabled = enabled;
        }
    }
}
