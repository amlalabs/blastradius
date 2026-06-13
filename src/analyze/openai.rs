//! Minimal OpenAI Chat Completions client (blocking, via `ureq`).
//!
//! Used ONLY by the opt-in `dashboard --ai` path. The caller is responsible for
//! ensuring the request body is value-free (see [`super::redaction_guard`]); this
//! module just performs the HTTPS POST and returns the assistant message text.

use std::time::Duration;

use anyhow::{anyhow, Context as _, Result};
use serde_json::{json, Value};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const TIMEOUT: Duration = Duration::from_secs(90);

/// Send a system+user prompt and return the assistant's message content.
/// `api_key` is used only as the bearer token and is never logged or returned.
pub fn chat_json(api_key: &str, model: &str, system: &str, user: &str) -> Result<String> {
    let body = json!({
        "model": model,
        "temperature": 0.55,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });

    let resp = ureq::post(ENDPOINT)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .timeout(TIMEOUT)
        .send_json(body);

    let value: Value = match resp {
        Ok(r) => r.into_json().context("decoding OpenAI response")?,
        Err(ureq::Error::Status(code, r)) => {
            // Surface the API error message but never the request/key.
            let detail = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| "no error detail".to_string());
            return Err(anyhow!("OpenAI API returned {code}: {detail}"));
        }
        Err(e) => return Err(anyhow!("OpenAI request failed: {e}")),
    };

    value
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("OpenAI response had no message content"))
}
