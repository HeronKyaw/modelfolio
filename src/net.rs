//! URL policy: HTTPS everywhere except localhost (for Ollama / local proxies).

use url::Url;

/// Validate and normalize a user-entered base URL.
///
/// Rules:
/// - `https` always allowed.
/// - `http` only for `localhost`, `127.0.0.1`, `::1` or `*.localhost`.
/// - No embedded credentials, no query string, no fragment (they never leave config anyway).
pub fn validate_base(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("URL is required".into());
    }
    let u = Url::parse(s).map_err(|_| format!("Not a valid URL: {s}"))?;
    match u.scheme() {
        "https" => {}
        "http" => {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let ok = host == "localhost"
                || host == "127.0.0.1"
                || host == "[::1]"
                || host == "::1"
                || host.ends_with(".localhost");
            if !ok {
                return Err("Plain HTTP is only allowed for localhost — use HTTPS".into());
            }
        }
        _ => return Err("Only http(s) URLs are supported".into()),
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("URLs with embedded credentials are not allowed".into());
    }
    if u.query().is_some() || u.fragment().is_some() {
        return Err("URLs with a query string or fragment are not allowed".into());
    }
    let mut out = u.as_str().to_string();
    while out.ends_with('/') {
        out.pop();
    }
    Ok(out)
}

/// Build the concrete endpoint for a gateway kind from a validated base URL.
pub fn endpoint(kind: crate::providers::GatewayKind, base: &str) -> Result<String, String> {
    let b = validate_base(base)?;
    Ok(match kind {
        crate::providers::GatewayKind::OpenaiCompat => {
            if b.ends_with("/v1") {
                format!("{b}/models")
            } else {
                format!("{b}/v1/models")
            }
        }
        crate::providers::GatewayKind::Ollama => format!("{b}/api/tags"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::GatewayKind;

    #[test]
    fn https_ok() {
        assert_eq!(
            validate_base("https://openrouter.ai/api/").unwrap(),
            "https://openrouter.ai/api"
        );
    }

    #[test]
    fn localhost_http_ok() {
        assert!(validate_base("http://localhost:11434").is_ok());
        assert!(validate_base("http://127.0.0.1:8080/").is_ok());
        assert!(validate_base("http://[::1]:11434").is_ok());
    }

    #[test]
    fn plain_http_rejected() {
        assert!(validate_base("http://example.com/v1").is_err());
    }

    #[test]
    fn query_rejected() {
        assert!(validate_base("https://x.com/v1?key=1").is_err());
    }

    #[test]
    fn credentials_rejected() {
        assert!(validate_base("https://user:pw@example.com").is_err());
    }

    #[test]
    fn openai_endpoint() {
        assert_eq!(
            endpoint(GatewayKind::OpenaiCompat, "https://openrouter.ai/api").unwrap(),
            "https://openrouter.ai/api/v1/models"
        );
        assert_eq!(
            endpoint(GatewayKind::OpenaiCompat, "https://api.groq.com/v1").unwrap(),
            "https://api.groq.com/v1/models"
        );
    }

    #[test]
    fn ollama_endpoint() {
        assert_eq!(
            endpoint(GatewayKind::Ollama, "http://localhost:11434").unwrap(),
            "http://localhost:11434/api/tags"
        );
    }
}
