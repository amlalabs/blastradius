//! §extra — post-escalation ("if root") blast radius.
//!
//! The other probes measure what's reachable *as you*. This one models the
//! EXTENDED surface that becomes reachable if the agent reaches **root** — either
//! via an escalation path already detected on this host (`docker`/`lxd` group,
//! `NOPASSWD` sudo — see `host.privilege_escalation`) or via a local kernel
//! exploit (the Claude Code sandbox runs default-ALLOW seccomp, so the full
//! syscall/LPE surface is exposed; see the security-model doc findings #4/#5).
//!
//! It does **NOT** attempt any escalation and **NOT** read any root-owned file.
//! It only `stat()`s a curated set of high-value root targets (metadata needs no
//! read permission) and records, for each: does it exist, is it root-owned, and
//! can we read it *right now* (an `open()` that returns EACCES for a root file is
//! a permission check, not a bypass). The result is an honest inventory of the
//! "blast radius of root" — plus any root file that is ALREADY readable (a live
//! misconfiguration, e.g. group-readable `/etc/shadow`).
//!
//! READ-ONLY and value-free: only labels, shortened paths, status enums, and
//! counts are emitted — never any file content.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct PrivilegedReachProbe;

/// A high-value asset that is normally root-only.
struct Target {
    label: &'static str,
    path: &'static str,
    /// Inspect by listing (dir) vs opening (file). Devices are stat-only.
    kind: TargetKind,
}

#[derive(PartialEq)]
enum TargetKind {
    File,
    Dir,
    /// Device / special file — never opened, only stat'd.
    Device,
}

use TargetKind::{Device, Dir, File};

// Each target is a high-value root asset. For `File` targets, being readable by
// the current user is a genuine misconfiguration (the file IS a secret). For
// `Dir`/`Device` targets, normal listability is NOT a leak — the secret lives in
// mode-restricted contents — so they are always counted as gated surface.
const TARGETS: &[Target] = &[
    // Secret-bearing files: readable-by-us == misconfiguration.
    Target { label: "password hashes (/etc/shadow)", path: "/etc/shadow", kind: File },
    Target { label: "group passwords (/etc/gshadow)", path: "/etc/gshadow", kind: File },
    Target { label: "root's kubeconfig", path: "/root/.kube/config", kind: File },
    Target { label: "Kerberos keytab", path: "/etc/krb5.keytab", kind: File },
    Target { label: "IPsec secrets", path: "/etc/ipsec.secrets", kind: File },
    Target { label: "Kubernetes admin kubeconfig", path: "/etc/kubernetes/admin.conf", kind: File },
    Target { label: "k3s admin kubeconfig", path: "/etc/rancher/k3s/k3s.yaml", kind: File },
    // Secret-bearing directories (contents gated regardless of listability).
    Target { label: "SSH host private keys", path: "/etc/ssh", kind: Dir },
    Target { label: "root's SSH identity", path: "/root/.ssh", kind: Dir },
    Target { label: "root's AWS credentials", path: "/root/.aws", kind: Dir },
    Target { label: "TLS private keys (Let's Encrypt)", path: "/etc/letsencrypt", kind: Dir },
    Target { label: "WireGuard configs", path: "/etc/wireguard", kind: Dir },
    Target { label: "NetworkManager WiFi/VPN secrets", path: "/etc/NetworkManager/system-connections", kind: Dir },
    // Service data / secrets.
    Target { label: "all Docker container data", path: "/var/lib/docker", kind: Dir },
    Target { label: "PostgreSQL data dir", path: "/var/lib/postgresql", kind: Dir },
    Target { label: "MySQL data dir", path: "/var/lib/mysql", kind: Dir },
    Target { label: "shadow/file backups", path: "/var/backups", kind: Dir },
    // Kernel memory — the literal payoff of a kernel exploit.
    Target { label: "kernel RAM (/proc/kcore)", path: "/proc/kcore", kind: Device },
    Target { label: "physical RAM (/dev/mem)", path: "/dev/mem", kind: Device },
];

impl Probe for PrivilegedReachProbe {
    fn id(&self) -> &'static str {
        "host.privileged_reachability"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }
    fn run(&self, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self)
    }
}

