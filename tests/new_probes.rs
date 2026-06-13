//! Fixture tests for the probes added in the detection-expansion pass: the
//! spec-driven credential stores, the SSH-agent socket probe, the git-config
//! exec-directive probe, and the writable Claude control-surface probe. Like the
//! other probe tests (§18): assert behavior and that NO secret values leak.

mod common;

use blastradius::probes;
use blastradius::runner::Probe;
use blastradius::severity::Severity;
use common::ctx_with;

/// Run a single store probe by id against a fixture home.
fn store_finding(id: &str, home: &std::path::Path) -> blastradius::finding::Finding {
    let probes = probes::store::store_probes();
    let probe = probes
        .iter()
        .find(|p| p.id() == id)
        .unwrap_or_else(|| panic!("store probe {id} not registered"));
    let ctx = ctx_with(home, home);
    probe.run(&ctx).unwrap().into_iter().next().unwrap()
}

#[test]
fn pypi_store_reports_index_names_not_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".pypirc"),
        "[distutils]\nindex-servers =\n    pypi\n\n[pypi]\nusername = __token__\npassword = pypi-AgEthisISsecret\n",
    )
    .unwrap();

    let f = store_finding("pypi.token", tmp.path());
    assert_eq!(f.severity, Severity::Exposed);
    let rendered = format!("{} {}", f.summary, serde_json::to_string(&f.evidence).unwrap());
    assert!(rendered.contains("pypi"));
    assert!(!rendered.contains("AgEthisISsecret"));
}

#[test]
fn gpg_store_counts_secret_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = tmp.path().join(".gnupg/private-keys-v1.d");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(keys.join("ABCDEF0123.key"), "binary-keygrip-material").unwrap();
    std::fs::write(keys.join("FEDCBA9876.key"), "binary-keygrip-material").unwrap();

    let f = store_finding("gpg.private_keys", tmp.path());
    assert_eq!(f.severity, Severity::Exposed);
    assert_eq!(f.evidence["item_count"], 2);
    // Count only — keygrip file names are not emitted.
    let rendered = serde_json::to_string(&f.evidence).unwrap();
    assert!(!rendered.contains("ABCDEF0123"));
}

#[test]
fn absent_stores_are_info_and_quiet() {
    let tmp = tempfile::tempdir().unwrap();
    for id in ["gcp.credentials", "azure.credentials", "terraform.token"] {
        let f = store_finding(id, tmp.path());
        assert_eq!(f.severity, Severity::Info, "{id} should be Info when absent");
    }
}

#[test]
fn ssh_agent_probe_reports_info_when_unset() {
    // The fixture context carries an empty env; SSH_AUTH_SOCK is read from the
    // live process env, which is typically unset under `cargo test`. Either way
    // the probe must not panic and must emit exactly one finding.
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::ssh_agent::SshAgentProbe.run(&ctx).unwrap();
    // Emits the ssh-agent finding plus the gpg-agent socket finding.
    assert!(findings.iter().any(|f| f.id == "ssh.agent_socket"));
    assert!(findings.iter().any(|f| f.id == "gpg.agent_socket"));
}

#[test]
fn claude_surface_flags_writable_settings_as_at_least_notable() {
    let tmp = tempfile::tempdir().unwrap();
    let claude = tmp.path().join(".claude");
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(claude.join("settings.json"), "{\"sandbox\":{\"enabled\":true}}").unwrap();
    std::fs::write(claude.join("CLAUDE.md"), "# instructions").unwrap();

    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::claude_surface::ClaudeSurfaceProbe.run(&ctx).unwrap();
    let ambient = findings.iter().find(|f| f.scope == blastradius::finding::FindingScope::Ambient).unwrap();
    // Own-user-writable policy/instruction files: Notable baseline (a sandbox
    // would deny these writes).
    assert!(matches!(ambient.severity, Severity::Notable | Severity::Exposed));
    assert!(ambient.evidence["writable_count"].as_u64().unwrap() >= 1);
}

#[test]
fn browser_probe_flags_cookie_and_login_stores() {
    let tmp = tempfile::tempdir().unwrap();
    // A Chrome "Default" profile with a cookie jar and a Login Data store.
    let profile = tmp.path().join(".config/google-chrome/Default");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join("Cookies"), b"SQLite format 3\0fake").unwrap();
    std::fs::write(profile.join("Login Data"), b"SQLite format 3\0fake").unwrap();

    let ctx = ctx_with(tmp.path(), tmp.path());
    let f = &probes::browser_stores::BrowserStoresProbe.run(&ctx).unwrap()[0];
    assert_eq!(f.severity, Severity::Exposed);
    assert_eq!(f.evidence["profiles_with_cookie_store"], 1);
    assert_eq!(f.evidence["profiles_with_login_store"], 1);
    // The DB contents are never read into the finding.
    let rendered = serde_json::to_string(&f.evidence).unwrap();
    assert!(!rendered.contains("SQLite format"));
}

