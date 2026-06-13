//! §23.3 — the frozen INPUT contract: `AgentEvent` / `SessionTrace`.
//!
//! Field names and serde tags are **frozen** so the engine, hook, fixtures,
//! parsers, and dashboard all agree. Events carry **paths / commands / hosts
//! only — never file contents or secret values** (§4.2, §23.11). The optional
//! value-bearing fields (`FileWrite.diff`, `McpCall.input`, `Approval.reason`)
//! exist for hook/fixture inputs but are dropped/redacted at `normalize.rs`
//! (Layer 1); the slurper's Layer-0 extractor never populates them (§24.2.2).

use serde::{Deserialize, Serialize};

/// One observed agent action. Serde-tagged so transcripts/fixtures are stable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    FileRead {
        path: String,
    },
    /// `diff` is OPTIONAL on input and DROPPED in `normalize.rs` before it can
    /// reach scoring, evidence, or any renderer (§4.2/§4.3, §23.11).
    FileWrite {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        diff: Option<String>,
    },
    ShellCommand {
        command: String,
    },
    NetworkAccess {
        host: String,
        port: u16,
    },
    /// `reason` is OPTIONAL human-typed free text — swept in `normalize.rs`;
    /// never scored.
    Approval {
        approved_by: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// `input` is OPTIONAL — dropped/redacted in `normalize.rs`; only
    /// `server`/`tool` survive.
    McpCall {
        server: String,
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
    },
}

/// Frozen INPUT; (de)serialized from checked-in `traces/*.json` and parsed
/// transcripts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionTrace {
    pub session_id: String,
    /// "claude-code" | "codex" | "cursor" | "mock" …
    pub agent: String,
    /// repo slug / shortened label (never an absolute `$HOME` path).
    pub repo: Option<String>,
    /// RFC3339, optional.
    pub started_at: Option<String>,
    pub events: Vec<AgentEvent>,
    /// drives `privileged_user` ×1.2 (§23.7).
    #[serde(default)]
    pub privileged_user: bool,
    /// drives `after_hours` ×1.1 (§23.7).
    #[serde(default)]
    pub after_hours: bool,
}

impl SessionTrace {
    /// Load a `SessionTrace` from a blastradius trace JSON file. (Transcript
    /// auto-detection/normalization lands in the discovery phase; this is the
    /// native-fixture path only.)
    pub fn from_json_str(s: &str) -> Result<SessionTrace, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SessionTrace {
        SessionTrace {
            session_id: "s-1".to_string(),
            agent: "mock".to_string(),
            repo: Some("blastradius".to_string()),
            started_at: Some("2026-06-10T09:00:00Z".to_string()),
            events: vec![
                AgentEvent::FileRead {
                    path: "~/.aws/credentials".to_string(),
                },
                AgentEvent::FileWrite {
                    path: ".github/workflows/deploy.yml".to_string(),
                    diff: None,
                },
                AgentEvent::ShellCommand {
                    command: "git push".to_string(),
                },
                AgentEvent::NetworkAccess {
                    host: "[custom egress target]".to_string(),
                    port: 443,
                },
                AgentEvent::Approval {
                    approved_by: "user".to_string(),
                    reason: None,
                },
                AgentEvent::McpCall {
                    server: "local".to_string(),
                    tool: "search".to_string(),
                    input: None,
                },
            ],
            privileged_user: false,
            after_hours: false,
        }
    }

    #[test]
    fn session_trace_json_roundtrip() {
        let t = sample();
        let json = serde_json::to_string(&t).expect("serialize");
        let back: SessionTrace = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, back);
    }

    #[test]
    fn checked_in_fixtures_parse() {
        for name in ["benign", "risky"] {
            let path = format!("{}/traces/{name}.json", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {path}: {e}"));
            let t = SessionTrace::from_json_str(&text)
                .unwrap_or_else(|e| panic!("parse {path}: {e}"));
            // round-trips cleanly
            let json = serde_json::to_string(&t).expect("serialize");
            let back: SessionTrace = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(t, back);
        }
    }
}
