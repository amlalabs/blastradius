//! §extra — sandbox enforcement-binary integrity (the bwrap/socat trusted path).
//!
//! `@anthropic-ai/sandbox-runtime` execs its enforcement binaries by **bare name**
//! (`bwrap`, `socat`) unless the caller passes an absolute `bwrapPath`/`socatPath`,
//! and performs no integrity check (`linux-sandbox-utils.ts:1290`). Static analysis
//! of the Claude Code client (see docs/claude-code-security-model.md §2.5) found it
//! does not pin absolute paths by default. So whichever `bwrap`/`socat` is first on
//! `$PATH` at exec time *is* the sandbox — and code running as the user (which the
//! unsandboxed Write tool always is) can shadow it.
//!
//! This probe resolves `bwrap`/`socat` exactly as a bare-name exec would, then
//! decides whether that resolution is **replaceable or shadowable** by the running
//! user — the enforcement binary being reachable-for-write is the finding. It is the
//! runc CVE-2019-5736 class: the trusted tool is writable by the thing it contains.
//!
//! READ-ONLY: nothing is written. Writability is inferred from host DAC (mode bits +
//! ownership vs the home-directory owner), the same model as `write_reach`. It does
//! NOT consult ACLs, immutable attrs, or mounts.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::read::read_to_string_capped;

pub struct SandboxIntegrityProbe;

/// The Linux sandbox enforcement binaries the runtime shells out to.
const TOOLS: &[&str] = &["bwrap", "socat"];

impl Probe for SandboxIntegrityProbe {
    fn id(&self) -> &'static str {
        "host.sandbox_binary_integrity"
    }
    fn class(&self) -> FindingClass {
        FindingClass::HostPersistence
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(unix)]
fn platform_run(probe: &SandboxIntegrityProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    use std::os::unix::fs::MetadataExt;

    // Identity = the owner of $HOME (owned by the running user by definition),
    // avoiding any geteuid/libc dependency — matches write_reach.
    let home = match &ctx.home {
        Some(h) => h.clone(),
        None => {
            return Ok(vec![info(
                probe,
                "home unknown; cannot resolve write identity",
            )])
        }
    };
    let (uid, gid) = match std::fs::metadata(&home) {
        Ok(m) => (m.uid(), m.gid()),
        Err(_) => {
            return Ok(vec![info(
                probe,
                "home unreadable; cannot resolve write identity",
            )])
        }
    };

    let path_dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .as_ref()
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();

    // Cross-reference Claude Code settings: an absolute, non-writable
    // `sandbox.bwrapPath`/`socatPath` pin defeats the bare-name PATH hijack
    // (the client resolves the pin, not $PATH). A pin to a *writable* path is no
    // better, so the writability check still governs severity.
    let pins = read_pinned_paths(ctx);

    let reports: Vec<imp::ToolReport> = TOOLS
        .iter()
        .map(|name| {
            let pin = match *name {
                "bwrap" => pins.bwrap.as_ref(),
                "socat" => pins.socat.as_ref(),
                _ => None,
            };
            imp::analyze(name, &path_dirs, uid, gid, &home, pin)
        })
        .collect();

    let any_found = reports.iter().any(|r| r.found);
    let any_shadowable = reports.iter().any(|r| r.shadowable);
    let setuid_replaceable = reports
        .iter()
        .any(|r| r.found && r.setuid_root && r.shadowable);
    // All found tools are pinned to an absolute path that is NOT shadowable.
    let all_found_pinned_safe = any_found
        && reports
            .iter()
            .filter(|r| r.found)
            .all(|r| r.pinned_absolute && !r.shadowable);

    let (severity, title) = if any_shadowable {
        (
            Severity::Exposed,
            "sandbox enforcement binary is replaceable by code running as you",
        )
    } else if all_found_pinned_safe {
        (
            Severity::Info,
            "sandbox enforcement binaries pinned to trusted absolute paths",
        )
    } else if any_found {
        (
            Severity::Notable,
            "sandbox enforcement binaries resolve via bare-name PATH lookup",
        )
    } else {
        (
            Severity::Info,
            "sandbox enforcement binaries (bwrap/socat) not found on PATH",
        )
    };

    let summary = if setuid_replaceable {
        "a setuid-root sandbox binary sits on a user-writable path — replacement is privilege escalation, not just sandbox bypass".to_string()
    } else if any_shadowable {
        "bwrap/socat resolve to a user-writable or PATH-shadowable location — a fake binary would silently defeat the sandbox while it reports success".to_string()
    } else if all_found_pinned_safe {
        "bwrap/socat are pinned to absolute, non-writable paths in Claude Code settings — bare-name PATH hijack does not apply".to_string()
    } else if any_found {
        "bwrap/socat are resolved by bare name (no absolute pin); not currently shadowable, but resolution depends on PATH at exec time".to_string()
    } else {
        "neither bwrap nor socat is on PATH; the Linux sandbox cannot run from here".to_string()
    };

    let confidence = if any_found {
        Confidence::Confirmed
    } else {
        Confidence::Likely
    };

    let finding = Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        title,
        severity,
        confidence,
    )
    .summary(summary)
    .evidence(json!({
        "tools": reports.iter().map(|r| r.json.clone()).collect::<Vec<_>>(),
        "path_total": path_dirs.len(),
        "any_shadowable": any_shadowable,
        "setuid_root_replaceable": setuid_replaceable,
        "all_found_pinned_safe": all_found_pinned_safe,
        "settings_pin": {
            "bwrap_pinned": pins.bwrap.is_some(),
            "socat_pinned": pins.socat.is_some(),
            "pin_from_managed": pins.any_managed(),
            "note": "sandbox.bwrapPath/socatPath from Claude Code settings (managed scope wins). An absolute, non-writable pin removes the bare-name PATH-hijack; a writable pin does not.",
        },
        "assumptions": "Writability inferred from host DAC mode bits + ownership (uid/gid vs home owner). A writable PATH directory preceding the resolved one, or a writable resolved dir, suffices to shadow/replace via unlink+create. ACLs, immutable attrs, and mounts are not consulted.",
    }))
    .remediation(&[
        "Pin absolute, root-owned bwrapPath/socatPath in Claude Code sandbox settings (ideally managed) so resolution can't be hijacked.",
        "Remove user-writable directories that precede the system bin dirs on $PATH.",
        "Install bubblewrap/socat from system packages into root-owned /usr/bin; keep those dirs non-user-writable.",
    ]);

    Ok(vec![finding])
}

