//! §extra — subprocess environment credential-scrubbing awareness.
//!
//! Claude Code can strip a subset of credential env vars before spawning Bash /
//! hook / MCP-stdio subprocesses (gated on `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`).
//! This probe reports, for the environment blastradius itself runs in, whether
//! that scrub flag is set and which *present* curated credential NAMES are known
//! NOT to be covered by the scrub (and therefore still flow to subprocesses).
//!
//! READ-ONLY, std-only, value-free: only the derived scrub boolean, credential
//! NAMES, and counts ever leave this probe — never any value or value length.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct EnvScrubProbe;

/// Credentials the docs describe as "Anthropic and cloud provider" creds — the
/// category the scrub is *expected* to strip. This is a docs-derived heuristic;
/// the authoritative allow/deny list lives in the closed-source CC client, so
/// these are treated as "expected-stripped", never asserted as guaranteed.
/// NOTE: deliberately NOT reused from env::CURATED — CURATED does not contain
/// the cloud/Anthropic identity names below (correction #1).
const SCRUB_COVERED: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_API_KEY",
];

/// Prefix-matched covered families (GCLOUD_*, AZURE_*, CLAUDE_CODE_OAUTH*).
const SCRUB_COVERED_PREFIXES: &[&str] = &["GCLOUD_", "AZURE_", "CLAUDE_CODE_OAUTH"];

/// Curated third-party credentials the scrub does NOT cover — these survive the
/// scrub and still reach every subprocess. OPENAI_API_KEY is here (correction
/// #2): it is in env::CURATED but is neither Anthropic nor a cloud-provider
/// credential, so under the docs' wording it is not scrubbed.
const SCRUB_EXEMPT: &[&str] = &[
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_PAT",
    "GITLAB_TOKEN",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "STRIPE_SECRET_KEY",
    "DATABASE_URL",
    "SUPABASE_SERVICE_ROLE_KEY",
    "SLACK_BOT_TOKEN",
    "HF_TOKEN",
    "CLOUDFLARE_API_TOKEN",
    "DIGITALOCEAN_TOKEN",
    "HOMEBREW_GITHUB_API_TOKEN",
    "OPENAI_API_KEY",
];

fn is_covered(key: &str) -> bool {
    SCRUB_COVERED.contains(&key) || SCRUB_COVERED_PREFIXES.iter().any(|p| key.starts_with(p))
}

impl Probe for EnvScrubProbe {
    fn id(&self) -> &'static str {
        "env.subprocess_scrub"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        // Read the scrub flag as a non-secret config boolean. We read the live
        // process env here (like aws.rs reads AWS_SHARED_CREDENTIALS_FILE) but
        // only ever emit the derived boolean, never the raw value.
        let raw = std::env::var_os("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB");
        let scrub_flag_present = raw.is_some();
        let scrub_active = raw
            .as_ref()
            .and_then(|v| v.to_str())
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        // Classify the PRESENT curated credential names (names only, from the
        // env snapshot — never values, never value lengths).
        let mut covered_present: Vec<String> = Vec::new();
        let mut exempt_present: Vec<String> = Vec::new();
        for var in &ctx.env.vars {
            let key = var.key.as_str();
            if is_covered(key) {
                if !covered_present.contains(&key.to_string()) {
                    covered_present.push(key.to_string());
                }
            } else if SCRUB_EXEMPT.contains(&key) && !exempt_present.contains(&key.to_string()) {
                exempt_present.push(key.to_string());
            }
        }
        covered_present.sort();
        exempt_present.sort();
        let covered_count = covered_present.len();
        let exempt_count = exempt_present.len();
        let total = covered_count + exempt_count;

        // Severity reflects residual env-borne exposure to subprocesses. Capped
        // at Notable deliberately (EnvProbe already raises the raw Exposed flag).
        let (severity, confidence, title, summary) = if !scrub_active {
            if total > 0 {
                (
                    Severity::Notable,
                    Confidence::Confirmed,
                    "subprocess env scrub OFF — credentials flow unscrubbed",
                    format!(
                        "scrub off: {total} curated credential name(s) flow unscrubbed to every Bash subprocess/hook/MCP stdio server"
                    ),
                )
            } else {
                (
                    Severity::Info,
                    Confidence::Confirmed,
                    "subprocess env scrub OFF — no curated credentials present",
                    "scrub off but no curated credential names present in this environment"
                        .to_string(),
                )
            }
        } else if exempt_count > 0 {
            (
                Severity::Notable,
                Confidence::Likely,
                "subprocess env scrub ON — non-covered credentials still reach subprocesses",
                format!(
                    "scrub on, but {exempt_count} non-covered credential name(s) still reach subprocesses (covered/exempt mapping is docs-inferred)"
                ),
            )
        } else {
            (
                Severity::Info,
                Confidence::Confirmed,
                "subprocess env scrub ON — nothing left that scrub misses",
                "scrub on and no known-non-covered credential names present".to_string(),
            )
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            confidence,
        )
        .summary(summary)
        .evidence(json!({
            "scrub_flag_present": scrub_flag_present,
            "scrub_active": scrub_active,
            "scrub_covered_present": covered_present,
            "scrub_exempt_present": exempt_present,
            "covered_count": covered_count,
            "exempt_count": exempt_count,
            "note": "Reflects the environment blastradius runs in. When run outside the agent's Bash tool this is the probe's own env, not necessarily the agent subprocess env. SCRUB_COVERED is docs-inferred (expected-stripped), not a guaranteed list.",
        }))
        .remediation(&[
            "Set CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1 to strip Anthropic/cloud credentials from subprocess environments.",
            "Scrub does not cover generic third-party tokens (GITHUB_TOKEN, NPM_TOKEN, DATABASE_URL, OPENAI_API_KEY, ...): inject those per-task instead of process-wide.",
        ]);

        Ok(vec![finding])
    }
}
