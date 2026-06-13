//! §extra — AF_UNIX / docker.sock reach and /proc/*/environ exposure.
//!
//! One probe emitting TWO Host-scoped findings, all READ-ONLY and value-free:
//!   A) whether AF_UNIX sockets can be created (a seccomp/sandbox signal) and
//!      whether a docker daemon socket is present and connectable (host root);
//!   B) Linux-only: whether other same-uid processes' /proc/<pid>/environ is
//!      readable (a secret-exfil surface).
//!
//! Only booleans, counts, error KINDS, and shortened paths leave this probe.
//! Where bytes are read (1 byte of an environ to test readability) they are
//! immediately discarded and never stored or emitted.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct SandboxReachProbe;

impl Probe for SandboxReachProbe {
    fn id(&self) -> &'static str {
        "process.sandbox_reach"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        Ok(vec![finding_a(ctx), finding_b(ctx)])
    }
}

// ---------------------------------------------------------------------------
// FINDING A — AF_UNIX socket creatability + docker.sock reach.
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn finding_a(ctx: &Context) -> Finding {
    use crate::util::paths::shorten;
    use std::io::ErrorKind;
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixDatagram;
    use std::path::PathBuf;

    // 1. Seccomp/sandbox signal: socket(AF_UNIX,SOCK_DGRAM,0) with NO bind /
    //    connect / path. Ok => creatable; EPERM => blocked (sandboxed).
    let (af_unix_socket_creatable, af_unix_error_kind, unknown_socket_err) =
        match UnixDatagram::unbound() {
            Ok(_sock) => (true, None, false),
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                (false, Some("PermissionDenied".to_string()), false)
            }
            // Non-EPERM (e.g. ENFILE/EMFILE) -> not a sandbox signal; unknown.
            Err(e) => (false, Some(format!("{:?}", e.kind())), true),
        };

    // 2. docker.sock presence.
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/var/run/docker.sock"),
        PathBuf::from("/run/docker.sock"),
    ];
    if cfg!(target_os = "macos") {
        if let Some(home) = &ctx.home {
            candidates.push(home.join(".docker/run/docker.sock"));
        }
    }

    // 3. DOCKER_HOST presence only (never the value).
    let docker_host_env_set = ctx.env.contains("DOCKER_HOST");

    let mut reachable_count = 0usize;
    let mut any_socket = false;
    let mut candidate_json: Vec<serde_json::Value> = Vec::new();

    for path in &candidates {
        let (exists, is_socket) = match std::fs::symlink_metadata(path) {
            Ok(m) => (true, m.file_type().is_socket()),
            Err(_) => (false, false),
        };
        if is_socket {
            any_socket = true;
        }

        // 4. Reachability: ONLY if AF_UNIX is creatable AND it is a socket.
        //    Connect, send ZERO bytes, drop. Bounded by a thread + timeout so a
        //    full accept backlog cannot hang the probe (UnixStream::connect has
        //    no timeout of its own).
        let reachable = if af_unix_socket_creatable && is_socket {
            connect_bounded(path)
        } else {
            false
        };
        if reachable {
            reachable_count += 1;
        }

        candidate_json.push(json!({
            "path": shorten(path, ctx.home.as_deref()),
            "exists": exists,
            "is_socket": is_socket,
            "reachable": reachable,
        }));
    }

    // Severity.
    let (severity, confidence, title, summary) = if reachable_count > 0 {
        (
            Severity::Exposed,
            Confidence::Likely,
            "docker.sock reachable via AF_UNIX — host-root takeover path open",
            format!("{reachable_count} docker.sock endpoint(s) connectable from this process"),
        )
    } else if any_socket {
        (
            Severity::Notable,
            Confidence::Likely,
            "docker.sock present but not connectable",
            "docker.sock exists but connect did not succeed (perms or backlog)".to_string(),
        )
    } else if !af_unix_socket_creatable && !unknown_socket_err {
        (
            Severity::Info,
            Confidence::Confirmed,
            "AF_UNIX socket creation blocked (sandbox confirmed)",
            "socket(AF_UNIX) returned EPERM — seccomp/seatbelt is blocking it".to_string(),
        )
    } else {
        (
            Severity::Info,
            if unknown_socket_err {
                Confidence::Unknown
            } else {
                Confidence::Confirmed
            },
            "no docker.sock reachable",
            "no docker daemon socket present in this namespace".to_string(),
        )
    };

    Finding::new(
        "process.afunix_docker_sock",
        FindingClass::Process,
        FindingScope::Host,
        title,
        severity,
        confidence,
    )
    .summary(summary)
    .evidence(json!({
        "af_unix_socket_creatable": af_unix_socket_creatable,
        "af_unix_error_kind": af_unix_error_kind,
        "docker_host_env_set": docker_host_env_set,
        "docker_sock_candidates": candidate_json,
        "reachable_count": reachable_count,
        "note": "Connect sends ZERO bytes then drops, mirroring egress.connectivity; the daemon may log an accepted connection (the only non-pure-read action).",
    }))
    .remediation(&[
        "Never bind the docker socket into agent environments — it is equivalent to host root.",
        "Run agents under seccomp that denies AF_UNIX (or a fresh namespace without docker.sock).",
    ])
}

