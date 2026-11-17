//! Ollama `/api/tags` (no key).

use super::ModelItem;
use serde::Deserialize;

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: Option<String>,
    model: Option<String>,
    #[serde(default)]
    digest: Option<String>,
}

pub fn parse(text: &str) -> Result<Vec<ModelItem>, String> {
    let parsed: TagsResponse =
        serde_json::from_str(text).map_err(|e| format!("unexpected response format: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for m in parsed.models {
        // Newer Ollama returns both `model` and `name` with the same value.
        let id = m.model.clone().or_else(|| m.name.clone()).or_else(|| m.digest.clone());
        if let Some(id) = id {
            if !id.is_empty() && seen.insert(id.clone()) {
                out.push(ModelItem { id, owner: None, api_created: None });
            }
        }
    }
    Ok(out)
}
