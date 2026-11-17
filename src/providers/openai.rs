//! OpenAI-compatible `/v1/models` (OpenRouter, Groq, Together, LiteLLM, Fireworks, …).

use super::ModelItem;
use serde::Deserialize;

#[derive(Deserialize)]
struct OpenAiModels {
    #[serde(default)]
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
    #[serde(default)]
    created: Option<i64>,
}

pub fn parse(text: &str) -> Result<Vec<ModelItem>, String> {
    let parsed: OpenAiModels =
        serde_json::from_str(text).map_err(|e| format!("unexpected response format: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for m in parsed.data {
        if m.id.is_empty() || !seen.insert(m.id.clone()) {
            continue;
        }
        out.push(ModelItem { id: m.id, owner: m.owned_by, api_created: m.created });
    }
    Ok(out)
}
