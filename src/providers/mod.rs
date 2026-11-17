pub mod ollama;
pub mod openai;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use crate::store::config::GatewayKind;

/// One model as returned by a gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelItem {
    pub id: String,
    pub owner: Option<String>,
    pub api_created: Option<i64>,
}

/// Fetch the model list for a gateway. Only ever contacts the saved base URL.
pub async fn fetch(
    kind: GatewayKind,
    base: &str,
    key: Option<Zeroizing<String>>,
    http: &reqwest::Client,
) -> Result<Vec<ModelItem>, String> {
    let url = crate::net::endpoint(kind, base)?;
    let mut rb = http
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "modelfolio/0.1");
    if kind == GatewayKind::OpenaiCompat {
        if let Some(k) = &key {
            rb = rb.bearer_auth(k.as_str());
        }
    }
    let resp = rb.send().await.map_err(|e| format!("network error: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let hint = match status.as_u16() {
            401 | 403 => " — check the API key",
            404 => " — wrong base URL for this kind?",
            429 => " — rate limited, try later",
            _ => "",
        };
        return Err(format!("HTTP {}{}", status.as_u16(), hint));
    }
    let text = resp.text().await.map_err(|e| format!("read error: {e}"))?;
    match kind {
        GatewayKind::OpenaiCompat => openai::parse(&text),
        GatewayKind::Ollama => ollama::parse(&text),
    }
}
