//! §23.4 — the Layer-1 value-free boundary + §24.8a path/url shape gate.
//!
//! Turns each [`AgentEvent`] into one or more [`NormalizedEvent`]s, each
//! carrying a value-free [`Signal`], an `approved` flag, and a value-free
//! `join_key`. At this boundary:
//! - `FileWrite.diff` is **dropped** (never read here);
//! - `ShellCommand.command`, `McpCall.input`, `Approval.reason` are reduced via
//!   the allowlist-by-default argv reducer (reused from the Layer-0 extractor)
//!   + `report::redaction::sweep`;
//! - `network_access.host` is redacted to `[custom egress target]` unless it is
//!   a well-known public endpoint;
//! - §24.8a: every `file_read.path`/`file_write.path`/`mcp_call.server`/`tool`
//!   is validated (single line, ≤4096 bytes, no control chars, swept); failure
//!   becomes `[unparseable path]` / `[redacted target]`.
//!
//! A single source `AgentEvent` may emit several `NormalizedEvent`s sharing one
//! `event_ix` (e.g. a shell command that both runs *and* matches a dangerous
//! pattern *and* reads a secret store). Nothing past this boundary carries a raw
//! value.

use serde::{Deserialize, Serialize};

use crate::session::discovery::extract::{dangerous_categories, reduce_command};
use crate::session::trace::AgentEvent;

/// Value-free, scored event. Join keys (shortened path/host/verb) are
/// value-free only; they are derived during normalization for `classify.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedEvent {
    pub signal: Signal,
    pub event_ix: usize,
    pub approved: bool,
    /// Value-free join key: a shortened path family, `host:port`, command
    /// **shape**, or `server·tool` — never a raw value. `None` when the signal
    /// carries no join target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_key: Option<String>,
}

/// Names match the §23.7 base-weight keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    ReadSecret,
    ModifiedProductionDeploy,
    ShellCommand,
    NetworkAccess,
    EditedAuthOrPaymentOrSecurityCode,
    DangerousShellPattern,
    ModifiedDependencyManifest,
    ExternalMcpCall,
    HumanApprovedRiskyAction,
}

/// The §24.8a fallback token for a path that fails the shape gate.
pub const UNPARSEABLE_PATH: &str = "[unparseable path]";
/// The §24.8a fallback token for a server/tool that fails the shape gate.
pub const REDACTED_TARGET: &str = "[redacted target]";
/// The §4.6/§12.11 custom-egress token.
pub const CUSTOM_EGRESS_TARGET: &str = "[custom egress target]";

/// A tiny allowlist of well-known *public* network endpoints whose hostname is
/// not credential-bearing and may be kept verbatim (§4.6). Everything else
/// collapses to `[custom egress target]`.
const WELL_KNOWN_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "raw.githubusercontent.com",
    "registry.npmjs.org",
    "crates.io",
    "static.crates.io",
    "pypi.org",
    "files.pythonhosted.org",
    "proxy.golang.org",
    "api.openai.com",
];

/// §24.8a path/url shape gate. Returns the swept value, or the fallback token
/// when the input fails the shape checks or trips a redaction pattern.
pub fn shape_gate(value: &str, fallback: &str) -> String {
    if value.is_empty()
        || value.len() > 4096
        || value.contains('\n')
        || value.chars().any(|c| c.is_control())
    {
        return fallback.to_string();
    }
    let swept = crate::report::redaction::sweep(value);
    // If the sweep altered the value, a secret shape was present → fall back.
    if swept != value || crate::report::redaction::contains_secret_shaped(&swept) {
        return fallback.to_string();
    }
    swept
}

