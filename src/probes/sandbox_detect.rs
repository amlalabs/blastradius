//! §extra — sandbox self-detection (the framing signal).
//!
//! Answers "is THIS process contained?" so every other finding can be read as
//! live vs. contained. Aggregates observable signals: per-namespace isolation
//! (net/pid/ipc/uts/cgroup/user — exposes the missing-namespace gap, finding
//! #10, when run inside a sandbox), an active seccomp filter, `AF_UNIX` blocking
//! (the sandbox-runtime signature), `HTTP(S)_PROXY` mediation, `/proc/self/environ`
//! reachability, and whether the host can run 32-bit binaries (the ia32
//! `socketcall` bypass surface, finding #5).
//!
//! READ-ONLY and value-free: reads `/proc`, env-var NAMES, and attempts one
//! throwaway `AF_UNIX` socket (no bind/connect). No values are emitted.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct SandboxDetectProbe;

impl Probe for SandboxDetectProbe {
    fn id(&self) -> &'static str {
        "process.sandbox_detect"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(target_os = "linux")]
fn platform_run(probe: &SandboxDetectProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    const NS: &[&str] = &["net", "pid", "ipc", "uts", "cgroup", "user", "mnt"];

    // Per-namespace isolation: self vs PID 1. /proc/1/ns/* is usually
    // unreadable to non-root (-> None = unknown), so net/pid also get heuristics.
    let mut ns_json = serde_json::Map::new();
    let mut isolated_count = 0usize;
    for name in NS {
        let self_ino = std::fs::read_link(format!("/proc/self/ns/{name}")).ok();
        let init_ino = std::fs::read_link(format!("/proc/1/ns/{name}")).ok();
        let isolated = match (&self_ino, &init_ino) {
            (Some(s), Some(i)) => Some(s != i),
            _ => None,
        };
        if isolated == Some(true) {
            isolated_count += 1;
        }
        ns_json.insert(
            name.to_string(),
            json!({
                "inode": self_ino.map(|p| p.to_string_lossy().to_string()),
                "isolated": isolated,
            }),
        );
    }

    // net heuristic: --unshare-net leaves only loopback.
    let only_loopback = net_only_loopback();
    let net_isolated = match ns_json.get("net").and_then(|v| v["isolated"].as_bool()) {
        Some(b) => Some(b),
        None => only_loopback, // Some(true) if only lo
    };

    // seccomp filter active? /proc/self/status "Seccomp:" (0 none, 1 strict, 2 filter)
    let seccomp = read_status_field("Seccomp").unwrap_or(0);
    let seccomp_filter = seccomp >= 1;

    // AF_UNIX socket creation blocked? (sandbox-runtime seccomp signature)
    let afunix_blocked = std::os::unix::net::UnixDatagram::unbound().is_err();

    // proxy mediation (env NAMES only; values not read)
    let proxy_set = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
    ]
    .iter()
    .any(|k| std::env::var_os(k).is_some());

    // /proc/self/environ reachable?
    let environ_readable = std::fs::read("/proc/self/environ").is_ok();

    // 32-bit / ia32 execution support (finding #5 surface).
    let ia32_support = ["/lib/ld-linux.so.2", "/lib32", "/usr/lib32", "/libx32"]
        .iter()
        .any(|p| std::path::Path::new(p).exists());

    // pid-isolation hint: a fresh pid ns shows very few processes.
    let pid_count = std::fs::read_dir("/proc")
        .map(|r| {
            r.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| s.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);
    let pid_isolated_hint = pid_count > 0 && pid_count < 15;

    // ---- verdict ----
    let mut mechanisms: Vec<&str> = Vec::new();
    if seccomp_filter {
        mechanisms.push("seccomp-filter");
    }
    if afunix_blocked {
        mechanisms.push("afunix-blocked");
    }
    if proxy_set {
        mechanisms.push("http-proxy");
    }
    if !environ_readable {
        mechanisms.push("proc-environ-blocked");
    }
    if net_isolated == Some(true) {
        mechanisms.push("net-namespace");
    }
    if pid_isolated_hint || ns_json.get("pid").and_then(|v| v["isolated"].as_bool()) == Some(true) {
        mechanisms.push("pid-namespace");
    }

    // Strong signature: the sandbox-runtime unix-socket block + a seccomp filter.
    let strong = afunix_blocked && seccomp_filter;
    let (verdict, confidence) = if strong {
        ("sandboxed", Confidence::Confirmed)
    } else if mechanisms.len() >= 2 {
        ("likely sandboxed", Confidence::Likely)
    } else if mechanisms.len() == 1 {
        ("possibly sandboxed", Confidence::Possible)
    } else {
        ("not sandboxed", Confidence::Confirmed)
    };
    let sandboxed = verdict != "not sandboxed";

    let (severity, title) = if !sandboxed {
        (
            Severity::Notable,
            "process is NOT sandboxed — findings are live, not contained",
        )
    } else {
        (
            Severity::Info,
            "process appears sandboxed — ambient findings are partially contained",
        )
    };

    let finding = Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        title,
        severity,
        confidence,
    )
    .summary(if sandboxed {
        format!("{verdict} (mechanisms: {})", mechanisms.join(", "))
    } else {
        "no namespace/seccomp/proxy containment detected; a same-user process here has full ambient authority".to_string()
    })
    .evidence(json!({
        "verdict": verdict,
        "mechanisms": mechanisms,
        "namespaces": ns_json,
        "namespaces_isolated_count": isolated_count,
        "net_isolated": net_isolated,
        "seccomp_mode": seccomp,
        "afunix_blocked": afunix_blocked,
        "http_proxy_set": proxy_set,
        "proc_environ_readable": environ_readable,
        "pid_count_visible": pid_count,
        "ia32_execution_supported": ia32_support,
        "note": "Detection is heuristic: /proc/1/ns is usually unreadable to non-root, so net/pid isolation falls back to interface/process-count heuristics. ia32 support flags the 32-bit socketcall seccomp-bypass surface (finding #5).",
    }))
    .remediation(&[
        "Run agents inside a sandbox (sandbox-runtime / devcontainer / VM); an un-sandboxed agent inherits full user authority.",
        "On hosts that don't need 32-bit, removing multilib closes the ia32 socketcall seccomp bypass.",
    ]);

    Ok(vec![finding])
}

/// True if the only network interface is loopback (a strong --unshare-net hint).
#[cfg(target_os = "linux")]
fn net_only_loopback() -> Option<bool> {
    let text = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut ifaces = 0usize;
    let mut only_lo = true;
    for line in text.lines().skip(2) {
        if let Some(name) = line.split(':').next() {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            ifaces += 1;
            if name != "lo" {
                only_lo = false;
            }
        }
    }
    if ifaces == 0 {
        None
    } else {
        Some(only_lo)
    }
}

/// Read an integer field from /proc/self/status (e.g. "Seccomp").
#[cfg(target_os = "linux")]
fn read_status_field(field: &str) -> Option<u32> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(field) {
            let rest = rest.trim_start_matches(':').trim();
            if let Some(tok) = rest.split_whitespace().next() {
                return tok.parse().ok();
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn platform_run(probe: &SandboxDetectProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "sandbox self-detection — Linux only",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("namespace/seccomp self-detection is implemented for Linux")
    .evidence(json!({ "platform_supported": false }))])
}
