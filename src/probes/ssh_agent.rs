//! §extra — SSH agent socket reachability.
//!
//! `ssh.rs` counts private-key *files*. But a reachable `SSH_AUTH_SOCK` is a
//! distinct, often higher-signal capability: code running as you can authenticate
//! with every identity loaded in the agent — INCLUDING passphrase-protected keys
//! whose files `ssh.rs` can only report as "readable but encrypted" — without ever
//! reading a key. (`env.rs` deliberately suppresses `SSH_AUTH_SOCK` as a non-secret
//! name; this probe is where its reachability is assessed.)
//!
//! READ-ONLY in effect and value-free. When the socket is connectable we send a
//! single `REQUEST_IDENTITIES` (list) message and read back the identity COUNT.
//! Listing identities performs no signing and mutates nothing; we never read the
//! key blobs or comments — only the count leaves this probe. The connect+query is
//! bounded by a worker thread + timeout, mirroring `sandbox_reach`'s docker.sock
//! probe.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct SshAgentProbe;

impl Probe for SshAgentProbe {
    fn id(&self) -> &'static str {
        "ssh.agent_socket"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(unix)]
fn platform_run(probe: &SshAgentProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    // Read the live process env for the socket PATH (never a secret value),
    // mirroring how aws.rs reads AWS_SHARED_CREDENTIALS_FILE.
    let sock = std::env::var_os("SSH_AUTH_SOCK");
    let sock_set = sock.is_some();

    let (connectable, identities, query_note) = match &sock {
        Some(p) if !p.is_empty() => match query_agent(std::path::Path::new(p)) {
            AgentQuery::Identities(n) => (true, Some(n), "queried agent identity list (count only)"),
            AgentQuery::ConnectedNoAnswer => {
                (true, None, "connected but agent did not return an identity list")
            }
            AgentQuery::Unreachable => {
                (false, None, "SSH_AUTH_SOCK set but socket not connectable (forwarded/stale?)")
            }
        },
        _ => (false, None, "SSH_AUTH_SOCK not set"),
    };

    let (severity, confidence, title, summary) = match (sock_set, identities) {
        (true, Some(n)) if n > 0 => (
            Severity::Exposed,
            Confidence::Confirmed,
            "SSH agent has loaded keys usable for authentication",
            format!(
                "ssh-agent reachable with {n} loaded identity(ies) — usable to authenticate as you (incl. passphrase-protected keys) without reading any key file"
            ),
        ),
        (true, Some(_)) => (
            Severity::Notable,
            Confidence::Confirmed,
            "SSH agent reachable but no keys loaded",
            "ssh-agent socket connectable; zero identities currently loaded".to_string(),
        ),
        (true, None) if connectable => (
            Severity::Notable,
            Confidence::Likely,
            "SSH agent socket connectable (identity list unavailable)",
            "ssh-agent socket connectable but identity count could not be read".to_string(),
        ),
        (true, None) => (
            Severity::Notable,
            Confidence::Likely,
            "SSH_AUTH_SOCK set but agent not connectable",
            "SSH_AUTH_SOCK is set; the socket did not accept a connection (forwarded or stale)"
                .to_string(),
        ),
        (false, _) => (
            Severity::Info,
            Confidence::Confirmed,
            "no SSH agent in this environment",
            "SSH_AUTH_SOCK not set; no ssh-agent reachable".to_string(),
        ),
    };

    let ssh = Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        title,
        severity,
        confidence,
    )
    .summary(summary)
    .evidence(json!({
        "ssh_auth_sock_set": sock_set,
        "socket_connectable": connectable,
        "loaded_identities": identities,
        "note": query_note,
        "method": "REQUEST_IDENTITIES (list) only — no signing, no mutation; key blobs/comments never read.",
    }))
    .remediation(&[
        "Don't forward or mount your ssh-agent into agent environments; loaded keys are usable without the key files.",
        "Use a dedicated, scoped key or short-lived signed cert per task instead of your full agent.",
    ]);

    Ok(vec![ssh, gpg_agent_finding(ctx)])
}

