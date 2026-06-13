//! Integration tests for the vetted probes (write-reach, sandbox-reach,
//! env-scrub, tool-surface posture). Mirrors tests/probes.rs: assert the probe
//! reports the expected counts/names AND that NO secret value leaks.

mod common;

use std::fs;

use blastradius::probes;
use blastradius::report::redaction::contains_secret_shaped;
use blastradius::runner::Probe;
use blastradius::severity::Severity;
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

// --------------------------------------------------------------------------
// write_reach
// --------------------------------------------------------------------------

#[test]
fn write_reach_reports_own_dotfiles_as_notable_baseline() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    // A writable shell rc and a writable dotfile (owned by us, the home owner).
    write(&home.join(".bashrc"), "export PATH=$PATH\n");
    write(&home.join(".gitconfig"), "[user]\n  name = test\n");

    let ctx = ctx_with(home, home);
    let findings = probes::write_reach::WriteReachProbe.run(&ctx).unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "host.writable_persistence_paths")
        .unwrap();

    // home owner identity resolved; baseline writable dotfiles -> Notable.
    assert_eq!(ambient.evidence["home_owner_resolved"], true);
    assert!(
        ambient.evidence["counts"]["writable_targets"]
            .as_u64()
            .unwrap()
            >= 2
    );
    // Own-dotfile baseline must not escalate beyond Notable unless escalation.
    assert!(matches!(
        ambient.severity,
        Severity::Notable | Severity::Exposed
    ));

    let ev = evidence_string(ambient);
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn write_reach_home_unknown_is_info() {
    let tmp = tempfile::tempdir().unwrap();
    let mut ctx = ctx_with(tmp.path(), tmp.path());
    ctx.home = None;
    let findings = probes::write_reach::WriteReachProbe.run(&ctx).unwrap();
    let f = &findings[0];
    assert_eq!(f.severity, Severity::Info);
    assert_eq!(f.evidence["home_owner_resolved"], false);
}

#[test]
fn write_reach_emits_no_secret_values() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    // Even if a dotfile contains a secret-shaped value, we never read contents.
    write(
        &home.join(".bashrc"),
        "export GITHUB_TOKEN=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345\n",
    );
    let ctx = ctx_with(home, home);
    let findings = probes::write_reach::WriteReachProbe.run(&ctx).unwrap();
    for f in &findings {
        let ev = evidence_string(f);
        assert!(!ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
        assert!(!contains_secret_shaped(&ev));
    }
}

// --------------------------------------------------------------------------
// sandbox_reach
// --------------------------------------------------------------------------

#[test]
fn sandbox_reach_emits_two_findings_no_leak() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::sandbox_reach::SandboxReachProbe.run(&ctx).unwrap();
    assert_eq!(findings.len(), 2);

    let a = findings
        .iter()
        .find(|f| f.id == "process.afunix_docker_sock")
        .unwrap();
    let b = findings
        .iter()
        .find(|f| f.id == "process.proc_environ")
        .unwrap();

    // Finding A reports a creatability boolean.
    assert!(a.evidence.get("af_unix_socket_creatable").is_some());
    // Finding B reports counts (or platform_supported:false off-Linux).
    assert!(b.evidence.get("platform_supported").is_some());

    for f in &findings {
        let ev = evidence_string(f);
        assert!(!contains_secret_shaped(&ev), "no secret in {}", f.id);
        // We must never enumerate environ contents.
        assert!(!ev.contains("environ_contents"));
    }
}

#[test]
fn sandbox_reach_proc_environ_not_exposed_when_isolated() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::sandbox_reach::SandboxReachProbe.run(&ctx).unwrap();
    let b = findings
        .iter()
        .find(|f| f.id == "process.proc_environ")
        .unwrap();
    // If a fresh/isolated namespace is detected, Exposed must be suppressed.
    if b.evidence["pid_namespace_isolated_hint"] == serde_json::json!(true) {
        assert_ne!(b.severity, Severity::Exposed);
    }
}

// --------------------------------------------------------------------------
// env_scrub
// --------------------------------------------------------------------------

