//! §extra — interactive DB/REPL client history files.
//!
//! `shell_history` scans the shell's history. But database clients and language
//! REPLs keep their OWN history files, and people type connection strings,
//! `\password`, and inline tokens straight into them — `~/.psql_history`,
//! `~/.mysql_history`, `~/.python_history`, `~/.node_repl_history`, etc. These
//! are exactly as readable as shell history and almost never considered.
//!
//! READ-ONLY and value-free: like `shell_history`, this counts secret-looking
//! lines per file and category. It never prints a line or a matched substring.

use regex::Regex;
use serde_json::json;
use std::io::Read;
use std::sync::OnceLock;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;

pub struct ReplHistoryProbe;

struct Patterns {
    /// Connection URIs that embed a password, incl. DB schemes.
    credential_url: Regex,
    /// High-signal token prefixes (same family as shell_history).
    token_prefix: Regex,
    /// Inline password directives (`\password`, `PGPASSWORD=`, `identified by`).
    inline_password: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        credential_url: Regex::new(
            r"(?i)\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis(?:s)?|amqps?|mssql|jdbc:[a-z]+)://[^/\s:@]+:[^@\s]+@",
        )
        .unwrap(),
        token_prefix: Regex::new(r"ghp_|github_pat_|sk-ant-|sk-|AKIA|ASIA|xox[bporas]-|npm_|glpat-|hf_").unwrap(),
        inline_password: Regex::new(
            r"(?i)(PGPASSWORD\s*=|MYSQL_PWD\s*=|identified\s+by\s+|\\password\b|--password[=\s])",
        )
        .unwrap(),
    })
}

/// Home-relative DB-client and REPL history files.
const HISTORY_FILES: &[&str] = &[
    ".psql_history",
    ".mysql_history",
    ".dbshell",            // mongo shell
    ".rediscli_history",
    ".sqlite_history",
    ".duckdb_history",
    ".python_history",
    ".node_repl_history",
    ".irb_history",        // ruby
    ".php_history",
    ".config/pgcli/history",
    ".local/share/mycli/history",
];

impl Probe for ReplHistoryProbe {
    fn id(&self) -> &'static str {
        "credentials.repl_history"
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
                    "REPL history not checked (home unknown)",
                    Severity::Info,
                    Confidence::Unknown,
                )])
            }
        };

        let p = patterns();
        let cap = ctx.limits.max_history_bytes_per_file as usize;
        let mut files_json: Vec<serde_json::Value> = Vec::new();
        let mut total = 0usize;

        for rel in HISTORY_FILES {
            let path = home.join(rel);
            let mut f = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let mut buf = Vec::new();
            if f.by_ref().take(cap as u64).read_to_end(&mut buf).is_err() {
                continue;
            }
            let text = String::from_utf8_lossy(&buf);

            let (mut url_n, mut prefix_n, mut pw_n) = (0usize, 0usize, 0usize);
            for line in text.lines() {
                if p.credential_url.is_match(line) {
                    url_n += 1;
                }
                if p.token_prefix.is_match(line) {
                    prefix_n += 1;
                }
                if p.inline_password.is_match(line) {
                    pw_n += 1;
                }
            }
            let matches = url_n + prefix_n + pw_n;
            if matches == 0 {
                continue;
            }
            total += matches;
            let mut categories = Vec::new();
            if url_n > 0 {
                categories.push("connection_url");
            }
            if prefix_n > 0 {
                categories.push("token_prefix");
            }
            if pw_n > 0 {
                categories.push("inline_password");
            }
            files_json.push(json!({
                "path": shorten(&path, Some(&home)),
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
            "no secret-looking lines in DB/REPL client histories".to_string()
        } else {
            format!(
                "{total} secret-looking line(s) across {} DB/REPL history file(s)",
                files_json.len()
            )
        };

        Ok(vec![Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            if total > 0 {
                "DB/REPL client history exposes secret-looking lines"
            } else {
                "no secrets in DB/REPL client history"
            },
            severity,
            Confidence::Likely,
        )
        .summary(summary)
        .evidence(json!({ "files": files_json, "total_matches": total }))
        .remediation(&[
            "Don't paste connection strings / passwords into psql/mysql/REPL prompts; they persist in per-client history files.",
            "Keep client history files out of agent scope, or disable history for sensitive sessions.",
        ])])
    }
}
