//! §18 — performance & limits: large generated trees stay bounded and fast.

mod common;

use std::fs;
use std::time::Instant;

use blastradius::probes;
use blastradius::runner::Probe;
use common::*;

#[test]
fn sibling_enumeration_respects_caps_and_ignores_node_modules() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // 100 repos, each with files and a noisy node_modules that must be skipped.
    for i in 0..100 {
        let repo = root.join(format!("repo{i:03}"));
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(repo.join(".env"), "K=v\n").unwrap();
        let nm = repo.join("node_modules/pkg");
        fs::create_dir_all(&nm).unwrap();
        for j in 0..20 {
            fs::write(nm.join(format!("f{j}.js")), "x").unwrap();
        }
    }

    let mut ctx = ctx_with(root, root);
    ctx.repo_root = None;
    ctx = with_roots(ctx, vec![root.to_path_buf()]);
    ctx.limits.max_sibling_repos = 50;

    let start = Instant::now();
    let findings = probes::sibling_repos::SiblingReposProbe.run(&ctx).unwrap();
    let elapsed = start.elapsed();

    let f = &findings[0];
    assert_eq!(f.evidence["count"], 50, "must honor max_sibling_repos cap");
    assert_eq!(f.evidence["truncated"], true);
    assert!(
        elapsed.as_secs() < 20,
        "enumeration should be fast, took {elapsed:?}"
    );
}

#[test]
fn lateral_secrets_counts_across_siblings() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for i in 0..10 {
        let repo = root.join(format!("repo{i}"));
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(repo.join(".env"), "A=1\nB=2\n").unwrap();
        fs::write(repo.join("server.pem"), "x").unwrap();
    }
    let mut ctx = ctx_with(root, root);
    ctx.repo_root = None;
    ctx = with_roots(ctx, vec![root.to_path_buf()]);

    let f = &probes::lateral_secrets::LateralSecretsProbe
        .run(&ctx)
        .unwrap()[0];
    assert_eq!(f.evidence["repos_with_secret_like_files"], 10);
    assert_eq!(f.evidence["key_like_files"], 10);
    assert_eq!(f.evidence["dotenv_keys"], 20);
}
