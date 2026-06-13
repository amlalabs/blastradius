//! §12.5 — environment-variable secret NAMES (never values). Curated-first;
//! the broad regex is always on and reported at most `Notable`.

use regex::Regex;
use serde_json::json;
use std::sync::OnceLock;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct EnvProbe;

/// High-signal curated names that drive `Exposed` (§12.5).
const CURATED: &[&str] = &[
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_PAT",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "SLACK_BOT_TOKEN",
    "DATABASE_URL",
    "SUPABASE_SERVICE_ROLE_KEY",
    "STRIPE_SECRET_KEY",
    "HF_TOKEN",
    "GITLAB_TOKEN",
    "DIGITALOCEAN_TOKEN",
    "CLOUDFLARE_API_TOKEN",
    "HOMEBREW_GITHUB_API_TOKEN",
    // Cloud / cluster / CI secrets (parity with the dedicated store probes).
    "GOOGLE_API_KEY",
    "AZURE_CLIENT_SECRET",
    "VAULT_TOKEN",
    "DOCKER_AUTH_CONFIG",
    "CI_JOB_TOKEN",
    "SENTRY_AUTH_TOKEN",
    "DATADOG_API_KEY",
    "TWILIO_AUTH_TOKEN",
    "DOPPLER_TOKEN",
];

fn broad_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY|PRIVATE[_-]?KEY|CREDENTIAL|AUTH|BEARER)",
        )
        .unwrap()
    })
}

/// Known-non-secret keys to always suppress (§12.5).
fn is_suppressed(key: &str) -> bool {
    matches!(key, "SSH_AUTH_SOCK" | "GPG_TTY" | "LESSKEY" | "KEYMAP")
        || key.starts_with("XDG_")
        || key.starts_with("LC_")
        || key.starts_with("TERM")
}

impl Probe for EnvProbe {
    fn id(&self) -> &'static str {
        "env.secret_names"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let mut curated_hits: Vec<serde_json::Value> = Vec::new();
        let mut broad_hits: Vec<serde_json::Value> = Vec::new();

        for var in &ctx.env.vars {
            let key = var.key.as_str();
            if is_suppressed(key) {
                continue;
            }
            if CURATED.contains(&key) {
                curated_hits.push(json!({ "key": key, "value_len": var.value_len }));
            } else if ctx.options.env_broad && broad_re().is_match(key) {
                broad_hits.push(json!({ "key": key, "value_len": var.value_len }));
            }
        }

        let (severity, via, matches, count) = if !curated_hits.is_empty() {
            (
                Severity::Exposed,
                "curated",
                curated_hits.clone(),
                curated_hits.len(),
            )
        } else if !broad_hits.is_empty() {
            (
                Severity::Notable,
                "broad",
                broad_hits.clone(),
                broad_hits.len(),
            )
        } else {
            (Severity::Info, "none", Vec::new(), 0)
        };

        let listing: Vec<String> = matches
            .iter()
            .map(|m| {
                format!(
                    "{}({})",
                    m.get("key").and_then(|k| k.as_str()).unwrap_or("?"),
                    m.get("value_len").and_then(|l| l.as_u64()).unwrap_or(0)
                )
            })
            .collect();

        let title = match severity {
            Severity::Exposed => "secret-like env vars reachable (curated)",
            Severity::Notable => "possibly-secret env vars reachable (heuristic)",
            Severity::Info => "no secret-like env vars reachable",
        };

        let summary = if listing.is_empty() {
            "no secret-named environment variables present".to_string()
        } else {
            format!("secret-like env vars reachable: {}", listing.join(", "))
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(summary)
        .evidence(json!({ "matches": matches, "count": count, "via": via }))
        .remediation(&[
            "Inject only the secrets a task needs, scoped to that task.",
            "Don't pass your full shell environment into agent processes.",
        ]);

        Ok(vec![finding])
    }
}