/// A reachable `gpg-agent` socket is the GPG analogue of ssh-agent: if a
/// passphrase is cached, the agent can decrypt/sign as you without re-prompting —
/// no key file or passphrase needed. We only check socket PRESENCE (never use it).
#[cfg(unix)]
fn gpg_agent_finding(ctx: &Context) -> Finding {
    use std::os::unix::fs::FileTypeExt;

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(gnupghome) = std::env::var_os("GNUPGHOME").filter(|v| !v.is_empty()) {
        candidates.push(std::path::PathBuf::from(&gnupghome).join("S.gpg-agent"));
    }
    if let Some(h) = &ctx.home {
        candidates.push(h.join(".gnupg/S.gpg-agent"));
    }
    // Modern gpg places sockets under the per-user runtime dir.
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        candidates.push(std::path::PathBuf::from(rt).join("gnupg/S.gpg-agent"));
    }

    let present = candidates.iter().any(|p| {
        std::fs::symlink_metadata(p)
            .map(|m| m.file_type().is_socket())
            .unwrap_or(false)
    });

    let (severity, title, summary) = if present {
        (
            Severity::Notable,
            "gpg-agent socket reachable",
            "a running gpg-agent socket is reachable — if a passphrase is cached it can decrypt/sign as you without the passphrase or key file".to_string(),
        )
    } else {
        (
            Severity::Info,
            "no gpg-agent socket reachable",
            "no gpg-agent socket found".to_string(),
        )
    };

    Finding::new(
        "gpg.agent_socket",
        FindingClass::Credentials,
        FindingScope::Ambient,
        title,
        severity,
        Confidence::Likely,
    )
    .summary(summary)
    .evidence(json!({
        "socket_present": present,
        "note": "Presence only — the socket is never used. Cache state can't be queried without exercising the agent.",
    }))
    .remediation(&[
        "Keep gpg-agent out of agent scope; set a short default-cache-ttl so cached passphrases expire quickly.",
    ])
}

/// Outcome of a bounded ssh-agent query.
#[cfg(unix)]
enum AgentQuery {
    Identities(u32),
    ConnectedNoAnswer,
    Unreachable,
}

/// Connect to the agent socket, send `SSH2_AGENTC_REQUEST_IDENTITIES`, and parse
/// the identity count from `SSH2_AGENT_IDENTITIES_ANSWER`. Bounded by a worker
/// thread + timeout so a hung/forwarded socket can't stall the scan. No signing
/// request is ever made; key material is never read.
#[cfg(unix)]
fn query_agent(path: &std::path::Path) -> AgentQuery {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::time::Duration;

    const REQUEST_IDENTITIES: u8 = 11;
    const IDENTITIES_ANSWER: u8 = 12;
    const TIMEOUT: Duration = Duration::from_secs(3);

    let p = path.to_path_buf();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let mut stream = match UnixStream::connect(&p) {
                Ok(s) => s,
                Err(_) => return AgentQuery::Unreachable,
            };
            let _ = stream.set_read_timeout(Some(TIMEOUT));
            let _ = stream.set_write_timeout(Some(TIMEOUT));

            // Request = uint32 length (1) + byte type.
            let req = [0u8, 0, 0, 1, REQUEST_IDENTITIES];
            if stream.write_all(&req).is_err() {
                return AgentQuery::ConnectedNoAnswer;
            }

            // Answer header: uint32 length, byte type, uint32 nkeys.
            let mut header = [0u8; 9];
            if stream.read_exact(&mut header).is_err() {
                return AgentQuery::ConnectedNoAnswer;
            }
            if header[4] != IDENTITIES_ANSWER {
                return AgentQuery::ConnectedNoAnswer;
            }
            let nkeys = u32::from_be_bytes([header[5], header[6], header[7], header[8]]);
            AgentQuery::Identities(nkeys)
        })();
        let _ = tx.send(result);
    });

    rx.recv_timeout(TIMEOUT + Duration::from_secs(1))
        .unwrap_or(AgentQuery::Unreachable)
}

#[cfg(not(unix))]
fn platform_run(probe: &SshAgentProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        "SSH agent probe — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("ssh-agent socket reachability is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}