#[cfg(unix)]
fn info(probe: &SandboxIntegrityProbe, why: &str) -> Finding {
    Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "sandbox binary integrity — not assessed",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary(why.to_string())
    .evidence(json!({ "assessed": false }))
}

/// A configured `bwrapPath`/`socatPath` pin and whether it came from a managed
/// (admin-controlled) settings scope.
#[cfg(unix)]
pub struct Pin {
    pub path: String,
    pub managed: bool,
}

#[cfg(unix)]
pub struct PinnedPaths {
    pub bwrap: Option<Pin>,
    pub socat: Option<Pin>,
}

#[cfg(unix)]
impl PinnedPaths {
    fn any_managed(&self) -> bool {
        self.bwrap.as_ref().map(|p| p.managed).unwrap_or(false)
            || self.socat.as_ref().map(|p| p.managed).unwrap_or(false)
    }
}

/// Read `sandbox.bwrapPath` / `sandbox.socatPath` (or top-level `bwrapPath`/
/// `socatPath`) from Claude Code settings, managed scope winning. Value-free
/// apart from the configured path itself (a policy string, not a secret).
#[cfg(unix)]
fn read_pinned_paths(ctx: &Context) -> PinnedPaths {
    const CAP: u64 = 4 * 1024 * 1024;

    // (path, is_managed) in no particular order; managed wins at merge time.
    let mut candidates: Vec<(std::path::PathBuf, bool)> = Vec::new();
    if let Some(h) = &ctx.home {
        candidates.push((h.join(".claude/settings.json"), false));
        candidates.push((h.join(".claude/settings.local.json"), false));
    }
    candidates.push((
        std::path::PathBuf::from("/etc/claude-code/managed-settings.json"),
        true,
    ));
    candidates.push((
        std::path::PathBuf::from("/Library/Application Support/ClaudeCode/managed-settings.json"),
        true,
    ));
    if let Some(root) = &ctx.checkout_root {
        candidates.push((root.join(".claude/settings.json"), false));
        candidates.push((root.join(".claude/settings.local.json"), false));
    }

    let extract = |v: &serde_json::Value, key: &str| -> Option<String> {
        v.get("sandbox")
            .and_then(|s| s.get(key))
            .and_then(|x| x.as_str())
            .or_else(|| v.get(key).and_then(|x| x.as_str()))
            .map(|s| s.to_string())
    };

    let mut bwrap: Option<Pin> = None;
    let mut socat: Option<Pin> = None;

    for (path, managed) in candidates {
        let text = match read_to_string_capped(&path, CAP) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let val: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(p) = extract(&val, "bwrapPath") {
            // Managed value wins; otherwise keep the first non-managed found.
            if managed || bwrap.is_none() {
                bwrap = Some(Pin { path: p, managed });
            }
        }
        if let Some(p) = extract(&val, "socatPath") {
            if managed || socat.is_none() {
                socat = Some(Pin { path: p, managed });
            }
        }
    }

    PinnedPaths { bwrap, socat }
}

#[cfg(unix)]
mod imp {
    use serde_json::json;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use crate::util::paths::shorten;

