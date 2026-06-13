//! Read-only git context discovery (§9.2, §12.8, §12.10). Never pushes; no dry-run.

use std::path::{Path, PathBuf};

use crate::context::{GitContext, GitRemote};
use crate::util::command::run_stdout;
use crate::util::parse::{git_remote_host_protocol, redact_url_userinfo};

/// Discover the git context for `cwd`. `repo_root` is anchored to the MAIN repo
/// (resolved via `git-common-dir`, §12.8), not a linked worktree's toplevel.
pub fn discover(cwd: &Path) -> GitContext {
    let toplevel = run_stdout("git", &["rev-parse", "--show-toplevel"], Some(cwd));
    let toplevel = match toplevel {
        Some(t) if !t.is_empty() => PathBuf::from(t),
        _ => return GitContext::default(),
    };

    let git_dir = run_stdout("git", &["rev-parse", "--git-dir"], Some(cwd))
        .map(|d| resolve_relative(cwd, &d));

    // Anchor to the MAIN repo root, even when run inside a linked worktree.
    let main_root = main_repo_root(cwd).unwrap_or_else(|| toplevel.clone());

    let current_branch =
        run_stdout("git", &["branch", "--show-current"], Some(cwd)).filter(|s| !s.is_empty());

    let head_sha_short =
        run_stdout("git", &["rev-parse", "--short", "HEAD"], Some(cwd)).filter(|s| !s.is_empty());

    let default_branch_guess = guess_default_branch(cwd);

    let remotes = read_remotes(cwd);

    GitContext {
        is_repo: true,
        repo_root: Some(main_root),
        worktree_toplevel: Some(toplevel),
        git_dir,
        current_branch,
        head_sha_short,
        default_branch_guess,
        remotes,
    }
}

/// Resolve the MAIN repo toplevel even from within a linked worktree (§12.8).
///
/// `git-common-dir` points at the main repo's `.git` (the worktree's points at
/// `<main>/.git/worktrees/<name>`). Its parent is the main repo root.
pub fn main_repo_root(cwd: &Path) -> Option<PathBuf> {
    let common = run_stdout("git", &["rev-parse", "--git-common-dir"], Some(cwd))?;
    let common = resolve_relative(cwd, &common);
    // common is typically `<main>/.git`; its parent is the main worktree root.
    let canon = common.canonicalize().unwrap_or(common);
    canon.parent().map(|p| p.to_path_buf())
}

fn resolve_relative(cwd: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        cwd.join(pb)
    }
}

fn guess_default_branch(cwd: &Path) -> Option<String> {
    // Prefer origin/HEAD if configured.
    if let Some(sym) = run_stdout(
        "git",
        &["symbolic-ref", "refs/remotes/origin/HEAD"],
        Some(cwd),
    ) {
        if let Some(name) = sym.rsplit('/').next() {
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    // Fall back to whichever common default branch exists locally.
    for cand in ["main", "master"] {
        if run_stdout(
            "git",
            &["rev-parse", "--verify", &format!("refs/heads/{cand}")],
            Some(cwd),
        )
        .is_some()
        {
            return Some(cand.to_string());
        }
    }
    None
}

fn read_remotes(cwd: &Path) -> Vec<GitRemote> {
    let raw = match run_stdout("git", &["remote", "-v"], Some(cwd)) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut out: Vec<GitRemote> = Vec::new();
    for line in raw.lines() {
        // Format: `origin\tgit@github.com:o/r.git (fetch)`
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let url = match parts.next() {
            Some(u) => u.to_string(),
            None => continue,
        };
        let direction = parts.next().unwrap_or("");
        let remote = remote_from_parts(name, &url);
        if let Some(existing) = out.iter_mut().find(|r| r.name == remote.name) {
            // `git remote -v` prints fetch before push. Push likelihood should
            // be inferred from the push URL when a remote has a separate one.
            if direction == "(push)" {
                *existing = remote;
            }
            continue;
        }
        out.push(remote);
    }
    out
}

fn remote_from_parts(name: String, url: &str) -> GitRemote {
    let (host, protocol) = git_remote_host_protocol(url);
    GitRemote {
        name,
        raw_url_redacted: redact_url_userinfo(url),
        host,
        protocol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn git(args: &[&str], cwd: &Path) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    }

    #[test]
    fn read_remotes_prefers_push_url_for_likelihood() {
        if !git_available() {
            eprintln!("skipping: git not available");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        git(&["init", "-q"], tmp.path());
        git(
            &["remote", "add", "origin", "https://github.com/org/repo.git"],
            tmp.path(),
        );
        git(
            &[
                "remote",
                "set-url",
                "--push",
                "origin",
                "git@github.com:org/repo.git",
            ],
            tmp.path(),
        );

        let remotes = read_remotes(tmp.path());
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].host.as_deref(), Some("github.com"));
        assert_eq!(remotes[0].protocol.as_deref(), Some("ssh"));
        assert_eq!(remotes[0].raw_url_redacted, "git@github.com:org/repo.git");
    }
}
