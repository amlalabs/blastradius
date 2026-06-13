//! §extra — process-memory introspection reach (Linux).
//!
//! Two surfaces people forget the agent inherits, both about *other* processes
//! running as the same user:
//!   A) **ptrace / memory read** — when `kernel.yama.ptrace_scope` is 0 (or YAMA
//!      is absent), code running as you can attach to and dump the memory of ANY
//!      same-uid process: ssh-agent and gpg-agent (decrypted keys), your browser
//!      (session cookies, saved passwords in memory), a running password manager.
//!      No file on disk is involved — the secrets are read straight from RAM.
//!   B) **command-line secrets** — secrets passed as CLI args (`--token=…`,
//!      `mysql -pPASS`) are visible to every same-uid process via `/proc/*/cmdline`.
//!
//! READ-ONLY and value-free. (A) reads `/proc/sys/*` sysctls. (B) counts matching
//! processes via the canonical secret-shape detector; the command line itself is
//! never stored or emitted.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct ProcessIntrospectProbe;

impl Probe for ProcessIntrospectProbe {
    fn id(&self) -> &'static str {
        "process.memory_introspection"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(target_os = "linux")]
fn platform_run(probe: &ProcessIntrospectProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![ptrace_finding(probe), cmdline_finding()])
}

#[cfg(target_os = "linux")]
fn read_sysctl_int(path: &str) -> Option<i64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

#[cfg(target_os = "linux")]
fn ptrace_finding(probe: &ProcessIntrospectProbe) -> Finding {
    // Absent YAMA ⇒ classic ptrace rules ⇒ any same-uid process is attachable.
    let yama_present = std::path::Path::new("/proc/sys/kernel/yama/ptrace_scope").exists();
    let scope = read_sysctl_int("/proc/sys/kernel/yama/ptrace_scope");
    let perf_paranoid = read_sysctl_int("/proc/sys/kernel/perf_event_paranoid");
    let bpf_restricted = read_sysctl_int("/proc/sys/kernel/unprivileged_bpf_disabled");

    // Unrestricted same-uid ptrace when scope==0, or YAMA absent entirely.
    let unrestricted = matches!(scope, Some(0)) || (!yama_present && scope.is_none());
    let child_only = matches!(scope, Some(1));

    let (severity, confidence, title, summary) = if unrestricted {
        (
            Severity::Exposed,
            Confidence::Confirmed,
            "agent can read the memory of any process you own",
            "ptrace_scope=0 (or YAMA absent): code running as you can dump the live memory of any same-uid process — ssh-agent/gpg-agent keys, browser sessions, password managers — straight from RAM".to_string(),
        )
    } else if child_only {
        (
            Severity::Notable,
            Confidence::Likely,
            "ptrace restricted to descendants (still attach-on-spawn)",
            "ptrace_scope=1: arbitrary same-uid attach is blocked, but a process the agent spawns can still be traced, and the agent can relax the scope for its own children".to_string(),
        )
    } else {
        (
            Severity::Info,
            Confidence::Confirmed,
            "process-memory introspection restricted",
            format!("ptrace_scope={} — broad same-uid memory reads are restricted", scope.map(|s| s.to_string()).unwrap_or_else(|| "n/a".into())),
        )
    };

    Finding::new(
        probe.id(),
        FindingClass::Process,
        FindingScope::Host,
        title,
        severity,
        confidence,
    )
    .summary(summary)
    .evidence(json!({
        "yama_present": yama_present,
        "ptrace_scope": scope,
        "perf_event_paranoid": perf_paranoid,
        "unprivileged_bpf_disabled": bpf_restricted,
        "note": "ptrace_scope: 0=any same-uid attachable, 1=descendants only, 2=admin only, 3=disabled. Memory reads need no file on disk.",
    }))
    .remediation(&[
        "Set kernel.yama.ptrace_scope=1 (or higher) so agents can't dump other processes' memory.",
        "Run agents in a separate PID namespace / user so ssh-agent, gpg-agent, and your browser aren't same-uid neighbors.",
    ])
}

#[cfg(target_os = "linux")]
fn cmdline_finding() -> Finding {
    use crate::report::redaction::contains_secret_shaped;
    use std::os::unix::fs::MetadataExt;

    let own_uid = std::fs::metadata("/proc/self").map(|m| m.uid()).ok();
    let mut same_uid = 0usize;
    let mut with_secret_args = 0usize;

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = match name.to_str() {
                Some(s) if s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let dir = e.path();
            let uid = match std::fs::metadata(&dir) {
                Ok(m) => m.uid(),
                Err(_) => continue,
            };
            if Some(uid) != own_uid {
                continue;
            }
            same_uid += 1;
            if let Ok(raw) = std::fs::read(dir.join("cmdline")) {
                // cmdline args are NUL-separated; join with spaces for shape scan.
                let joined: String = String::from_utf8_lossy(&raw).replace('\0', " ");
                if contains_secret_shaped(&joined) || looks_like_password_flag(&joined) {
                    with_secret_args += 1;
                }
            }
            let _ = name;
        }
    }

    let severity = if with_secret_args > 0 {
        Severity::Notable
    } else {
        Severity::Info
    };

    Finding::new(
        "process.cmdline_secrets",
        FindingClass::Process,
        FindingScope::Host,
        if with_secret_args > 0 {
            "secrets visible in other processes' command lines"
        } else {
            "no secret-shaped process command lines"
        },
        severity,
        Confidence::Likely,
    )
    .summary(if with_secret_args > 0 {
        format!("{with_secret_args} same-uid process(es) expose secret-shaped command-line args readable via /proc/*/cmdline")
    } else {
        "no same-uid process command lines contained secret-shaped arguments".to_string()
    })
    .evidence(json!({
        "same_uid_processes": same_uid,
        "processes_with_secret_args": with_secret_args,
        "note": "Counts only; the command lines themselves are never stored or emitted. Secrets passed as CLI args are visible to every same-uid process.",
    }))
    .remediation(&[
        "Never pass secrets as command-line arguments; they are world-visible (same-uid) via /proc/*/cmdline. Use env files or stdin.",
    ])
}

/// Heuristic: a `--password=…` / `-pSECRET` style flag with a value attached.
#[cfg(target_os = "linux")]
fn looks_like_password_flag(s: &str) -> bool {
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(--?(password|token|secret|api[_-]?key)[=\s]\S|-p\S{6,})").unwrap()
    })
    .is_match(s)
}

#[cfg(not(target_os = "linux"))]
fn platform_run(probe: &ProcessIntrospectProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        FindingClass::Process,
        FindingScope::Host,
        "process-memory introspection — Linux only",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("ptrace_scope / cmdline introspection checks are implemented for Linux")
    .evidence(json!({ "platform_supported": false }))])
}
