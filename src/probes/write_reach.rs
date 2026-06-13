//! §extra — writable persistence / escalation paths (read-only W_OK inference).
//!
//! Enumerates the classic agent-persistence surface — shell rc/profile files,
//! a few high-value home dotfiles, `$PATH` directories, and the current repo's
//! git hooks/config — and decides whether each is *writable* by the running
//! user using a pure permission check (file mode bits + ownership vs the home
//! directory owner). NO files are ever created, modified, or deleted.
//!
//! Writability here reflects host DAC (mode + ownership) under the
//! agent-runs-as-user condition (sandbox NOT engaged). It does NOT account for
//! bwrap ro-bind/tmpfs overlays, ACLs, immutable (chattr +i) attributes, or
//! read-only mounts. This is the persistence surface a sandbox would lock down,
//! not a claim that any sandbox "fails to deny" these writes.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct WriteReachProbe;

/// Shell rc/profile files under `$HOME` that load on every interactive shell —
/// the canonical agent-persistence write targets.
const SHELL_RC: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".bash_login",
    ".profile",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".zlogin",
    ".config/fish/config.fish",
];

/// Other high-value home dotfiles that can drive code execution / exfil.
/// `~/.ssh/authorized_keys` is a remote-login backdoor if writable; `~/.ssh/config`
/// can carry `ProxyCommand`/`LocalCommand` (command execution on `ssh`). Editor
/// rc files execute on file-open (autocmds/plugins/modelines); X-session and
/// PAM files execute at login.
const HOME_CONFIG: &[&str] = &[
    ".gitconfig",
    ".ripgreprc",
    ".mcp.json",
    ".ssh/authorized_keys",
    ".ssh/config",
    // Editor configs — run code when a repo file is opened.
    ".vimrc",
    ".config/nvim/init.vim",
    ".config/nvim/init.lua",
    ".config/Code/User/settings.json",
    ".tmux.conf",
    // Login / session startup — run code at login or X session start.
    ".xprofile",
    ".xinitrc",
    ".xsession",
    ".pam_environment",
    ".bash_logout",
];

