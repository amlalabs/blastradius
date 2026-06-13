//! §12.7 — shell-history token-pattern COUNTS. Never prints lines or matched
//! substrings — counts by file and category only.

use regex::Regex;
use serde_json::json;
use std::io::Read;
use std::sync::OnceLock;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;

pub struct ShellHistoryProbe;

struct Patterns {
    export_secret: Regex,
    token_prefix: Regex,
    credential_url: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        export_secret: Regex::new(
            r"(?i)\b(export|set|env)\s+[A-Za-z_][A-Za-z0-9_]*(TOKEN|SECRET|PASSWORD|API_KEY|ACCESS_KEY)",
        )
        .unwrap(),
        token_prefix: Regex::new(
            r"ghp_|github_pat_|sk-ant-|sk-proj-|sk-|AKIA|ASIA|xox[bporas]-|npm_|glpat-|hf_|dop_v1_|shpat_|AIza|ya29\.",
        )
        .unwrap(),
        credential_url: Regex::new(r"https?://[^/\s:]+:[^@\s]+@").unwrap(),
    })
}

impl Probe for ShellHistoryProbe {
    fn id(&self) -> &'static str {
        "credentials.shell_history"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = match &ctx.home {
            Some(h) => h.clone(),
            None => {
                return Ok(vec![Finding::new(
                    self.id(),
                    self.class(),
                    FindingScope::Ambient,
                    "shell history not checked (home unknown)",
                    Severity::Info,
                    Confidence::Unknown,
                )])
            }
        };

        let candidates = [
            home.join(".zsh_history"),
            home.join(".bash_history"),
            home.join(".history"),
            home.join(".local/share/fish/fish_history"),
        ];

        let p = patterns();
        let mut files_json: Vec<serde_json::Value> = Vec::new();
        let mut total = 0usize;

        for path in candidates {
            let mut f = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            // Bounded read respecting the size cap.
            let mut buf = Vec::new();
            let cap = ctx.limits.max_history_bytes_per_file as usize;
            if f.by_ref().take(cap as u64).read_to_end(&mut buf).is_err() {
                continue;
            }
            let text = String::from_utf8_lossy(&buf);

            let mut export_n = 0usize;
            let mut prefix_n = 0usize;
            let mut url_n = 0usize;
            for line in text.lines() {
                if p.export_secret.is_match(line) {
                    export_n += 1;
                }
                if p.token_prefix.is_match(line) {
                    prefix_n += 1;
                }
                if p.credential_url.is_match(line) {
                    url_n += 1;
                }
            }
            let matches = export_n + prefix_n + url_n;
            if matches == 0 {
                continue;
            }
            total += matches;
            let mut categories = Vec::new();
            if prefix_n > 0 {
                categories.push("token_prefix");
            }
            if export_n > 0 {
                categories.push("export_secret");
            }
            if url_n > 0 {
                categories.push("credential_url");
            }
            files_json.push(json!({
                "path": shorten(&path, ctx.home.as_deref()),
                "matches": matches,
                "categories": categories,
            }));
        }

        let severity = if total >= 3 {
            Severity::Exposed
        } else if total > 0 {
            Severity::Notable
        } else {
            Severity::Info
        };

        let summary = if total == 0 {
            "no secret-looking lines found in shell history".to_string()
        } else {
            format!(
                "shell history contains {total} secret-looking line(s) across {} file(s)",
                files_json.len()
            )
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            if total > 0 {
                "shell history exposes secret-looking lines"
            } else {
                "no secrets in shell history"
            },
            severity,
            Confidence::Likely,
        )
        .summary(summary)
        .evidence(json!({ "files": files_json, "total_matches": total }))
        .remediation(&[
            "Avoid passing secrets as inline command arguments; they persist in history.",
            "Agents reading shell history can recover these — keep history out of agent scope.",
        ]);

        Ok(vec![finding])
    }
}