/// Reduce a `network_access` host to a value-free token: keep a tiny allowlist
/// of well-known public endpoints, otherwise `[custom egress target]`.
pub fn reduce_host(host: &str) -> String {
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    // A credential-bearing or malformed host never survives.
    if h.is_empty()
        || h.contains('@')
        || h.contains('/')
        || h.contains(':')
        || h.len() > 253
        || crate::report::redaction::contains_secret_shaped(&h)
    {
        return CUSTOM_EGRESS_TARGET.to_string();
    }
    if WELL_KNOWN_HOSTS.contains(&h.as_str()) {
        return h;
    }
    CUSTOM_EGRESS_TARGET.to_string()
}

/// Reduce a swept path to a value-free join *family* the classifier can match
/// against finding ids, while still rendering a shortened path-shape.
///
/// The returned string is the (already gated) path shape; classification uses
/// the dedicated path predicates below, not this string.
fn gate_read_path(path: &str) -> String {
    shape_gate(path, UNPARSEABLE_PATH)
}

/// Lowercased basename of a gated path.
fn basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}

/// §24.3.1 secret-bearing member only: `~/.aws/credentials` not `~/.aws/config`;
/// `id_rsa` not `id_rsa.pub`. Returns true only for the concrete secret artifact.
pub fn is_secret_store_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let base = basename(&p);

    // SSH private keys — never the `.pub` member.
    if base.ends_with(".pub") {
        return false;
    }
    if base == "id_rsa" || base == "id_ed25519" || base == "id_ecdsa" || base == "id_dsa" {
        return true;
    }

    // AWS — credentials, never config.
    if p.contains(".aws/credentials") {
        return true;
    }

    // dotenv files (cross_repo.dotenv / env.secret_names).
    if base == ".env" || base.starts_with(".env.") {
        return true;
    }

    // git credential store.
    if base == ".git-credentials" || p.contains("/.config/git/credentials") {
        return true;
    }

    // browser session/cookie stores (browser.session_stores).
    if base == "cookies.sqlite"
        || base == "logins.json"
        || base == "cookies"
        || (base == "login data")
    {
        return true;
    }

    // generic credential files.
    if base == "credentials"
        || base == "credentials.json"
        || base == "credentials.toml"
        || base == ".netrc"
        || base == ".pgpass"
    {
        return true;
    }

    false
}

/// Browser session/cookie store member specifically (for `saas_session_hijack`).
pub fn is_browser_session_path(path: &str) -> bool {
    let base = basename(&path.to_ascii_lowercase());
    matches!(base.as_str(), "cookies.sqlite" | "logins.json" | "cookies" | "login data")
}

/// `.github/workflows/*.yml`, k8s/CI deploy manifests (modified_production_deploy).
pub fn is_deploy_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    if p.contains(".github/workflows/") && (p.ends_with(".yml") || p.ends_with(".yaml")) {
        return true;
    }
    let base = basename(&p);
    matches!(
        base.as_str(),
        "deployment.yaml"
            | "deployment.yml"
            | "deploy.yaml"
            | "deploy.yml"
            | "kustomization.yaml"
            | "kustomization.yml"
            | ".gitlab-ci.yml"
            | "fly.toml"
            | "vercel.json"
            | "render.yaml"
    ) || p.contains("/k8s/")
        || p.contains("/kubernetes/")
        || p.contains("/.circleci/")
}

/// auth/payment/security source path (edited_auth/payment/security_code).
pub fn is_auth_payment_security_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    if is_secret_store_path(&p) || is_deploy_path(&p) {
        return false;
    }
    const NEEDLES: &[&str] = &[
        "auth", "login", "session", "password", "passwd", "credential",
        "payment", "billing", "charge", "stripe", "checkout", "security",
        "crypto", "token", "oauth", "jwt", "permission", "rbac", "acl",
    ];
    // Only source-ish files (have an extension) so we don't match data dirs.
    let base = basename(&p);
    let is_source = base.contains('.')
        && !base.ends_with(".md")
        && !base.ends_with(".txt")
        && !base.ends_with(".lock");
    is_source && NEEDLES.iter().any(|n| base.contains(n))
}

