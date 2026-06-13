//! §12.3 — GitHub token source, OFFLINE by default. Reads local gh hosts.yml.
//! Never calls GitHub or `gh auth status`. Scopes require network → not checked.

use serde_json::json;
use std::path::PathBuf;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct GithubProbe;

const MAX_GH_HOSTS_BYTES: u64 = 4 * 1024 * 1024;

fn hosts_yml_paths(ctx: &Context) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // XDG_CONFIG_HOME path (Linux + override).
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        out.push(PathBuf::from(xdg).join("gh/hosts.yml"));
    }
    if let Some(home) = &ctx.home {
        out.push(home.join(".config/gh/hosts.yml"));
        // macOS GitHub CLI location.
        out.push(home.join("Library/Application Support/GitHub CLI/hosts.yml"));
    }
    // Dedupe.
    let mut seen = Vec::new();
    out.retain(|p| {
        if seen.contains(p) {
            false
        } else {
            seen.push(p.clone());
            true
        }
    });
    out
}

impl Probe for GithubProbe {
    fn id(&self) -> &'static str {
        "github.token_source"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let mut hosts: Vec<serde_json::Value> = Vec::new();
        let mut skipped_files: Vec<serde_json::Value> = Vec::new();

        for path in hosts_yml_paths(ctx) {
            let text = match read_to_string_capped(&path, MAX_GH_HOSTS_BYTES) {
                Ok(t) => t,
                Err(CappedReadError::NotFound | CappedReadError::NotFile) => continue,
                Err(e) => {
                    skipped_files.push(json!({
                        "path": shorten(&path, ctx.home.as_deref()),
                        "reason": e.reason(),
                    }));
                    continue;
                }
            };
            // hosts.yml maps host -> { user, oauth_token, ... }.
            let parsed: serde_yaml::Value = match serde_yaml::from_str(&text) {
                Ok(v) => v,
                Err(_) => {
                    skipped_files.push(json!({
                        "path": shorten(&path, ctx.home.as_deref()),
                        "reason": "yaml parse error",
                    }));
                    continue;
                }
            };
            if let serde_yaml::Value::Mapping(map) = parsed {
                for (host_key, val) in map {
                    let host = host_key.as_str().unwrap_or("unknown").to_string();
                    let user = val
                        .get("user")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string());
                    let token_len = val
                        .get("oauth_token")
                        .and_then(|t| t.as_str())
                        .map(|t| t.len())
                        .unwrap_or(0);
                    let token_present = token_len > 0;
                    hosts.push(json!({
                        "host": host,
                        "user": user,
                        "token_present": token_present,
                        "token_len": token_len,
                        "scopes_checked": false,
                    }));
                }
            }
        }

        let any_token = hosts
            .iter()
            .any(|h| h.get("token_present").and_then(|v| v.as_bool()) == Some(true));

        let (severity, title) = if any_token {
            (Severity::Exposed, "GitHub token source present")
        } else if !skipped_files.is_empty() {
            (
                Severity::Notable,
                "GitHub CLI host config present (not parsed)",
            )
        } else if !hosts.is_empty() {
            (Severity::Notable, "GitHub CLI host config present")
        } else {
            (Severity::Info, "no GitHub token source reachable")
        };

        let summary = if any_token {
            "GitHub token source present; scopes not checked in offline mode".to_string()
        } else if !skipped_files.is_empty() {
            "GitHub CLI host config present; one or more files were not parsed".to_string()
        } else {
            "no local gh token source found".to_string()
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
        .evidence(json!({ "hosts": hosts, "skipped_files": skipped_files }))
        .remediation(&[
            "Issue scoped, short-lived tokens to agents instead of your gh login.",
            "Avoid mounting ~/.config/gh into agent environments.",
        ]);

        Ok(vec![finding])
    }
}
