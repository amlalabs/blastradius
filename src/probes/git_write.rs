//! §12.10 — git remotes & push LIKELIHOOD (never "can push"; no push, no dry-run).
//! Infers push likelihood from local credential sources matching remote hosts.

use serde_json::json;
use std::path::Path;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::read::read_to_string_capped;

pub struct GitWriteProbe;

const MAX_CREDENTIAL_SOURCE_BYTES: u64 = 4 * 1024 * 1024;

/// Does ~/.ssh contain at least one readable private key?
fn has_readable_ssh_key(ctx: &Context) -> bool {
    let dir = match &ctx.home {
        Some(h) => h.join(".ssh"),
        None => return false,
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.ends_with(".pub") || matches!(name, "known_hosts" | "authorized_keys" | "config") {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if !meta.is_file() {
                continue;
            }
        }
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open(&path) {
            let mut buf = [0u8; 256];
            let n = f.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            if head.contains("-----BEGIN") && head.contains("PRIVATE KEY-----") {
                return true;
            }
        }
    }
    false
}

/// Hosts that have a gh token source or a git-credential / netrc entry.
fn credential_hosts(ctx: &Context) -> Vec<String> {
    let mut hosts = Vec::new();
    if let Some(home) = &ctx.home {
        // .git-credentials hosts.
        if let Ok(text) =
            read_to_string_capped(&home.join(".git-credentials"), MAX_CREDENTIAL_SOURCE_BYTES)
        {
            for line in text.lines() {
                if let (Some(h), _) = crate::util::parse::git_remote_host_protocol(line.trim()) {
                    if !hosts.contains(&h) {
                        hosts.push(h);
                    }
                }
            }
        }
        // .netrc machines.
        for nm in [".netrc", "_netrc"] {
            let p: &Path = &home.join(nm);
            if let Ok(text) = read_to_string_capped(p, MAX_CREDENTIAL_SOURCE_BYTES) {
                let toks: Vec<&str> = text.split_whitespace().collect();
                let mut i = 0;
                while i < toks.len() {
                    if toks[i] == "machine" && i + 1 < toks.len() {
                        let m = toks[i + 1].to_string();
                        if !hosts.contains(&m) {
                            hosts.push(m);
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        // gh hosts.yml.
        for rel in [
            ".config/gh/hosts.yml",
            "Library/Application Support/GitHub CLI/hosts.yml",
        ] {
            if let Ok(text) = read_to_string_capped(&home.join(rel), MAX_CREDENTIAL_SOURCE_BYTES) {
                if let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str(&text) {
                    for (k, v) in map {
                        if v.get("oauth_token").is_some() {
                            if let Some(h) = k.as_str() {
                                if !hosts.contains(&h.to_string()) {
                                    hosts.push(h.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    hosts
}

impl Probe for GitWriteProbe {
    fn id(&self) -> &'static str {
        "git.push_likelihood"
    }
    fn class(&self) -> FindingClass {
        FindingClass::GitWrite
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        if !ctx.git.is_repo || ctx.git.remotes.is_empty() {
            return Ok(vec![Finding::new(
                self.id(),
                self.class(),
                FindingScope::CurrentRepo,
                "no git remotes configured",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary(if ctx.git.is_repo {
                "repository has no remotes".to_string()
            } else {
                "not a git repository".to_string()
            })]);
        }

        let ssh_keys = has_readable_ssh_key(ctx);
        let cred_hosts = credential_hosts(ctx);
        let env_github = ctx.env.contains("GITHUB_TOKEN") || ctx.env.contains("GH_TOKEN");

        let mut basis: Vec<String> = Vec::new();
        let mut likely = false;

        let remotes_json: Vec<serde_json::Value> = ctx
            .git
            .remotes
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "host": r.host,
                    "protocol": r.protocol,
                })
            })
            .collect();

        for r in &ctx.git.remotes {
            let host = r.host.clone().unwrap_or_default();
            let is_ssh = r.protocol.as_deref() == Some("ssh");

            if is_ssh && ssh_keys {
                likely = true;
                push_unique(&mut basis, "ssh_remote");
                push_unique(&mut basis, "readable_ssh_private_keys");
            }
            let is_github = host == "github.com";
            if is_github && env_github {
                likely = true;
                push_unique(&mut basis, "github_token_env");
            }
            if !host.is_empty() && cred_hosts.iter().any(|h| h == &host) {
                likely = true;
                push_unique(&mut basis, "credential_source_host_match");
            }
        }

        let (push_likelihood, confidence, severity) = if likely {
            ("likely", Confidence::Likely, Severity::Exposed)
        } else {
            ("unknown", Confidence::Unknown, Severity::Notable)
        };

        let host_list: Vec<String> = ctx
            .git
            .remotes
            .iter()
            .filter_map(|r| {
                r.host.as_ref().map(|h| {
                    format!(
                        "{} {} over {}",
                        r.name,
                        h,
                        r.protocol.as_deref().unwrap_or("?")
                    )
                })
            })
            .collect();

        let summary = if likely {
            format!(
                "push likelihood: likely — {}",
                basis.join(", ").replace('_', " ")
            )
        } else {
            "push likelihood: unknown — remotes exist, no local credential source detected"
                .to_string()
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::CurrentRepo,
            "git remotes reachable",
            severity,
            confidence,
        )
        .summary(summary)
        .evidence(json!({
            "remotes": remotes_json,
            "remote_hosts": host_list,
            "push_likelihood": push_likelihood,
            "basis": basis,
            "current_branch": ctx.git.current_branch,
            "default_branch_guess": ctx.git.default_branch_guess,
            "branch_protection": "server-side; not verified by this local scan",
        }))
        .remediation(&[
            "Branch protection, review, and token scopes are enforced server-side — keep them strict.",
            "Give agents scoped push credentials, not your full git identity.",
        ]);

        Ok(vec![finding])
    }
}

fn push_unique(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}