/// dependency manifest / lockfile (modified_dependency_manifest).
pub fn is_dependency_manifest_path(path: &str) -> bool {
    let base = basename(&path.to_ascii_lowercase());
    matches!(
        base.as_str(),
        "package.json"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "cargo.toml"
            | "cargo.lock"
            | "requirements.txt"
            | "pipfile"
            | "pipfile.lock"
            | "poetry.lock"
            | "pyproject.toml"
            | "go.mod"
            | "go.sum"
            | "gemfile"
            | "gemfile.lock"
            | "composer.json"
            | "composer.lock"
    )
}

/// Does a (raw) shell command read a secret store? Conservative path-shape
/// match over operands of read-ish verbs; directory-prefix-only joins do not
/// count (§24.3.1) — we require a concrete secret artifact operand.
fn shell_reads_secret(command: &str) -> bool {
    let toks: Vec<&str> = command.split_whitespace().collect();
    let Some(prog) = toks.first().map(|t| t.rsplit('/').next().unwrap_or(t)) else {
        return false;
    };
    let read_verbs = matches!(prog, "cat" | "less" | "more" | "head" | "tail" | "cp" | "scp");
    if !read_verbs {
        return false;
    }
    toks.iter().skip(1).any(|t| is_secret_store_path(t))
}

/// Is an MCP server local (loopback / unix socket)? External servers fire
/// `external_mcp_call`.
fn is_local_mcp_server(server: &str) -> bool {
    let s = server.trim().to_ascii_lowercase();
    s == "local"
        || s == "localhost"
        || s.starts_with("127.")
        || s.starts_with("unix:")
        || s.starts_with("stdio")
        || s.starts_with("local:")
        || s == "::1"
}

