//! §extra — local privilege-escalation reach.
//!
//! The most under-appreciated authority a coding agent inherits: the ability to
//! become **root** without any exploit. Two everyday paths people forget the
//! agent has:
//!   - **Passwordless sudo** — if `NOPASSWD` is configured (or a sudo timestamp
//!     is cached), code running as you can run anything as root non-interactively.
//!   - **Root-equivalent group membership** — being in `docker`/`lxd`/`libvirt`/
//!     `kvm` is *already* root-equivalent (mount the host fs in a container);
//!     `sudo`/`wheel`/`admin` membership means sudo is available at all.
//!
//! READ-ONLY and value-free. Group membership comes from `id -nG` (pure read).
//! Passwordless sudo is detected with `sudo -n -l` (non-interactive **list**, no
//! command is run as root, no password prompt) — the same "bounded local probe"
//! posture as the docker.sock connect. Only group/flag booleans are emitted.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::command::{run_stdout, run_status};

pub struct PrivilegeProbe;

/// Groups whose membership is effectively root-equivalent (container/VM escape
/// to the host, or direct device access).
const ROOT_EQUIVALENT_GROUPS: &[&str] = &["docker", "lxd", "libvirt", "kvm", "incus", "podman"];
/// Groups that grant administrative authority (sudo is available).
const ADMIN_GROUPS: &[&str] = &["sudo", "wheel", "admin", "root", "adm", "sudoers"];

impl Probe for PrivilegeProbe {
    fn id(&self) -> &'static str {
        "host.privilege_escalation"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }

    fn run(&self, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        // --- (1) group membership (pure read via `id -nG`). ---
        let groups: Vec<String> = run_stdout("id", &["-nG"], None)
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        let root_equiv: Vec<&str> = ROOT_EQUIVALENT_GROUPS
            .iter()
            .copied()
            .filter(|g| groups.iter().any(|m| m == g))
            .collect();
        let admin: Vec<&str> = ADMIN_GROUPS
            .iter()
            .copied()
            .filter(|g| groups.iter().any(|m| m == g))
            .collect();
        let is_root = groups.first().map(|g| g == "root").unwrap_or(false)
            || run_stdout("id", &["-u"], None).as_deref() == Some("0");

        // --- (2) passwordless sudo (non-interactive LIST; no command run). ---
        // `sudo -n -l` succeeds without a prompt only if a password isn't needed
        // (NOPASSWD) or a timestamp is cached; otherwise it exits non-zero.
        let sudo_listing = run_stdout("sudo", &["-n", "-l"], None);
        let sudo_nopasswd = sudo_listing
            .as_deref()
            .map(|s| s.contains("NOPASSWD"))
            .unwrap_or(false);
        // A successful listing without NOPASSWD still means sudo is reachable
        // (cached credential / passwordless for `-l` itself).
        let sudo_reachable = sudo_listing.is_some();

        // --- (3) pkexec (polkit) presence — an alternative escalation path. ---
        let pkexec_present = run_status("pkexec", &["--version"], None).unwrap_or(false);

        // --- verdict ---
        let root_now = is_root;
        let root_equivalent_reach =
            !root_equiv.is_empty() || sudo_nopasswd || (sudo_reachable && !admin.is_empty());

        let (severity, confidence, title, summary) = if root_now {
            (
                Severity::Exposed,
                Confidence::Confirmed,
                "process already runs as root",
                "this process is uid 0 — full host authority, no escalation needed".to_string(),
            )
        } else if root_equivalent_reach {
            let basis = escalation_basis(&root_equiv, sudo_nopasswd, sudo_reachable && !admin.is_empty());
            (
                Severity::Exposed,
                Confidence::Likely,
                "code running as you can escalate to root",
                format!("root-equivalent reach: {basis} — an agent here can take over the host"),
            )
        } else if !admin.is_empty() || pkexec_present {
            (
                Severity::Notable,
                Confidence::Likely,
                "administrative escalation path present (password-gated)",
                format!(
                    "member of {} / pkexec={} — escalation is available but appears to require a secret",
                    if admin.is_empty() { "—".to_string() } else { admin.join(",") },
                    pkexec_present
                ),
            )
        } else {
            (
                Severity::Info,
                Confidence::Confirmed,
                "no obvious local privilege-escalation path",
                "not root, no root-equivalent group, no passwordless sudo detected".to_string(),
            )
        };

        Ok(vec![Finding::new(
            self.id(),
            self.class(),
            FindingScope::Host,
            title,
            severity,
            confidence,
        )
        .summary(summary)
        .evidence(json!({
            "is_root": root_now,
            "root_equivalent_groups": root_equiv,
            "admin_groups": admin,
            "sudo_reachable_noninteractive": sudo_reachable,
            "sudo_nopasswd": sudo_nopasswd,
            "pkexec_present": pkexec_present,
            "group_count": groups.len(),
            "note": "Group membership via `id -nG`; passwordless sudo via `sudo -n -l` (a non-interactive LIST — no command is run as root and no password is prompted). docker/lxd/libvirt/kvm membership is root-equivalent on its own.",
        }))
        .remediation(&[
            "Don't run agents as a user in the docker/lxd/libvirt/kvm groups, or with NOPASSWD sudo — both are root-equivalent.",
            "Run agents as a dedicated low-privilege user with no sudoers entry and no escalation-capable group membership.",
        ])])
    }
}

fn escalation_basis(root_equiv: &[&str], nopasswd: bool, sudo_admin: bool) -> String {
    let mut parts = Vec::new();
    if !root_equiv.is_empty() {
        parts.push(format!("{} group membership", root_equiv.join("/")));
    }
    if nopasswd {
        parts.push("passwordless (NOPASSWD) sudo".to_string());
    } else if sudo_admin {
        parts.push("reachable sudo (cached/uncontested)".to_string());
    }
    parts.join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_combines_group_and_sudo() {
        assert_eq!(
            escalation_basis(&["docker"], true, false),
            "docker group membership + passwordless (NOPASSWD) sudo"
        );
        assert_eq!(escalation_basis(&["lxd"], false, false), "lxd group membership");
        assert_eq!(escalation_basis(&[], true, false), "passwordless (NOPASSWD) sudo");
    }
}
