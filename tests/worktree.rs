//! §18 — worktree compare: ambient findings equal across contexts; the dirty
//! current-repo .env differs and the worktree is cleaned up afterwards.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use blastradius::compare::{diff, worktree};
use blastradius::context::{Context, ContextLabel};
use blastradius::runner::{default_probes, run_all};

fn git(args: &[&str], cwd: &Path) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(ok, "git {args:?} failed");
}

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn compare_ambient_equal_currentrepo_differs_and_cleans_up() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("app");
    fs::create_dir_all(&repo).unwrap();
    git(&["init", "-q"], &repo);
    git(&["config", "user.email", "t@t"], &repo);
    git(&["config", "user.name", "t"], &repo);
    fs::write(repo.join("README.md"), "hi").unwrap();
    git(&["add", "-A"], &repo);
    git(&["commit", "-qm", "init"], &repo);
    // Untracked local .env — present in root, absent in HEAD worktree.
    fs::write(repo.join(".env"), "LOCAL_TOKEN=abc\n").unwrap();

    // Root context, with discovery disabled for determinism/speed.
    let mut root_ctx = Context::build(
        ContextLabel::RepoRoot,
        repo.clone(),
        Default::default(),
        blastradius::context::NetworkPolicy {
            egress_enabled: false,
            offline: true,
            ..Default::default()
        },
    );
    root_ctx.discovery_roots = Vec::new();
    let root_findings = run_all(&root_ctx, &default_probes());

    let wt = worktree::Worktree::create(&repo, false).unwrap();
    let wt_path = wt.path().to_path_buf();
    assert!(wt_path.exists(), "worktree should be created");

    let mut wt_ctx = root_ctx.clone();
    wt_ctx.label = ContextLabel::Worktree;
    wt_ctx.cwd = wt_path.clone();
    wt_ctx.checkout_root = Some(wt_path.clone());
    let wt_findings = run_all(&wt_ctx, &default_probes());

    let cmp = diff::compare(&root_findings, &wt_findings);
    assert!(
        cmp.ambient_unchanged,
        "ambient blast radius must be identical across the worktree"
    );

    // The current-repo .env row must differ (1 -> 0).
    let cr = cmp
        .rows
        .iter()
        .find(|r| r.metric == "current-repo .env files")
        .expect("current-repo .env row present");
    assert!(
        !cr.equal,
        "untracked .env should differ in the HEAD worktree"
    );

    drop(wt);
    assert!(!wt_path.exists(), "worktree should be removed on drop");
}