    pub struct ToolReport {
        pub json: serde_json::Value,
        pub found: bool,
        pub shadowable: bool,
        pub setuid_root: bool,
        /// The tool is pinned to an absolute path (resolution can't be PATH-hijacked).
        pub pinned_absolute: bool,
    }

    /// (writable, world_writable, group_writable_nonowner) for an identity.
    fn writable(meta: &std::fs::Metadata, uid: u32, gid: u32) -> bool {
        let mode = meta.permissions().mode();
        let owner_w = mode & 0o200 != 0 && meta.uid() == uid;
        let group_w = mode & 0o020 != 0 && meta.gid() == gid;
        let world_w = mode & 0o002 != 0;
        owner_w || group_w || world_w
    }

    /// A directory is replaceable-into only if writable AND searchable: dir write
    /// permission alone lets you unlink+recreate entries (replacing a binary even
    /// if the file itself is root-owned).
    fn dir_writable_searchable(p: &Path, uid: u32, gid: u32) -> bool {
        let meta = match std::fs::metadata(p) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if !writable(&meta, uid, gid) {
            return false;
        }
        let mode = meta.permissions().mode();
        let owner_x = mode & 0o100 != 0 && meta.uid() == uid;
        let group_x = mode & 0o010 != 0 && meta.gid() == gid;
        let world_x = mode & 0o001 != 0;
        owner_x || group_x || world_x
    }

    fn is_executable(p: &Path) -> bool {
        match std::fs::metadata(p) {
            Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }

    /// Stat an absolute pinned path: found?, writable-in-place?, setuid-root?
    fn pin_facts(p: &Path, uid: u32, gid: u32) -> (bool, bool, bool, bool) {
        let found = is_executable(p);
        let (file_writable, setuid_root) = match std::fs::metadata(p) {
            Ok(m) => (
                writable(&m, uid, gid),
                m.permissions().mode() & 0o4000 != 0 && m.uid() == 0,
            ),
            Err(_) => (false, false),
        };
        let dir_writable = p
            .parent()
            .map(|d| dir_writable_searchable(d, uid, gid))
            .unwrap_or(false);
        (found, file_writable, dir_writable, setuid_root)
    }

    /// Resolve `name`, honoring an absolute settings pin if present, and assess
    /// shadowability. `pin` is `sandbox.bwrapPath`/`socatPath` from settings.
    pub fn analyze(
        name: &str,
        path_dirs: &[PathBuf],
        uid: u32,
        gid: u32,
        home: &Path,
        pin: Option<&super::Pin>,
    ) -> ToolReport {
        // An ABSOLUTE pin defeats PATH hijack: the client resolves the pin, not
        // $PATH. Severity then turns purely on whether the pinned path is
        // writable-in-place. A relative pin is not trusted — fall through.
        if let Some(pin) = pin {
            let pp = PathBuf::from(&pin.path);
            if pp.is_absolute() {
                let (found, file_writable, dir_writable, setuid_root) = pin_facts(&pp, uid, gid);
                let shadowable = found && (file_writable || dir_writable);
                return ToolReport {
                    json: json!({
                        "name": name,
                        "found": found,
                        "resolution": "settings-pin-absolute",
                        "pinned": true,
                        "pinned_managed": pin.managed,
                        "pinned_absolute": true,
                        "resolved_path": shorten(&pp, Some(home)),
                        "file_writable": file_writable,
                        "dir_writable": dir_writable,
                        "setuid_root": setuid_root,
                        "shadowable": shadowable,
                    }),
                    found,
                    shadowable,
                    setuid_root,
                    pinned_absolute: found && !shadowable,
                };
            }
        }
        let pin_relative = pin.is_some();

        // First PATH entry holding an executable `name`.
        let mut resolved: Option<(usize, PathBuf)> = None;
        for (i, dir) in path_dirs.iter().enumerate() {
            let cand = dir.join(name);
            if is_executable(&cand) {
                resolved = Some((i, cand));
                break;
            }
        }
        let (resolved_index, resolved_path) = match resolved {
            Some(x) => x,
            None => {
                return ToolReport {
                    json: json!({
                        "name": name,
                        "found": false,
                        "resolution": "path-bare-name",
                        "pinned": pin_relative,
                        "pin_relative": pin_relative,
                        "pinned_absolute": false,
                    }),
                    found: false,
                    shadowable: false,
                    setuid_root: false,
                    pinned_absolute: false,
                }
            }
        };

        // Resolved file: writable in place? setuid root?
        let (file_writable, setuid_root) = match std::fs::metadata(&resolved_path) {
            Ok(m) => (
                writable(&m, uid, gid),
                m.permissions().mode() & 0o4000 != 0 && m.uid() == 0,
            ),
            Err(_) => (false, false),
        };

        // Resolved directory: write+search lets you unlink+replace the entry.
        let dir_writable = resolved_path
            .parent()
            .map(|d| dir_writable_searchable(d, uid, gid))
            .unwrap_or(false);

        // Earlier PATH entries that let you plant a winning shadow.
        let mut earlier_writable: Vec<serde_json::Value> = Vec::new();
        let mut earlier_creatable = 0usize;
        for (i, dir) in path_dirs.iter().enumerate().take(resolved_index) {
            if dir.exists() {
                if dir_writable_searchable(dir, uid, gid) {
                    earlier_writable.push(json!({ "path": shorten(dir, Some(home)), "index": i }));
                }
            } else if dir
                .parent()
                .map(|p| dir_writable_searchable(p, uid, gid))
                .unwrap_or(false)
            {
                earlier_creatable += 1;
            }
        }

        let shadowable =
            file_writable || dir_writable || !earlier_writable.is_empty() || earlier_creatable > 0;

        ToolReport {
            json: json!({
                "name": name,
                "found": true,
                "resolution": "path-bare-name",
                "pinned": pin_relative,
                "pin_relative": pin_relative,
                "pinned_absolute": false,
                "resolved_path": shorten(&resolved_path, Some(home)),
                "resolved_index": resolved_index,
                "file_writable": file_writable,
                "dir_writable": dir_writable,
                "setuid_root": setuid_root,
                "earlier_writable_dirs": earlier_writable,
                "earlier_creatable_dir_count": earlier_creatable,
                "shadowable": shadowable,
            }),
            found: true,
            shadowable,
            setuid_root,
            pinned_absolute: false,
        }
    }
}

#[cfg(not(unix))]
fn platform_run(probe: &SandboxIntegrityProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Host,
        "sandbox binary integrity — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("bare-name PATH resolution analysis is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}

#[cfg(all(test, unix))]
mod tests {
    use super::imp;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;

