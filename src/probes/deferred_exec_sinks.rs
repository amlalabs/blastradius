//! §extra (finding #13 / doc §4.1) — deferred-execution sinks.
//!
//! Inventories files an agent could write that execute LATER, OUTSIDE any bwrap
//! sandbox — when the developer runs a build/test, CI runs, direnv loads, an
//! editor opens the workspace, or the session restarts. The Bash sandbox never
//! sees these; the unsandboxed Write/Edit tools can plant them. Two findings:
//! repo-scoped sinks (CurrentRepo) and home autostart (Ambient).
//!
//! READ-ONLY: only presence + writability (mode/ownership inference). Never
//! writes. For `package.json`, counts lifecycle-script KEY names only (no values).

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::read::read_to_string_capped;

pub struct DeferredExecSinksProbe;

/// Repo-local files whose contents run later outside the sandbox.
const REPO_SINKS: &[&str] = &[
    "Makefile",
    "makefile",
    "GNUmakefile",
    "justfile",
    ".justfile",
    "Rakefile",
    "Taskfile.yml",
    "Taskfile.yaml",
    ".envrc",
    "package.json",
    ".gitlab-ci.yml",
    ".vscode/tasks.json",
    ".vscode/settings.json",
    ".vscode/launch.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "pyproject.toml",
    "setup.py",
    "conftest.py",
    // Editor / IDE run configs that execute on open / task-run.
    ".idea",
    // Dev-environment bootstrappers that run on entry / setup.
    ".devcontainer/devcontainer.json",
    ".pre-commit-config.yaml",
    ".husky",
    ".lefthook.yml",
    ".tool-versions",
    "mise.toml",
    ".mise.toml",
    "flake.nix",
    "shell.nix",
    "default.nix",
    "Brewfile",
    ".bazelrc",
    "Procfile",
];

/// npm/yarn lifecycle scripts that auto-run on `install` (no explicit invoke).
const LIFECYCLE_SCRIPTS: &[&str] = &[
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "prepublish",
    "prepublishOnly",
];

