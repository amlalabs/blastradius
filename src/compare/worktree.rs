//! §13 — temp worktree harness with a cleanup guard. Creates a detached worktree
//! at HEAD, removes it on drop, and on failure prints the exact manual command.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context as _, Result};

use crate::util::command::{run_status, run_stdout};

/// Global registry of live worktrees so a Ctrl-C handler can clean them up (§13).
static REGISTRY: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

/// Best-effort removal of every registered worktree — invoked from the signal
/// handler. Prints the manual command if a removal fails.
pub fn cleanup_all() {
    let entries = {
        let mut guard = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::take(&mut *guard)
    };
    for (main_root, path) in entries {
        force_remove(&main_root, &path);
    }
}

fn force_remove(main_root: &Path, path: &Path) {
    let ok = run_status(
        "git",
        &["worktree", "remove", "--force", &path.to_string_lossy()],
        Some(main_root),
    )
    .unwrap_or(false);
    if !ok && path.exists() {
        eprintln!(
            "\nTemporary worktree cleanup failed. Remove it manually with:\n  git worktree remove --force {}",
            path.display()
        );
    }
}

pub struct Worktree {
    main_root: PathBuf,
    path: PathBuf,
    keep_on_error: bool,
    removed: bool,
}

impl Worktree {
    /// Create a detached worktree at HEAD under `$TMPDIR/blastradius-worktree-*`.
    pub fn create(main_root: &Path, keep_on_error: bool) -> Result<Worktree> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let suffix = format!("{}-{}", std::process::id(), nanos);
        let path = std::env::temp_dir().join(format!("blastradius-worktree-{suffix}"));

        let ok = run_status(
            "git",
            &[
                "worktree",
                "add",
                "--detach",
                &path.to_string_lossy(),
                "HEAD",
            ],
            Some(main_root),
        )
        .unwrap_or(false);

        if !ok {
            bail!(
                "failed to create temporary worktree at {} (git worktree add)",
                path.display()
            );
        }

        REGISTRY
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((main_root.to_path_buf(), path.clone()));

        Ok(Worktree {
            main_root: main_root.to_path_buf(),
            path,
            keep_on_error,
            removed: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn deregister(&self) {
        if let Ok(mut g) = REGISTRY.lock() {
            g.retain(|(_, p)| p != &self.path);
        }
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if self.keep_on_error {
            eprintln!(
                "Keeping temporary worktree for inspection:\n  {}",
                self.path.display()
            );
            self.deregister();
            return;
        }
        force_remove(&self.main_root, &self.path);
        self.removed = true;
        self.deregister();
    }
}

/// Resolve the MAIN repo root from `cwd`, erroring cleanly if not a repo (§19,
/// exit code 3 is applied by the caller).
pub fn require_main_repo_root(cwd: &Path) -> Result<PathBuf> {
    let toplevel = run_stdout("git", &["rev-parse", "--show-toplevel"], Some(cwd))
        .context("not inside a git repository")?;
    if toplevel.is_empty() {
        bail!("not inside a git repository");
    }
    crate::util::git::main_repo_root(cwd).context("could not resolve main repository root")
}