    #[test]
    fn writable_path_dir_is_shadowable() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Plant a fake, executable `bwrap` in a dir we own (== writable).
        let fake = dir.join("bwrap");
        std::fs::write(&fake, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        let mut perms = std::fs::metadata(&fake).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&fake, perms).unwrap();

        // Identity = owner of the temp dir (the test user).
        let meta = std::fs::metadata(dir).unwrap();
        let report = imp::analyze(
            "bwrap",
            &[dir.to_path_buf()],
            meta.uid(),
            meta.gid(),
            dir,
            None,
        );
        assert!(report.found);
        assert!(
            report.shadowable,
            "a user-owned PATH dir must be flagged shadowable"
        );
        assert!(!report.pinned_absolute);
        // No secret-shaped strings in the evidence.
        let s = serde_json::to_string(&report.json).unwrap();
        assert!(!crate::report::redaction::contains_secret_shaped(&s));
    }

    #[test]
    fn missing_tool_is_not_found() {
        let report = imp::analyze(
            "definitely-not-a-real-binary-xyz",
            &[PathBuf::from("/nonexistent-dir-xyz")],
            0,
            0,
            std::path::Path::new("/"),
            None,
        );
        assert!(!report.found);
        assert!(!report.shadowable);
    }

    #[test]
    fn absolute_pin_to_root_owned_path_is_safe() {
        // A real root-owned, non-writable executable to stand in for a pinned
        // bwrap. /bin/sh exists and is root-owned on essentially all unixes.
        let candidates = ["/bin/sh", "/bin/true", "/usr/bin/true"];
        let pinned = candidates.iter().find(|p| std::path::Path::new(p).exists());
        let pinned = match pinned {
            Some(p) => p,
            None => return, // unusual host; skip
        };
        // Identity = a non-root uid (1000) so root-owned files aren't "ours".
        let pin = super::Pin {
            path: pinned.to_string(),
            managed: true,
        };
        let report = imp::analyze(
            "bwrap",
            &[],
            1000,
            1000,
            std::path::Path::new("/home/x"),
            Some(&pin),
        );
        assert!(report.found, "pinned path should resolve");
        assert!(
            !report.shadowable,
            "a root-owned absolute pin must not be shadowable by a non-root user"
        );
        assert!(report.pinned_absolute);
    }

    #[test]
    fn absolute_pin_to_writable_path_stays_shadowable() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = tmp.path().join("bwrap");
        std::fs::write(&fake, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        let mut perms = std::fs::metadata(&fake).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&fake, perms).unwrap();
        let meta = std::fs::metadata(tmp.path()).unwrap();
        let pin = super::Pin {
            path: fake.to_string_lossy().to_string(),
            managed: false,
        };
        let report = imp::analyze("bwrap", &[], meta.uid(), meta.gid(), tmp.path(), Some(&pin));
        assert!(report.found);
        assert!(
            report.shadowable,
            "pinning to a user-writable path is no protection"
        );
        assert!(!report.pinned_absolute);
    }
}
