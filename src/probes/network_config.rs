//! §extra — host network-config tampering & stored network secrets (Linux/unix).
//!
//! Two often-missed angles:
//!   A) **Resolution/redirect tampering** — a writable `/etc/hosts` or
//!      `/etc/resolv.conf` lets code running as you silently redirect domains or
//!      DNS (MITM of package mirrors, registries, internal hosts). Normally
//!      root-owned, so this is `Info` unless actually writable by you.
//!   B) **Stored network secrets** — NetworkManager keeps WiFi PSKs and VPN
//!      credentials in `/etc/NetworkManager/system-connections/`; if those files
//!      are readable by the agent, the secrets are reachable.
//!
//! READ-ONLY: a pure permission/readability check. Nothing is written; file
//! contents are never read or emitted — only path, writability, and a readable
//! connection-profile COUNT.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct NetworkConfigProbe;

impl Probe for NetworkConfigProbe {
    fn id(&self) -> &'static str {
        "host.network_config"
    }
    fn class(&self) -> FindingClass {
        FindingClass::HostPersistence
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(unix)]
fn platform_run(probe: &NetworkConfigProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    use crate::util::fsperm::{check, home_identity};
    use std::path::Path;

    let home = match &ctx.home {
        Some(h) => h.clone(),
        None => return Ok(vec![info(probe, "home unknown; cannot resolve write identity")]),
    };
    let (uid, gid) = match home_identity(&home) {
        Some(x) => x,
        None => return Ok(vec![info(probe, "home unreadable; cannot resolve write identity")]),
    };

    // (A) redirect/DNS files.
    let mut redirect_writable: Vec<&str> = Vec::new();
    for f in ["/etc/hosts", "/etc/resolv.conf"] {
        let c = check(Path::new(f), uid, gid);
        if c.writable {
            redirect_writable.push(f);
        }
    }

    // (B) NetworkManager stored connection profiles readable by us.
    let mut nm_readable = 0usize;
    if let Ok(entries) = std::fs::read_dir("/etc/NetworkManager/system-connections") {
        for e in entries.flatten() {
            // Readable == we can open it (the secrets live inside; we don't read them).
            if std::fs::File::open(e.path()).is_ok() {
                nm_readable += 1;
            }
        }
    }

    let (severity, title, summary) = if !redirect_writable.is_empty() {
        (
            Severity::Exposed,
            "host name/DNS resolution is tamperable",
            format!(
                "{} is writable by you — code running as you can silently redirect domains/DNS (MITM of mirrors, registries, internal hosts)",
                redirect_writable.join(" & ")
            ),
        )
    } else if nm_readable > 0 {
        (
            Severity::Exposed,
            "stored WiFi/VPN secrets readable",
            format!("{nm_readable} NetworkManager connection profile(s) readable — WiFi PSKs / VPN credentials are reachable"),
        )
    } else {
        (
            Severity::Info,
            "network config not tamperable, no stored network secrets readable",
            "/etc/hosts & /etc/resolv.conf not writable; no readable NetworkManager profiles".to_string(),
        )
    };

    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        title,
        severity,
        Confidence::Confirmed,
    )
    .summary(summary)
    .evidence(json!({
        "redirect_files_writable": redirect_writable,
        "networkmanager_profiles_readable": nm_readable,
        "note": "Permission/readability check only; file contents (hosts entries, PSKs) are never read or emitted.",
    }))
    .remediation(&[
        "Keep /etc/hosts and /etc/resolv.conf root-owned and non-writable by the agent user.",
        "Restrict /etc/NetworkManager/system-connections to root; don't expose stored WiFi/VPN secrets to agents.",
    ])])
}

#[cfg(unix)]
fn info(probe: &NetworkConfigProbe, why: &str) -> Finding {
    Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "network config — not assessed",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary(why.to_string())
    .evidence(json!({ "assessed": false }))
}

#[cfg(not(unix))]
fn platform_run(probe: &NetworkConfigProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "network config — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("network-config tampering check is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}