impl Probe for WriteReachProbe {
    fn id(&self) -> &'static str {
        "host.writable_persistence_paths"
    }
    fn class(&self) -> FindingClass {
        FindingClass::HostPersistence
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

#[cfg(unix)]
fn platform_run(probe: &WriteReachProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    use crate::util::paths::shorten;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    // --- "us" identity: the owner of $HOME (home is owned by the running user
    // by definition), avoiding any geteuid/libc dependency. ---
    let home = match &ctx.home {
        Some(h) => h.clone(),
        None => {
            // Home unknown -> we cannot resolve ownership; report Info.
            return Ok(vec![Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::Ambient,
                "writable persistence paths — home unknown",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("home directory unknown; cannot resolve write-ownership identity")
            .evidence(json!({ "home_owner_resolved": false }))]);
        }
    };
    let (home_uid, home_gid) = match std::fs::metadata(&home) {
        Ok(m) => (m.uid(), m.gid()),
        Err(_) => {
            return Ok(vec![Finding::new(
                probe.id(),
                probe.class(),
                FindingScope::Ambient,
                "writable persistence paths — home unreadable",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("could not stat home directory; cannot resolve write-ownership identity")
            .evidence(json!({ "home_owner_resolved": false }))]);
        }
    };

    /// Outcome of a single writability check.
    struct Check {
        exists: bool,
        writable: bool,
        creatable: bool,
        is_symlink: bool,
        world_writable: bool,
        group_writable_nonowner: bool,
    }

    // Resolve mode/uid/gid by following symlinks (symlink mode bits are always
    // 0o777 and cannot decide writability — correction #1).
    let writable_target = |meta: &std::fs::Metadata| -> (bool, bool, bool) {
        let mode = meta.permissions().mode();
        let uid = meta.uid();
        let gid = meta.gid();
        let owner_w = mode & 0o200 != 0 && uid == home_uid;
        let group_w = mode & 0o020 != 0 && gid == home_gid;
        let world_w = mode & 0o002 != 0;
        let writable = owner_w || group_w || world_w;
        let group_writable_nonowner = mode & 0o020 != 0 && uid != home_uid;
        (writable, world_w, group_writable_nonowner)
    };

    // A directory is "creatable into" only if it is both writable AND has the
    // search/execute bit for the relevant principal (correction #2).
    let dir_writable_and_searchable = |path: &Path| -> bool {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let (writable, _, _) = writable_target(&meta);
        if !writable {
            return false;
        }
        let mode = meta.permissions().mode();
        let uid = meta.uid();
        let gid = meta.gid();
        let owner_x = mode & 0o100 != 0 && uid == home_uid;
        let group_x = mode & 0o010 != 0 && gid == home_gid;
        let world_x = mode & 0o001 != 0;
        owner_x || group_x || world_x
    };

    let check = |path: &Path| -> Check {
        match std::fs::symlink_metadata(path) {
            Ok(lmeta) => {
                let is_symlink = lmeta.file_type().is_symlink();
                // Follow the link (if any) to decide writability.
                match std::fs::metadata(path) {
                    Ok(tmeta) => {
                        let (writable, world_writable, group_writable_nonowner) =
                            writable_target(&tmeta);
                        Check {
                            exists: true,
                            writable,
                            creatable: false,
                            is_symlink,
                            world_writable,
                            group_writable_nonowner,
                        }
                    }
                    // Broken symlink -> target does not exist; not writable.
                    Err(_) => Check {
                        exists: true,
                        writable: false,
                        creatable: false,
                        is_symlink,
                        world_writable: false,
                        group_writable_nonowner: false,
                    },
                }
            }
            // Absent -> creatable iff parent dir is writable + searchable.
            Err(_) => {
                let creatable = path
                    .parent()
                    .map(dir_writable_and_searchable)
                    .unwrap_or(false);
                Check {
                    exists: false,
                    writable: false,
                    creatable,
                    is_symlink: false,
                    world_writable: false,
                    group_writable_nonowner: false,
                }
            }
        }
    };

    let mut writable_targets = 0usize;
    let mut creatable_targets = 0usize;
    let mut any_world_writable = false;
    let mut any_group_writable_nonowner = false;

    let mut entry = |path: &Path, name: Option<&str>| -> serde_json::Value {
        let c = check(path);
        if c.writable {
            writable_targets += 1;
        }
        if c.creatable {
            creatable_targets += 1;
        }
        if c.world_writable {
            any_world_writable = true;
        }
        if c.group_writable_nonowner {
            any_group_writable_nonowner = true;
        }
        let mut obj = json!({
            "path": shorten(path, Some(&home)),
            "exists": c.exists,
            "writable": c.writable,
            "creatable": c.creatable,
            "is_symlink": c.is_symlink,
            "world_writable": c.world_writable,
            "group_writable_nonowner": c.group_writable_nonowner,
        });
        if let Some(n) = name {
            obj.as_object_mut()
                .unwrap()
                .insert("name".to_string(), json!(n));
        }
        obj
    };

    // (a) shell rc/profile files.
    let shell_rc: Vec<serde_json::Value> = SHELL_RC
        .iter()
        .map(|rel| {
            let p = home.join(rel);
            entry(&p, None)
        })
        .collect();

    // (b) other high-value home dotfiles.
    let config_files: Vec<serde_json::Value> = HOME_CONFIG
        .iter()
        .map(|name| {
            let p = home.join(name);
            entry(&p, Some(name))
        })
        .collect();

    // (c) $PATH escalation. We read our own PATH (a non-secret-shaped value),
    // storing only dir names / shortened paths + indices, never the raw string.
    let path_var = std::env::var_os("PATH");
    let path_dirs: Vec<std::path::PathBuf> = path_var
        .as_ref()
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    let path_total = path_dirs.len();

    // First index of a standard system bin dir actually present in PATH.
    const SYSTEM_BINS: &[&str] = &["/usr/bin", "/bin", "/usr/local/bin", "/sbin", "/usr/sbin"];
    let first_system_bin_index = path_dirs
        .iter()
        .position(|d| SYSTEM_BINS.iter().any(|s| d.as_os_str() == *s));

    let mut writable_path_dirs: Vec<serde_json::Value> = Vec::new();
    let mut escalating_path_dirs = 0usize;
    for (index, dir) in path_dirs.iter().enumerate() {
        let c = check(dir);
        if !c.writable {
            continue;
        }
        let precedes_system_bin = match first_system_bin_index {
            Some(sbi) => index < sbi,
            None => false,
        };
        if precedes_system_bin || c.world_writable || c.group_writable_nonowner {
            escalating_path_dirs += 1;
        }
        if c.world_writable {
            any_world_writable = true;
        }
        if c.group_writable_nonowner {
            any_group_writable_nonowner = true;
        }
        writable_path_dirs.push(json!({
            "path": shorten(dir, Some(&home)),
            "index": index,
            "precedes_system_bin": precedes_system_bin,
            "world_writable": c.world_writable,
            "group_writable_nonowner": c.group_writable_nonowner,
        }));
    }

    // --- Severity for the Ambient finding (correction #5: own-dotfile baseline
    // stays Notable; genuine escalation is Exposed). ---
    let escalating = escalating_path_dirs > 0 || any_world_writable || any_group_writable_nonowner;
    let (severity, title) = if escalating {
        (
            Severity::Exposed,
            "writable escalation path (PATH shadowing or non-owner-writable target)",
        )
    } else if writable_targets > 0 || creatable_targets > 0 {
        (
            Severity::Notable,
            "writable persistence surface (own shell rc / dotfiles)",
        )
    } else {
        (
            Severity::Info,
            "no writable persistence/escalation paths found",
        )
    };

    let summary = if escalating {
        format!(
            "{escalating_path_dirs} escalating PATH dir(s); non-owner-writable target(s) present — command shadowing / shared-write surface"
        )
    } else if writable_targets > 0 || creatable_targets > 0 {
        format!(
            "{writable_targets} writable + {creatable_targets} creatable persistence target(s) (own-user baseline; what a sandbox would lock down)"
        )
    } else {
        "no writable or creatable persistence/escalation targets".to_string()
    };

    let ambient = Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        title,
        severity,
        Confidence::Confirmed,
    )
    .summary(summary)
    .evidence(json!({
        "home_owner_resolved": true,
        "shell_rc": shell_rc,
        "config_files": config_files,
        "path_dirs": {
            "total": path_total,
            "writable_count": writable_path_dirs.len(),
            "writable": writable_path_dirs,
            "first_system_bin_index": first_system_bin_index,
        },
        "counts": {
            "writable_targets": writable_targets,
            "creatable_targets": creatable_targets,
            "escalating_path_dirs": escalating_path_dirs,
        },
        "assumptions": "Writability inferred from host DAC mode bits + ownership (uid/gid vs home owner); reflects the agent-runs-as-user (sandbox NOT engaged) condition. ACLs, immutable (chattr +i) attrs, bwrap ro-bind/tmpfs overlays, and read-only mounts are NOT consulted.",
        "protection_note": "Under @anthropic-ai/sandbox-runtime, home dotfile protection derives from the narrow write allow-list (getDefaultWritePaths excludes home); the cwd-relative mandatory deny protects project-local .bashrc/.gitconfig/.git/hooks/config. This probe does not assert any file is hard-coded as a mandatory write-deny.",
    }))
    .remediation(&[
        "Run agents under a sandbox with a narrow write allow-list that excludes $HOME and rc/profile files.",
        "Ensure no writable PATH directory precedes the system bin dirs (prevents command shadowing).",
        "Audit any group-writable/world-writable target on the persistence surface.",
    ]);

    let mut findings = vec![ambient];

    // --- (d) git hooks / config — CurrentRepo-flavored, separate finding. ---
    if ctx.git.is_repo {
        // Resolve the common git dir (hooks live in the common dir for linked
        // worktrees, not the per-worktree gitdir).
        let common_dir = crate::util::command::run_stdout(
            "git",
            &["rev-parse", "--git-common-dir"],
            Some(&ctx.cwd),
        )
        .map(|d| {
            let p = std::path::PathBuf::from(&d);
            if p.is_absolute() {
                p
            } else {
                ctx.cwd.join(p)
            }
        })
        .or_else(|| ctx.git.git_dir.clone());

        if let Some(git_dir) = common_dir {
            // core.hooksPath override (read-only git config query).
            let hooks_override = crate::util::command::run_stdout(
                "git",
                &["config", "--get", "core.hooksPath"],
                Some(&ctx.cwd),
            )
            .filter(|s| !s.is_empty());
            let hooks_path_overridden = hooks_override.is_some();
            let hooks_dir = match &hooks_override {
                Some(h) => {
                    let p = std::path::PathBuf::from(h);
                    if p.is_absolute() {
                        p
                    } else {
                        ctx.cwd.join(p)
                    }
                }
                None => git_dir.join("hooks"),
            };
            let config_path = git_dir.join("config");

            let hooks_check = check(&hooks_dir);
            let config_check = check(&config_path);

            // Redirection of hooks to a writable location is the escalation
            // signal; non-owner-writable hooks/config also escalate.
            let git_escalating = (hooks_path_overridden && hooks_check.writable)
                || hooks_check.world_writable
                || hooks_check.group_writable_nonowner
                || config_check.world_writable
                || config_check.group_writable_nonowner;

            let (git_sev, git_title) = if git_escalating {
                (
                    Severity::Exposed,
                    "git hooks/config writable via redirection or non-owner write",
                )
            } else if hooks_check.writable || config_check.writable {
                (
                    Severity::Notable,
                    "git hooks/config writable (own-repo baseline)",
                )
            } else {
                (Severity::Info, "git hooks/config not writable")
            };

            findings.push(
                Finding::new(
                    "host.writable_git_hooks",
                    probe.class(),
                    FindingScope::CurrentRepo,
                    git_title,
                    git_sev,
                    Confidence::Confirmed,
                )
                .summary(if git_escalating {
                    "git hooks/config present on a writable/redirected path — code-exec persistence on next git op".to_string()
                } else if hooks_check.writable || config_check.writable {
                    "git hooks dir / config writable by owner (expected for your own repo)".to_string()
                } else {
                    "git hooks dir and config are not writable".to_string()
                })
                .evidence(json!({
                    "git_dir": shorten(&git_dir, Some(&home)),
                    "hooks_path": shorten(&hooks_dir, Some(&home)),
                    "hooks_writable": hooks_check.writable,
                    "config_writable": config_check.writable,
                    "hooks_path_overridden": hooks_path_overridden,
                    "hooks_world_writable": hooks_check.world_writable,
                    "config_world_writable": config_check.world_writable,
                    "hooks_group_writable_nonowner": hooks_check.group_writable_nonowner,
                    "config_group_writable_nonowner": config_check.group_writable_nonowner,
                }))
                .remediation(&[
                    "Treat .git/hooks and .git/config as code: deny writes to them in agent sandboxes.",
                    "Pin core.hooksPath or disable hooks for untrusted repos.",
                ]),
            );
        }
    }

    Ok(findings)
}

#[cfg(not(unix))]
fn platform_run(probe: &WriteReachProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        "writable persistence paths — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("writability inference is implemented for unix only")
    .evidence(
        json!({ "home_owner_resolved": false, "platform_supported": false }),
    )])
}