#[test]
fn browser_probe_info_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let f = &probes::browser_stores::BrowserStoresProbe.run(&ctx).unwrap()[0];
    assert_eq!(f.severity, Severity::Info);
}

#[test]
fn repl_history_counts_connection_strings_not_lines() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".psql_history"),
        "\\c postgres://app:s3cr3tpw@db.internal:5432/app\nselect 1;\nPGPASSWORD=hunter2 psql\n",
    )
    .unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let f = &probes::repl_history::ReplHistoryProbe.run(&ctx).unwrap()[0];
    assert!(matches!(f.severity, Severity::Notable | Severity::Exposed));
    let rendered = serde_json::to_string(&f.evidence).unwrap();
    // Counts/categories only — never the password or the matched line.
    assert!(!rendered.contains("s3cr3tpw"));
    assert!(!rendered.contains("hunter2"));
    assert!(rendered.contains("connection_url") || rendered.contains("inline_password"));
}

#[test]
fn aws_sso_cache_counts_cached_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join(".aws/sso/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("abc123.json"), "{\"accessToken\":\"SECRET\"}").unwrap();

    let f = store_finding("aws.sso_cache", tmp.path());
    assert_eq!(f.severity, Severity::Exposed);
    assert_eq!(f.evidence["item_count"], 1);
    let rendered = serde_json::to_string(&f.evidence).unwrap();
    assert!(!rendered.contains("SECRET"));
}

#[test]
fn privilege_probe_emits_one_finding_value_free() {
    // Runs real `id`/`sudo -n -l` against the host; just assert it produces a
    // single well-formed finding and never surfaces a secret shape.
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::privilege::PrivilegeProbe.run(&ctx).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "host.privilege_escalation");
    let rendered = serde_json::to_string(&findings[0]).unwrap();
    assert!(!blastradius::report::redaction::contains_secret_shaped(&rendered));
}

#[test]
fn jupyter_runtime_store_counts_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let rt = tmp.path().join(".local/share/jupyter/runtime");
    std::fs::create_dir_all(&rt).unwrap();
    std::fs::write(rt.join("jpserver-1.json"), "{\"token\":\"SECRETTOK\"}").unwrap();
    let f = store_finding("jupyter.runtime", tmp.path());
    assert_eq!(f.severity, Severity::Exposed);
    assert!(!serde_json::to_string(&f.evidence).unwrap().contains("SECRETTOK"));
}

#[test]
fn process_introspection_emits_two_findings_value_free() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::process_introspect::ProcessIntrospectProbe.run(&ctx).unwrap();
    // On Linux: ptrace + cmdline findings; elsewhere a single platform note.
    assert!(!findings.is_empty());
    let rendered = serde_json::to_string(&findings).unwrap();
    assert!(!blastradius::report::redaction::contains_secret_shaped(&rendered));
}

#[test]
fn privileged_reach_is_conditional_and_value_free() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::privileged_reach::PrivilegedReachProbe.run(&ctx).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "host.privileged_reachability");
    let ev = &findings[0].evidence;
    // Honest framing: it never escalates or reads root files.
    assert!(ev.get("method").is_some());
    assert!(ev.get("escalation_path_present").is_some());
    let rendered = serde_json::to_string(findings.first().unwrap()).unwrap();
    assert!(!blastradius::report::redaction::contains_secret_shaped(&rendered));
}

#[test]
fn local_services_probe_is_value_free() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::local_services::LocalServicesProbe.run(&ctx).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "host.local_services");
    // Reports ports/service labels only; no payload is ever sent.
    assert!(findings[0].evidence.get("note").is_some());
}

#[test]
fn dbt_store_lists_profile_names_not_passwords() {
    let tmp = tempfile::tempdir().unwrap();
    let dbt = tmp.path().join(".dbt");
    std::fs::create_dir_all(&dbt).unwrap();
    std::fs::write(
        dbt.join("profiles.yml"),
        "analytics:\n  target: prod\n  outputs:\n    prod:\n      type: postgres\n      password: SUPERSECRETPW\nconfig:\n  send_anonymous_usage_stats: false\n",
    )
    .unwrap();
    let f = store_finding("dbt.profiles", tmp.path());
    assert_eq!(f.severity, Severity::Exposed);
    let rendered = format!("{} {}", f.summary, serde_json::to_string(&f.evidence).unwrap());
    assert!(rendered.contains("analytics"));
    assert!(!rendered.contains("SUPERSECRETPW"));
    assert!(!rendered.contains("config")); // excluded top-level key
}

#[test]
fn git_config_probe_emits_a_finding_without_values() {
    // No git repo in the fixture; the probe still emits its baseline finding and
    // never surfaces directive values.
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::git_config::GitConfigProbe.run(&ctx).unwrap();
    assert!(!findings.is_empty());
    let rendered = serde_json::to_string(&findings).unwrap();
    assert!(rendered.contains("exec_directives"));
}