/// Connect to a unix socket in a worker thread, bounded by a short timeout, send
/// zero bytes, and drop. Returns whether the connect succeeded within the bound.
#[cfg(unix)]
fn connect_bounded(path: &std::path::Path) -> bool {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::time::Duration;

    let p = path.to_path_buf();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let ok = UnixStream::connect(&p).is_ok();
        let _ = tx.send(ok);
        // Stream dropped here (zero bytes sent).
    });
    rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false)
}

#[cfg(not(unix))]
fn finding_a(_ctx: &Context) -> Finding {
    Finding::new(
        "process.afunix_docker_sock",
        FindingClass::Process,
        FindingScope::Host,
        "AF_UNIX / docker.sock probe — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("AF_UNIX reach probe is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))
}

// ---------------------------------------------------------------------------
// FINDING B — /proc/<pid>/environ exposure (Linux only).
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn finding_b(_ctx: &Context) -> Finding {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;

    // 1. Own uid via /proc/self (no libc needed).
    let own_uid = std::fs::metadata("/proc/self").map(|m| m.uid()).ok();

    // Helper: can we read at least 1 byte of an environ file? (Byte discarded.)
    let environ_readable = |path: &std::path::Path| -> bool {
        match std::fs::File::open(path) {
            Ok(mut f) => {
                let mut buf = [0u8; 1];
                matches!(f.read(&mut buf), Ok(n) if n > 0)
            }
            Err(_) => false,
        }
    };

    // 2. Our own environ readability.
    let proc_self_environ_readable = environ_readable(std::path::Path::new("/proc/self/environ"));

    // 3. Walk /proc for numeric pids.
    let mut proc_pids_total = 0usize;
    let mut same_uid_pids = 0usize;
    let mut other_pid_environ_readable = 0usize;

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = match name.to_str() {
                Some(s) => s,
                None => continue,
            };
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            proc_pids_total += 1;

            // Skip our own pid for the "other pid" accounting.
            let pid_dir = entry.path();
            let is_self = std::fs::canonicalize("/proc/self")
                .ok()
                .map(|s| s == pid_dir)
                .unwrap_or(false);
            if is_self {
                continue;
            }

            let uid = match std::fs::metadata(&pid_dir) {
                Ok(m) => m.uid(),
                Err(_) => continue,
            };
            if Some(uid) == own_uid {
                same_uid_pids += 1;
                if environ_readable(&pid_dir.join("environ")) {
                    other_pid_environ_readable += 1;
                }
            }
        }
    }

    // A small /proc indicates a fresh pid+proc namespace (secure sandbox), in
    // which same-uid siblings are sandbox helpers, not a host leak.
    let pid_namespace_isolated_hint = proc_pids_total < 8;

    // Severity (correction: gate Exposed on NOT isolated — same-namespace
    // sandbox helpers being readable is not a host leak).
    let (severity, title, summary) = if other_pid_environ_readable > 0 {
        if pid_namespace_isolated_hint {
            (
                Severity::Notable,
                "same-namespace sibling environ readable (isolated)",
                format!(
                    "{other_pid_environ_readable} same-uid sibling environ readable inside an isolated namespace (sandbox helpers, not a host leak)"
                ),
            )
        } else {
            (
                Severity::Exposed,
                "other same-uid processes' environ readable — secret exfil surface",
                format!(
                    "un-sandboxed baseline: agent can read {other_pid_environ_readable} other same-uid process(es)' environ"
                ),
            )
        }
    } else if same_uid_pids > 1 {
        (
            Severity::Notable,
            "same-uid processes present but environ unreadable",
            format!("{same_uid_pids} same-uid pids, none with readable environ"),
        )
    } else {
        (
            Severity::Info,
            "no other-process environ exposure",
            "only self / no readable same-uid environ".to_string(),
        )
    };

    Finding::new(
        "process.proc_environ",
        FindingClass::Process,
        FindingScope::Host,
        title,
        severity,
        Confidence::Confirmed,
    )
    .summary(summary)
    .evidence(json!({
        "platform_supported": true,
        "proc_self_environ_readable": proc_self_environ_readable,
        "proc_pids_total": proc_pids_total,
        "same_uid_pids": same_uid_pids,
        "other_pid_environ_readable": other_pid_environ_readable,
        "pid_namespace_isolated_hint": pid_namespace_isolated_hint,
        "note": "Same-uid environ readability is the Linux DAC default on any un-sandboxed box. Counts only; environ bytes are never stored.",
    }))
    .remediation(&[
        "Run agents in a fresh PID namespace so host processes are invisible.",
        "Avoid placing secrets in the environment of long-lived same-uid processes.",
    ])
}

#[cfg(not(target_os = "linux"))]
fn finding_b(_ctx: &Context) -> Finding {
    Finding::new(
        "process.proc_environ",
        FindingClass::Process,
        FindingScope::Host,
        "/proc environ probe — not applicable (no /proc)",
        Severity::Info,
        Confidence::Confirmed,
    )
    .summary("/proc/*/environ exposure check is Linux-only")
    .evidence(json!({ "platform_supported": false }))
}