impl Probe for DeferredExecSinksProbe {
    fn id(&self) -> &'static str {
        "host.deferred_exec_sinks"
    }
    fn class(&self) -> FindingClass {
        FindingClass::HostPersistence
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(unix)]
fn platform_run(probe: &DeferredExecSinksProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    use crate::util::fsperm::{check, dir_writable_searchable, home_identity};
    use crate::util::paths::shorten;

    let home = match &ctx.home {
        Some(h) => h.clone(),
        None => {
            return Ok(vec![Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::CurrentRepo,
                "deferred-exec sinks — home unknown",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("home directory unknown; cannot resolve write-ownership identity")
            .evidence(json!({ "assessed": false }))]);
        }
    };
    let (uid, gid) = match home_identity(&home) {
        Some(x) => x,
        None => {
            return Ok(vec![Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::CurrentRepo,
                "deferred-exec sinks — home unreadable",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("could not stat home directory")
            .evidence(json!({ "assessed": false }))]);
        }
    };

    let mut findings = Vec::new();

    // ---- (1) repo-scoped sinks (CurrentRepo) ----
    let root = ctx.checkout_root.clone().or_else(|| ctx.repo_root.clone());
    if let Some(root) = root {
        let mut present = 0usize;
        let mut writable = 0usize;
        let mut nonowner = 0usize;
        let mut entries: Vec<serde_json::Value> = Vec::new();

        for rel in REPO_SINKS {
            let p = root.join(rel);
            let c = check(&p, uid, gid);
            if !c.exists {
                continue;
            }
            present += 1;
            if c.writable {
                writable += 1;
            }
            if c.nonowner_writable {
                nonowner += 1;
            }
            entries.push(json!({
                "name": rel,
                "writable": c.writable,
                "nonowner_writable": c.nonowner_writable,
            }));
        }

        // package.json lifecycle scripts that auto-run on install.
        let mut lifecycle: Vec<String> = Vec::new();
        let pkg = root.join("package.json");
        if let Ok(text) = read_to_string_capped(&pkg, 4 * 1024 * 1024) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
                    for k in scripts.keys() {
                        if LIFECYCLE_SCRIPTS.contains(&k.as_str()) {
                            lifecycle.push(k.clone());
                        }
                    }
                }
            }
        }

        // CI workflow files (count only).
        let mut ci_workflows = 0usize;
        if let Ok(rd) = std::fs::read_dir(root.join(".github/workflows")) {
            ci_workflows = rd
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|x| x == "yml" || x == "yaml")
                        .unwrap_or(false)
                })
                .count();
        }

        let auto_lifecycle = !lifecycle.is_empty();
        let (severity, title) = if nonowner > 0 {
            (
                Severity::Exposed,
                "deferred-exec sinks writable by a non-owner principal",
            )
        } else if writable > 0 {
            (
                Severity::Notable,
                "writable deferred-execution sinks in this repo",
            )
        } else if present > 0 {
            (Severity::Info, "deferred-exec sinks present (not writable)")
        } else {
            (Severity::Info, "no deferred-execution sinks in this repo")
        };

        findings.push(
            Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::CurrentRepo,
                title,
                severity,
                Confidence::Confirmed,
            )
            .summary(if present == 0 {
                "no build/CI/editor/direnv/lockfile sinks found in this repo".to_string()
            } else {
                format!(
                    "{present} deferred-exec sink(s), {writable} writable{} — code written here runs later, outside bwrap, under your authority",
                    if auto_lifecycle {
                        format!("; package.json has {} auto-run lifecycle script(s)", lifecycle.len())
                    } else {
                        String::new()
                    }
                )
            })
            .evidence(json!({
                "root": shorten(&root, Some(&home)),
                "present": present,
                "writable": writable,
                "nonowner_writable": nonowner,
                "sinks": entries,
                "package_json_lifecycle_scripts": lifecycle,
                "ci_workflow_files": ci_workflows,
                "note": "These execute outside the Bash sandbox (npm install, CI, direnv, editor, next commit). The Write/Edit tools are not sandboxed, so an agent can plant them even when sandbox.enabled=true.",
            }))
            .remediation(&[
                "Apply sandbox write-denies to deferred-exec sinks even inside the workspace (build/CI/editor/direnv files, lockfiles).",
                "Review agent edits to these files as code-execution changes, not config.",
            ]),
        );
    }

    // ---- (2) home autostart sinks (Ambient) ----
    let autostart_dirs = [
        home.join(".config/autostart"),
        home.join(".config/systemd/user"),
        home.join(".config/environment.d"),
        // macOS user persistence (no-op on Linux, where these don't exist).
        home.join("Library/LaunchAgents"),
    ];
    let mut autostart_entries: Vec<serde_json::Value> = Vec::new();
    let mut any_writable_dir = false;
    let mut any_nonowner = false;
    for d in &autostart_dirs {
        if !d.exists() {
            continue;
        }
        let writable = dir_writable_searchable(d, uid, gid);
        let c = check(d, uid, gid);
        if writable {
            any_writable_dir = true;
        }
        if c.nonowner_writable {
            any_nonowner = true;
        }
        let count = std::fs::read_dir(d)
            .map(|r| r.flatten().count())
            .unwrap_or(0);
        autostart_entries.push(json!({
            "path": shorten(d, Some(&home)),
            "dir_writable": writable,
            "nonowner_writable": c.nonowner_writable,
            "entry_count": count,
        }));
    }

    // Scheduled jobs (cron + systemd user timers) — time-triggered persistence
    // that runs outside any sandbox.
    let timer_count = std::fs::read_dir(home.join(".config/systemd/user"))
        .map(|r| {
            r.flatten()
                .filter(|e| e.path().extension().map(|x| x == "timer").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let crontab_lines = crate::util::command::run_stdout("crontab", &["-l"], None)
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                .count()
        })
        .unwrap_or(0);

    if !autostart_entries.is_empty() || timer_count > 0 || crontab_lines > 0 {
        let sev = if any_nonowner {
            Severity::Exposed
        } else if any_writable_dir || timer_count > 0 || crontab_lines > 0 {
            Severity::Notable
        } else {
            Severity::Info
        };
        findings.push(
            Finding::new(
                "host.autostart_sinks",
                probe.class(),
                FindingScope::Ambient,
                if any_writable_dir {
                    "writable user-autostart / scheduled-job surface"
                } else {
                    "user-autostart / scheduled-job surface present"
                },
                sev,
                Confidence::Confirmed,
            )
            .summary(format!(
                "{} autostart location(s), {timer_count} systemd timer(s), {crontab_lines} crontab line(s){} — these run at login/session start or on a schedule, outside any sandbox",
                autostart_entries.len(),
                if any_writable_dir { ", writable" } else { "" }
            ))
            .evidence(json!({
                "locations": autostart_entries,
                "systemd_user_timers": timer_count,
                "user_crontab_lines": crontab_lines,
            }))
            .remediation(&[
                "Keep agent write scope away from ~/.config/autostart, ~/.config/systemd/user, and the user crontab.",
                "Audit existing systemd user timers and crontab entries — they run on a schedule outside any sandbox.",
            ]),
        );
    }

    if findings.is_empty() {
        findings.push(
            Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::CurrentRepo,
                "no deferred-execution sinks found",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("no repo or autostart deferred-execution sinks detected")
            .evidence(json!({ "present": 0 })),
        );
    }

    Ok(findings)
}

#[cfg(not(unix))]
fn platform_run(probe: &DeferredExecSinksProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::CurrentRepo,
        "deferred-exec sinks — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("writability inference is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}
