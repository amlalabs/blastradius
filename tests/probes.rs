//! Probe fixture tests (§18): assert counts/names are reported and NO values leak.

mod common;

use std::fs;

use blastradius::probes;
use blastradius::report::redaction::contains_secret_shaped;
use blastradius::runner::Probe;
use common::*;

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn evidence_string(f: &blastradius::finding::Finding) -> String {
    serde_json::to_string(&f.evidence).unwrap()
}

#[test]
fn aws_profiles_counted_no_values() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".aws/credentials"),
        "[default]\naws_access_key_id = AKIATESTSECRETKEY01\naws_secret_access_key = abcd\n[prod]\naws_access_key_id = AKIAPRODSECRETKEY02\n",
    );
    let ctx = ctx_with(home, home);
    let findings = probes::aws::AwsProbe.run(&ctx).unwrap();
    let f = &findings[0];
    let ev = evidence_string(f);
    assert_eq!(f.evidence["profile_count"], 2);
    assert!(ev.contains("default") && ev.contains("prod"));
    // No secret value or key id should appear anywhere.
    assert!(!ev.contains("AKIATESTSECRETKEY01"));
    assert!(!ev.contains("abcd"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn ssh_private_keys_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".ssh/id_ed25519"),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk=\n-----END OPENSSH PRIVATE KEY-----\n",
    );
    write(&home.join(".ssh/id_ed25519.pub"), "ssh-ed25519 AAAA test\n");
    write(&home.join(".ssh/config"), "Host github.com\n  User git\n");
    let ctx = ctx_with(home, home);
    let findings = probes::ssh::SshProbe.run(&ctx).unwrap();
    let f = &findings[0];
    assert_eq!(f.evidence["key_count"], 1, "should not count the .pub file");
    let ev = evidence_string(f);
    assert!(ev.contains("github.com"));
}

#[test]
fn dotenv_keys_counted_not_valued() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    write(
        &repo.join(".env"),
        "API_KEY=supersecretvalue\nDB_PASS=p@ss\n# comment\n",
    );
    write(&repo.join(".env.example"), "API_KEY=\n"); // excluded
    let ctx = ctx_with(repo, repo);
    let findings = probes::dotenv::DotenvProbe.run(&ctx).unwrap();
    let current = findings
        .iter()
        .find(|f| f.id.ends_with(".current"))
        .unwrap();
    assert_eq!(current.evidence["file_count"], 1, "example excluded");
    assert_eq!(current.evidence["key_count"], 2);
    let ev = evidence_string(current);
    assert!(!ev.contains("supersecretvalue"));
    assert!(!ev.contains("p@ss"));
}

#[test]
fn git_credentials_host_only() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".git-credentials"),
        "https://octocat:ghp_TOPSECRETTOKENVALUE0001@github.com\n",
    );
    let ctx = ctx_with(home, home);
    let findings = probes::git_credentials::GitCredentialsProbe
        .run(&ctx)
        .unwrap();
    let f = &findings[0];
    let ev = evidence_string(f);
    assert!(ev.contains("github.com"));
    assert!(!ev.contains("ghp_TOPSECRETTOKENVALUE0001"));
    assert!(!ev.contains("octocat:"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn shell_history_counts_not_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".zsh_history"),
        "ls\nexport GITHUB_TOKEN=ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\ncurl https://u:p@host/x\npwd\n",
    );
    let ctx = ctx_with(home, home);
    let findings = probes::shell_history::ShellHistoryProbe.run(&ctx).unwrap();
    let f = &findings[0];
    let ev = evidence_string(f);
    assert!(f.evidence["total_matches"].as_u64().unwrap() >= 2);
    assert!(!ev.contains("ghp_AAAA"));
    assert!(!ev.contains("u:p@host"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn env_curated_drives_exposed() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = with_env(ctx_with(tmp.path(), tmp.path()), "GITHUB_TOKEN", 40);
    let findings = probes::env::EnvProbe.run(&ctx).unwrap();
    let f = &findings[0];
    assert_eq!(f.severity, blastradius::severity::Severity::Exposed);
    assert_eq!(f.evidence["via"], "curated");
    // Suppressed keys must not appear.
    let ctx2 = with_env(ctx_with(tmp.path(), tmp.path()), "KEYMAP", 4);
    let f2 = &probes::env::EnvProbe.run(&ctx2).unwrap()[0];
    assert_eq!(f2.severity, blastradius::severity::Severity::Info);
}

#[test]
fn sibling_enumeration_anchored_to_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // Two sibling repos + the "current" repo (excluded).
    for r in ["api", "web", "app"] {
        fs::create_dir_all(root.join(r).join(".git")).unwrap();
    }
    let mut ctx = ctx_with(root, &root.join("app"));
    ctx.repo_root = Some(root.join("app"));
    ctx = with_roots(ctx, vec![root.to_path_buf()]);
    let findings = probes::sibling_repos::SiblingReposProbe.run(&ctx).unwrap();
    let f = &findings[0];
    assert_eq!(f.evidence["count"], 2, "current repo excluded");
}

#[test]
fn oversized_credential_configs_are_not_parsed_or_leaked() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let oversized = format!(
        "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345\n{}",
        "x".repeat(4 * 1024 * 1024)
    );

    write(&home.join(".aws/credentials"), &oversized);
    let aws = &probes::aws::AwsProbe.run(&ctx_with(home, home)).unwrap()[0];
    assert_eq!(aws.evidence["profile_count"], 0);
    assert_eq!(aws.evidence["skipped_files"].as_array().unwrap().len(), 1);
    let aws_ev = evidence_string(aws);
    assert!(!aws_ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&aws_ev));

    write(&home.join(".config/gh/hosts.yml"), &oversized);
    let gh = &probes::github::GithubProbe
        .run(&ctx_with(home, home))
        .unwrap()[0];
    assert_eq!(gh.evidence["hosts"].as_array().unwrap().len(), 0);
    assert_eq!(gh.evidence["skipped_files"].as_array().unwrap().len(), 1);
    let gh_ev = evidence_string(gh);
    assert!(!gh_ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&gh_ev));

    write(&home.join(".git-credentials"), &oversized);
    let git = &probes::git_credentials::GitCredentialsProbe
        .run(&ctx_with(home, home))
        .unwrap()[0];
    assert_eq!(git.evidence["stored_hosts"].as_array().unwrap().len(), 0);
    assert_eq!(git.evidence["skipped_files"].as_array().unwrap().len(), 1);
    let git_ev = evidence_string(git);
    assert!(!git_ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&git_ev));

    write(&home.join(".ssh/config"), &oversized);
    let ssh = &probes::ssh::SshProbe.run(&ctx_with(home, home)).unwrap()[0];
    assert_eq!(ssh.evidence["config_skipped"], true);
    let ssh_ev = evidence_string(ssh);
    assert!(!ssh_ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&ssh_ev));
}