/// Layer-1 normalization of a full event stream. Emits one or more
/// `NormalizedEvent`s per source event, all value-free.
pub fn normalize(events: &[AgentEvent]) -> Vec<NormalizedEvent> {
    // First pass: which event indices are explicit `Approval`s? An approval in
    // the trace marks the *session* as having a covering approval for risky
    // writes — used to set the `approved` flag on sensitive-write signals.
    let session_has_approval = events
        .iter()
        .any(|e| matches!(e, AgentEvent::Approval { .. }));

    let mut out: Vec<NormalizedEvent> = Vec::new();

    for (ix, event) in events.iter().enumerate() {
        match event {
            AgentEvent::FileRead { path } => {
                let gated = gate_read_path(path);
                if is_secret_store_path(path) {
                    out.push(NormalizedEvent {
                        signal: Signal::ReadSecret,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(gated),
                    });
                }
            }

            // `FileWrite.diff` is dropped here: we never read it.
            AgentEvent::FileWrite { path, diff: _ } => {
                let gated = gate_read_path(path);
                if is_deploy_path(path) {
                    out.push(NormalizedEvent {
                        signal: Signal::ModifiedProductionDeploy,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(gated.clone()),
                    });
                }
                if is_auth_payment_security_path(path) {
                    out.push(NormalizedEvent {
                        signal: Signal::EditedAuthOrPaymentOrSecurityCode,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(gated.clone()),
                    });
                }
                if is_dependency_manifest_path(path) {
                    out.push(NormalizedEvent {
                        signal: Signal::ModifiedDependencyManifest,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(gated),
                    });
                }
            }

            AgentEvent::ShellCommand { command } => {
                // Reduce to a value-free shape over the swept reduction.
                let shape = crate::report::redaction::sweep(&reduce_command(command));
                let cats = dangerous_categories(command);

                // Every shell command emits the base shell_command signal.
                out.push(NormalizedEvent {
                    signal: Signal::ShellCommand,
                    event_ix: ix,
                    approved: session_has_approval,
                    join_key: Some(shape.clone()),
                });

                if !cats.is_empty() {
                    let labels: Vec<&str> = cats.iter().map(|c| c.label()).collect();
                    out.push(NormalizedEvent {
                        signal: Signal::DangerousShellPattern,
                        event_ix: ix,
                        approved: session_has_approval,
                        // value-free: only the pattern category, never the substring.
                        join_key: Some(labels.join(",")),
                    });
                }

                if shell_reads_secret(command) {
                    out.push(NormalizedEvent {
                        signal: Signal::ReadSecret,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(shape),
                    });
                }
            }

            AgentEvent::NetworkAccess { host, port } => {
                let h = reduce_host(host);
                out.push(NormalizedEvent {
                    signal: Signal::NetworkAccess,
                    event_ix: ix,
                    approved: session_has_approval,
                    join_key: Some(format!("{h}:{port}")),
                });
            }

            // `Approval.reason` is swept-and-dropped: we never retain it.
            AgentEvent::Approval { approved_by: _, reason: _ } => {
                out.push(NormalizedEvent {
                    signal: Signal::HumanApprovedRiskyAction,
                    event_ix: ix,
                    approved: true,
                    join_key: None,
                });
            }

            // `McpCall.input` is dropped: only server/tool survive (gated).
            AgentEvent::McpCall { server, tool, input: _ } => {
                if !is_local_mcp_server(server) {
                    let s = shape_gate(server, REDACTED_TARGET);
                    let t = shape_gate(tool, REDACTED_TARGET);
                    out.push(NormalizedEvent {
                        signal: Signal::ExternalMcpCall,
                        event_ix: ix,
                        approved: session_has_approval,
                        join_key: Some(format!("{s}·{t}")),
                    });
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_store_member_only() {
        assert!(is_secret_store_path("~/.aws/credentials"));
        assert!(!is_secret_store_path("~/.aws/config"));
        assert!(is_secret_store_path("/home/u/.ssh/id_rsa"));
        assert!(!is_secret_store_path("/home/u/.ssh/id_rsa.pub"));
    }

    #[test]
    fn deploy_and_dependency_classification() {
        assert!(is_deploy_path(".github/workflows/deploy.yml"));
        assert!(is_dependency_manifest_path("Cargo.toml"));
        assert!(is_dependency_manifest_path("package.json"));
        assert!(!is_dependency_manifest_path("src/main.rs"));
    }

    #[test]
    fn host_reduction_collapses_custom_to_token() {
        assert_eq!(reduce_host("evil.attacker.example"), CUSTOM_EGRESS_TARGET);
        assert_eq!(reduce_host("github.com"), "github.com");
        assert_eq!(reduce_host("user:pass@host"), CUSTOM_EGRESS_TARGET);
    }

    #[test]
    fn shape_gate_rejects_control_and_secrets() {
        assert_eq!(shape_gate("a\nb", UNPARSEABLE_PATH), UNPARSEABLE_PATH);
        assert_eq!(
            shape_gate("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", REDACTED_TARGET),
            REDACTED_TARGET
        );
        assert_eq!(shape_gate("src/lib.rs", UNPARSEABLE_PATH), "src/lib.rs");
    }

    #[test]
    fn normalize_risky_trace_tags_expected_signals() {
        let events = vec![
            AgentEvent::FileRead { path: "~/.aws/credentials".into() },
            AgentEvent::FileWrite { path: ".github/workflows/deploy.yml".into(), diff: None },
            AgentEvent::ShellCommand { command: "git push".into() },
            AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
        ];
        let norm = normalize(&events);
        let signals: Vec<Signal> = norm.iter().map(|n| n.signal).collect();
        assert!(signals.contains(&Signal::ReadSecret));
        assert!(signals.contains(&Signal::ModifiedProductionDeploy));
        assert!(signals.contains(&Signal::ShellCommand));
        assert!(signals.contains(&Signal::NetworkAccess));
    }

    #[test]
    fn normalize_drops_nonsecret_reads() {
        let events = vec![AgentEvent::FileRead { path: "src/lib.rs".into() }];
        assert!(normalize(&events).is_empty());
    }
}
