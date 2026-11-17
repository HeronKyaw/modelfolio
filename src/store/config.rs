//! Gateway metadata + settings on disk.
//!
//! SECURITY: this file never contains API keys — only a `secret_id` naming an
//! entry in the OS secure store (Credential Manager / Keychain / Secret Service).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePref {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Minutes between background refreshes (min 15, default 60).
    pub background_interval_min: u32,
    /// OS toasts when a *background* fetch finds changes.
    pub background_toasts: bool,
    /// Include removed models in the background toast.
    pub toast_include_removed: bool,
    /// How long a model keeps its "NEW" pill (days).
    pub new_window_days: u32,
    /// Ask before deleting a gateway.
    pub confirm_delete_gateway: bool,
    pub theme: ThemePref,
    /// Accent color as hex, e.g. "#0a84ff".
    pub accent: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            background_interval_min: 60,
            background_toasts: true,
            toast_include_removed: false,
            new_window_days: 7,
            confirm_delete_gateway: true,
            theme: ThemePref::System,
            accent: "#0a84ff".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GatewayKind {
    #[default]
    OpenaiCompat,
    Ollama,
}

impl GatewayKind {
    pub fn label(self) -> &'static str {
        match self {
            GatewayKind::OpenaiCompat => "OpenAI-compatible",
            GatewayKind::Ollama => "Ollama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gateway {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub kind: GatewayKind,
    /// Keyring account for this gateway's key, e.g. "modelfolio/<uuid>". None = no key stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_id: Option<String>,
    #[serde(default = "crate::store::config::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub notes: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub gateways: Vec<Gateway>,
    #[serde(default)]
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self { version: 1, gateways: Vec::new(), settings: Settings::default() }
    }
}

impl Config {
    pub fn load_or_default() -> Self {
        let path = crate::paths::config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("config.json unreadable ({e}); starting fresh (old file kept as .corrupt)");
                    backup_corrupt(&path);
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    /// Atomic write: temp file + rename. Never contains secrets.
    pub fn save(&self) -> Result<(), String> {
        let path = crate::paths::config_path();
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn gateway(&self, id: &str) -> Option<&Gateway> {
        self.gateways.iter().find(|g| g.id == id)
    }
}

fn backup_corrupt(path: &Path) {
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let _ = std::fs::rename(path, path.with_extension(format!("json.corrupt-{ts}")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_no_secret() {
        let mut c = Config::default();
        c.gateways.push(Gateway {
            id: "abc".into(),
            name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api".into(),
            kind: GatewayKind::OpenaiCompat,
            secret_id: Some("modelfolio/abc".into()),
            enabled: true,
            notes: "".into(),
        });
        let json = serde_json::to_string(&c).unwrap();
        // The key must never appear in serialized config by construction.
        assert!(json.contains("modelfolio/abc"));
        assert!(!json.to_lowercase().contains("sk-"));
        let c2: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c2.gateways.len(), 1);
    }
}