#[test]
fn env_scrub_off_with_exempt_cred_is_notable() {
    let tmp = tempfile::tempdir().unwrap();
    // Ensure the flag is off for this process.
    std::env::remove_var("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB");
    let ctx = with_env(ctx_with(tmp.path(), tmp.path()), "GITHUB_TOKEN", 40);
    let f = &probes::env_scrub::EnvScrubProbe.run(&ctx).unwrap()[0];
    assert_eq!(f.severity, Severity::Notable);
    assert_eq!(f.evidence["scrub_active"], false);
    assert_eq!(f.evidence["exempt_count"], 1);
    let ev = evidence_string(f);
    assert!(ev.contains("GITHUB_TOKEN"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn env_scrub_off_with_no_creds_is_info() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::remove_var("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB");
    let ctx = ctx_with(tmp.path(), tmp.path());
    let f = &probes::env_scrub::EnvScrubProbe.run(&ctx).unwrap()[0];
    assert_eq!(f.severity, Severity::Info);
}

#[test]
fn env_scrub_classifies_openai_as_exempt() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::remove_var("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB");
    let ctx = with_env(ctx_with(tmp.path(), tmp.path()), "OPENAI_API_KEY", 51);
    let f = &probes::env_scrub::EnvScrubProbe.run(&ctx).unwrap()[0];
    let ev = evidence_string(f);
    // OPENAI_API_KEY is exempt (correction #2), not covered.
    assert!(f.evidence["scrub_exempt_present"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "OPENAI_API_KEY"));
    assert_eq!(f.evidence["covered_count"], 0);
    assert!(!contains_secret_shaped(&ev));
}

// --------------------------------------------------------------------------
// sandbox_posture
// --------------------------------------------------------------------------

#[test]
fn posture_no_config_is_info() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let ctx = ctx_with(home, home);
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "claude_code.sandbox_posture")
        .unwrap();
    assert_eq!(ambient.severity, Severity::Info);
    assert_eq!(
        ambient.evidence["scopes_found"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn posture_sandbox_off_is_notable() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".claude/settings.json"),
        r#"{ "skipDangerousModePermissionPrompt": true }"#,
    );
    let ctx = ctx_with(home, home);
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "claude_code.sandbox_posture")
        .unwrap();
    // No sandbox key + escape hatch -> Notable (not Exposed; capped per review).
    assert_eq!(ambient.severity, Severity::Notable);
    assert_ne!(ambient.severity, Severity::Exposed);
    assert!(ambient
        .evidence
        .pointer("/scopes_found")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "user"));
}

#[test]
fn posture_contained_sandbox_is_info() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".claude/settings.json"),
        r#"{
            "sandbox": {
                "enabled": true,
                "allowUnsandboxedCommands": false,
                "filesystem": { "denyRead": ["~/.aws", "~/.ssh"] }
            }
        }"#,
    );
    let ctx = ctx_with(home, home);
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "claude_code.sandbox_posture")
        .unwrap();
    assert_eq!(ambient.severity, Severity::Info);
    assert_eq!(ambient.evidence["sandbox"]["enabled"], true);
    assert_eq!(
        ambient.evidence["sandbox"]["fs"]["deny_read_covers_home_or_creds"],
        true
    );
}

#[test]
fn posture_project_scope_is_currentrepo_and_value_free() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&repo).unwrap();
    // A project-scoped MCP server with a (fake) secret-shaped token in env.
    write(
        &repo.join(".mcp.json"),
        r#"{ "mcpServers": { "evilserver": { "command": "node", "env": { "TOKEN": "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345" } } } }"#,
    );
    let mut ctx = ctx_with(&home, &repo);
    ctx.checkout_root = Some(repo.clone());
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let proj = findings
        .iter()
        .find(|f| f.id == "claude_code.project_tool_surface")
        .unwrap();
    assert_eq!(proj.scope, blastradius::finding::FindingScope::CurrentRepo);
    let ev = evidence_string(proj);
    // Server NAME is reported; its env token VALUE must not leak.
    assert!(ev.contains("evilserver"));
    assert!(!ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn posture_degraded_parse_records_generic_reason_no_snippet() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    // Malformed JSON containing a secret-shaped string.
    write(
        &home.join(".claude/settings.json"),
        "{ not valid json ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
    );
    let ctx = ctx_with(home, home);
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "claude_code.sandbox_posture")
        .unwrap();
    let ev = evidence_string(ambient);
    // Degraded scope is recorded but never echoes the file content.
    assert!(ev.contains("json parse error"));
    assert!(!ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&ev));
}

#[test]
fn posture_oversized_settings_records_size_cap_no_snippet() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(
        &home.join(".claude/settings.json"),
        &format!(
            "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345\n{}",
            "x".repeat(4 * 1024 * 1024)
        ),
    );
    let ctx = ctx_with(home, home);
    let findings = probes::sandbox_posture::SandboxPostureProbe
        .run(&ctx)
        .unwrap();
    let ambient = findings
        .iter()
        .find(|f| f.id == "claude_code.sandbox_posture")
        .unwrap();
    let ev = evidence_string(ambient);
    assert!(ev.contains("file exceeds size cap; not parsed"));
    assert!(!ev.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(!contains_secret_shaped(&ev));
}
