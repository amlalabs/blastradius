//! §12.2 — SSH private keys. Counts readable private-key files; collects Host
//! aliases from config. Never reads key contents beyond the header sniff.

use serde_json::json;
use std::path::Path;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct SshProbe;

const MAX_SSH_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

/// A file is treated as a private key if it's a regular readable non-`.pub`
/// file whose first KB contains a PEM private-key header (§12.2).
fn looks_like_private_key(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    if name.ends_with(".pub")
        || matches!(name, "known_hosts" | "authorized_keys" | "config")
        || name.starts_with("known_hosts")
    {
        return false;
    }
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if !meta.is_file() {
        return false;
    }
    // Sniff first KB for a PEM private-key header.
    let mut buf = vec![0u8; 1024];
    use std::io::Read;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let n = f.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    head.contains("-----BEGIN") && head.contains("PRIVATE KEY-----")
}

impl Probe for SshProbe {
    fn id(&self) -> &'static str {
        "ssh.private_keys"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = ctx.home.clone();
        let ssh_dir = match &home {
            Some(h) => h.join(".ssh"),
            None => {
                return Ok(vec![Finding::new(
                    self.id(),
                    self.class(),
                    FindingScope::Ambient,
                    "no SSH directory (home unknown)",
                    Severity::Info,
                    Confidence::Unknown,
                )]);
            }
        };

        let mut key_paths: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if looks_like_private_key(&path) {
                    key_paths.push(shorten(&path, home.as_deref()));
                }
            }
        }
        key_paths.sort();

        // Host aliases from ~/.ssh/config — names only.
        let mut configured_hosts: Vec<String> = Vec::new();
        let mut config_skipped = false;
        match read_to_string_capped(&ssh_dir.join("config"), MAX_SSH_CONFIG_BYTES) {
            Ok(text) => {
                for line in text.lines() {
                    let line = line.trim();
                    if let Some(rest) = line
                        .strip_prefix("Host ")
                        .or_else(|| line.strip_prefix("host "))
                    {
                        for h in rest.split_whitespace() {
                            if h != "*" && !configured_hosts.contains(&h.to_string()) {
                                configured_hosts.push(h.to_string());
                            }
                        }
                    }
                }
            }
            Err(CappedReadError::NotFound | CappedReadError::NotFile) => {}
            Err(_) => {
                config_skipped = true;
            }
        }

        let key_count = key_paths.len();
        let severity = if key_count > 0 {
            Severity::Exposed
        } else if !configured_hosts.is_empty() || config_skipped {
            Severity::Notable
        } else {
            Severity::Info
        };

        let title = if key_count > 0 {
            "SSH private keys readable"
        } else if !configured_hosts.is_empty() || config_skipped {
            "SSH config present (no readable private keys)"
        } else {
            "no SSH private keys reachable"
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(format!(
            "{key_count} private key file(s) readable; passphrase status not checked"
        ))
        .evidence(json!({
            "key_count": key_count,
            "paths": key_paths,
            "configured_hosts": configured_hosts,
            "config_skipped": config_skipped,
        }))
        .remediation(&[
            "Provide agents a dedicated, scoped key — not your full ~/.ssh.",
            "Prefer short-lived signed certs or per-task deploy keys.",
        ]);

        Ok(vec![finding])
    }
}
