//! §12.4 — git credential store. Reads ~/.git-credentials, ~/.netrc, and global
//! credential.helper. Reports host/helper names + presence only — never URLs.

use serde_json::json;
use std::path::Path;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::command::run_stdout;
use crate::util::parse::git_remote_host_protocol;
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct GitCredentialsProbe;

const MAX_GIT_CREDENTIAL_FILE_BYTES: u64 = 4 * 1024 * 1024;

// Recognized credential helpers (§12.4): store, cache, osxkeychain, manager,
// manager-core, libsecret, wincred. We record whatever helper is configured.

/// Parse hosts from ~/.git-credentials lines (`https://user:pass@host`),
/// returning host + presence flags only.
fn parse_git_credentials(text: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (host, _proto) = git_remote_host_protocol(line);
        let host = match host {
            Some(h) => h,
            None => continue,
        };
        // Determine presence of username/password from the userinfo segment.
        let mut username_present = false;
        let mut password_present = false;
        if let Some(scheme_end) = line.find("://") {
            let rest = &line[scheme_end + 3..];
            if let Some(at) = rest.find('@') {
                let userinfo = &rest[..at];
                if let Some(colon) = userinfo.find(':') {
                    username_present = colon > 0;
                    password_present = colon + 1 < userinfo.len();
                } else {
                    username_present = !userinfo.is_empty();
                }
            }
        }
        if !out
            .iter()
            .any(|v: &serde_json::Value| v.get("host").and_then(|h| h.as_str()) == Some(&host))
        {
            out.push(json!({
                "host": host,
                "username_present": username_present,
                "password_present": password_present,
            }));
        }
    }
    out
}

/// Parse machine names from a .netrc, plus whether login/password fields appear.
fn parse_netrc(text: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "machine" && i + 1 < tokens.len() {
            let machine = tokens[i + 1].to_string();
            let mut login = false;
            let mut password = false;
            // Scan until the next `machine`/`default` keyword.
            let mut j = i + 2;
            while j < tokens.len() && tokens[j] != "machine" && tokens[j] != "default" {
                match tokens[j] {
                    "login" => login = true,
                    "password" => password = true,
                    _ => {}
                }
                j += 1;
            }
            out.push(json!({
                "machine": machine,
                "login_present": login,
                "password_present": password,
            }));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

impl Probe for GitCredentialsProbe {
    fn id(&self) -> &'static str {
        "git.credential_store"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = ctx.home.clone();

        let mut stored_hosts: Vec<serde_json::Value> = Vec::new();
        let mut netrc_hosts: Vec<serde_json::Value> = Vec::new();
        let mut helpers: Vec<String> = Vec::new();
        let mut skipped_files: Vec<serde_json::Value> = Vec::new();
        let mut plaintext_creds = false;

        if let Some(home) = &home {
            // Plaintext store lives at ~/.git-credentials by default, but git
            // also honors the XDG path ($XDG_CONFIG_HOME/git/credentials).
            let mut credential_paths = vec![home.join(".git-credentials")];
            match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                Some(xdg) => credential_paths.push(std::path::PathBuf::from(xdg).join("git/credentials")),
                None => credential_paths.push(home.join(".config/git/credentials")),
            }
            for git_credentials_path in credential_paths {
                match read_to_string_capped(&git_credentials_path, MAX_GIT_CREDENTIAL_FILE_BYTES) {
                    Ok(text) => {
                        for entry in parse_git_credentials(&text) {
                            plaintext_creds = true;
                            let host = entry.get("host").and_then(|h| h.as_str());
                            let dup = host.is_some()
                                && stored_hosts
                                    .iter()
                                    .any(|h| h.get("host").and_then(|x| x.as_str()) == host);
                            if !dup {
                                stored_hosts.push(entry);
                            }
                        }
                    }
                    Err(CappedReadError::NotFound | CappedReadError::NotFile) => {}
                    Err(e) => {
                        skipped_files.push(json!({
                            "path": shorten(&git_credentials_path, Some(home)),
                            "reason": e.reason(),
                        }));
                    }
                }
            }
            for nm in [".netrc", "_netrc"] {
                let p: &Path = &home.join(nm);
                match read_to_string_capped(p, MAX_GIT_CREDENTIAL_FILE_BYTES) {
                    Ok(text) => {
                        let mut parsed = parse_netrc(&text);
                        if parsed.iter().any(|m| {
                            m.get("password_present").and_then(|v| v.as_bool()) == Some(true)
                        }) {
                            plaintext_creds = true;
                        }
                        netrc_hosts.append(&mut parsed);
                    }
                    Err(CappedReadError::NotFound | CappedReadError::NotFile) => {}
                    Err(e) => {
                        skipped_files.push(json!({
                            "path": shorten(p, Some(home)),
                            "reason": e.reason(),
                        }));
                    }
                }
            }
        }

        // Read global credential helpers (read-only).
        if let Some(out) = run_stdout(
            "git",
            &["config", "--global", "--get-all", "credential.helper"],
            None,
        ) {
            for line in out.lines() {
                let first = line.split_whitespace().next().unwrap_or("");
                let name = first.rsplit('/').next().unwrap_or(first);
                let name = name.strip_prefix("git-credential-").unwrap_or(name);
                if !name.is_empty() && !helpers.contains(&name.to_string()) {
                    helpers.push(name.to_string());
                }
            }
        }

        let severity = if plaintext_creds {
            Severity::Exposed
        } else if !skipped_files.is_empty() {
            Severity::Notable
        } else if !helpers.is_empty() {
            Severity::Notable
        } else {
            Severity::Info
        };

        let title = if plaintext_creds {
            "plaintext git credentials readable"
        } else if !skipped_files.is_empty() {
            "git credential file present (not parsed)"
        } else if !helpers.is_empty() {
            "git credential helper configured"
        } else {
            "no git credential store reachable"
        };

        let host_names: Vec<String> = stored_hosts
            .iter()
            .filter_map(|h| h.get("host").and_then(|v| v.as_str()).map(String::from))
            .collect();

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(if plaintext_creds {
            format!("plaintext credentials for {} host(s)", host_names.len())
        } else if !skipped_files.is_empty() {
            "git credential file present; one or more files were not parsed".to_string()
        } else if !helpers.is_empty() {
            format!("credential helper(s): {}", helpers.join(", "))
        } else {
            "no stored git credentials found".to_string()
        })
        .evidence(json!({
            "helpers": helpers,
            "stored_hosts": host_names,
            "stored_host_details": stored_hosts,
            "netrc_hosts": netrc_hosts,
            "skipped_files": skipped_files,
        }))
        .remediation(&[
            "Prefer OS keychain/credential-manager helpers over plaintext stores.",
            "Scope credentials to the task; don't expose ~/.git-credentials or ~/.netrc to agents.",
        ]);

        Ok(vec![finding])
    }
}