#[cfg(unix)]
fn platform_run(probe: &PrivilegedReachProbe) -> anyhow::Result<Vec<Finding>> {
    let mut readable_now: Vec<&str> = Vec::new(); // root file we can ALREADY read
    let mut root_gated: Vec<&str> = Vec::new(); // exists, root-owned, unreadable
    let mut unobservable = 0usize; // parent not traversable -> can't even stat
    let mut absent = 0usize;

    for t in TARGETS {
        let path = std::path::Path::new(t.path);
        match std::fs::symlink_metadata(path) {
            Ok(_meta) => match t.kind {
                // A secret FILE we can open() right now is a live misconfiguration
                // (e.g. group-readable /etc/shadow). The open is closed immediately;
                // no content is read.
                File => {
                    if std::fs::File::open(path).is_ok() {
                        readable_now.push(t.label);
                    } else {
                        root_gated.push(t.label);
                    }
                }
                // Dirs/devices: listability/stat is not a secret leak — the payoff
                // is mode-restricted contents/memory. Always gated surface.
                Dir | Device => root_gated.push(t.label),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => absent += 1,
            // EACCES on stat means a parent dir isn't traversable (e.g. /root 0700)
            // — the target may well exist but is fully root-gated.
            Err(_) => unobservable += 1,
        }
    }

    // Escalation paths already present (cheap, group-only — no sudo re-invocation;
    // host.privilege_escalation covers NOPASSWD sudo precisely).
    let groups: Vec<String> = crate::util::command::run_stdout("id", &["-nG"], None)
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    let escalation_groups: Vec<&str> = ["docker", "lxd", "libvirt", "kvm", "sudo", "wheel", "admin"]
        .into_iter()
        .filter(|g| groups.iter().any(|m| m == g))
        .collect();
    let is_root = crate::util::command::run_stdout("id", &["-u"], None).as_deref() == Some("0");
    let escalation_present = !escalation_groups.is_empty();

    let extended_count = root_gated.len() + unobservable;

    let (severity, confidence, title, summary) = if is_root {
        (
            Severity::Info,
            Confidence::Confirmed,
            "already root — no escalation needed (see process privilege findings)",
            "this process is root; the 'post-escalation' surface is simply reachable now".to_string(),
        )
    } else if !readable_now.is_empty() {
        (
            Severity::Exposed,
            Confidence::Confirmed,
            "root-owned secrets already readable (misconfiguration)",
            format!(
                "{} root-owned target(s) are readable WITHOUT escalation — e.g. {} — a live misconfiguration",
                readable_now.len(),
                readable_now.first().copied().unwrap_or("")
            ),
        )
    } else if extended_count > 0 && escalation_present {
        (
            Severity::Exposed,
            Confidence::Likely,
            "large post-escalation blast radius, with an escalation path present",
            format!(
                "an escalation path is present ({}); on root, {} root-only asset(s) become reachable — host keys, shadow, root creds, kernel memory",
                escalation_groups.join("/"),
                extended_count
            ),
        )
    } else if extended_count > 0 {
        (
            Severity::Notable,
            Confidence::Possible,
            "post-escalation blast radius (root via a local kernel exploit)",
            format!(
                "no direct escalation path detected, but a local kernel exploit (sandbox seccomp is default-allow) would unlock {} root-only asset(s): host keys, shadow, root creds, kernel memory",
                extended_count
            ),
        )
    } else {
        (
            Severity::Info,
            Confidence::Confirmed,
            "no high-value root-only targets present",
            "none of the curated root-only assets exist on this host".to_string(),
        )
    };

    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        title,
        severity,
        confidence,
    )
    .summary(summary)
    .evidence(json!({
        "currently_readable_root_targets": readable_now,
        "root_gated_targets": root_gated,
        "root_gated_count": root_gated.len(),
        "unobservable_count": unobservable,
        "absent_count": absent,
        "extended_surface_count": extended_count,
        "escalation_path_present": escalation_present,
        "escalation_groups": escalation_groups,
        "is_root": is_root,
        "method": "stat/permission metadata only — NO escalation is attempted and NO root file content is read. 'readable_now' means an open()/read_dir() succeeded as the current user; 'root_gated' means it exists but is not readable.",
        "note": "This is conditional reachability: the inventory is the blast radius IF root is reached (via a detected escalation path and/or a kernel LPE). The sandbox runs default-allow seccomp, so the kernel-LPE surface is not contained (security-model findings #4/#5).",
    }))
    .remediation(&[
        "Shrink the post-root blast radius: keep host keys, root creds, and kernel-memory interfaces on hosts the agent can't escalate on.",
        "Close direct escalation (remove docker/lxd group membership and NOPASSWD sudo for the agent user).",
        "Run agents in a VM/microVM so a kernel LPE doesn't expose the real host's root assets.",
        "Fix any root-owned file that is already readable (e.g. group-readable /etc/shadow).",
    ])])
}

#[cfg(not(unix))]
fn platform_run(probe: &PrivilegedReachProbe) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "post-escalation reachability — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("root-only target inventory is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}
