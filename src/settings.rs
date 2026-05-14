use crate::models::AIProvider;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APICredential {
    pub provider: AIProvider,
    pub api_key:  String,
    pub enabled:  bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub credentials:                Vec<APICredential>,
    pub auto_switch_to_local:       bool,
    pub notify_on_local_available:  bool,
    pub admin_enabled:              bool,
    pub admin_password:             String,
    pub preferred_local:            Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            credentials:               vec![],
            auto_switch_to_local:      true,
            notify_on_local_available: true,
            admin_enabled:             false,
            admin_password:            String::new(),
            preferred_local:           None,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir  = base.join("jarvis");
        let _    = fs::create_dir_all(&dir);
        dir.join("settings.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        if p.exists() {
            fs::read_to_string(&p)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::path(), json);
        }
    }

    pub fn get_key(&self, provider: &AIProvider) -> Option<&str> {
        self.credentials
            .iter()
            .find(|c| &c.provider == provider && c.enabled && !c.api_key.is_empty())
            .map(|c| c.api_key.as_str())
    }

    pub fn set_key(&mut self, provider: AIProvider, key: String) {
        if let Some(c) = self.credentials.iter_mut().find(|c| c.provider == provider) {
            c.api_key = key;
            c.enabled = true;
        } else {
            self.credentials.push(APICredential { provider, api_key: key, enabled: true });
        }
    }
}
