//! Tests for the sandbox-awareness probes added for the Claude Code audit:
//! deferred-execution sinks, sandbox self-detection, egress mediation.
//! Every assertion confirms NO secret value leaks.

mod common;

use blastradius::probes;
use blastradius::report::redaction::contains_secret_shaped;
use blastradius::runner::Probe;
use common::*;

fn ev(f: &blastradius::finding::Finding) -> String {
    serde_json::to_string(&f.evidence).unwrap()
}

#[test]
fn deferred_sinks_detects_repo_sinks_and_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("Makefile"), "all:\n\techo hi\n").unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"postinstall":"node x.js","test":"jest"}}"#,
    )
    .unwrap();

    let ctx = ctx_with(root, root);
    let findings = probes::deferred_exec_sinks::DeferredExecSinksProbe
        .run(&ctx)
        .unwrap();
    let repo = findings
        .iter()
        .find(|f| f.id == "host.deferred_exec_sinks")
        .expect("repo sink finding present");

    assert!(repo.evidence["present"].as_u64().unwrap() >= 2);
    let lifecycle = ev(repo);
    assert!(
        lifecycle.contains("postinstall"),
        "lifecycle script detected"
    );
    // "test" is not a lifecycle (auto-run) script and must not be listed there.
    let scripts = &repo.evidence["package_json_lifecycle_scripts"];
    assert!(scripts.as_array().unwrap().iter().all(|s| s != "test"));
    for f in &findings {
        assert!(!contains_secret_shaped(&ev(f)));
    }
}

#[test]
fn sandbox_detect_emits_verdict_and_namespaces() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::sandbox_detect::SandboxDetectProbe
        .run(&ctx)
        .unwrap();
    let f = &findings[0];
    assert_eq!(f.id, "process.sandbox_detect");
    // On any platform the finding must carry a verdict and not leak.
    let e = ev(f);
    assert!(e.contains("verdict") || e.contains("platform_supported"));
    assert!(!contains_secret_shaped(&e));
}

#[test]
fn egress_mediation_always_checks_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_with(tmp.path(), tmp.path());
    let findings = probes::egress_mediation::EgressMediationProbe
        .run(&ctx)
        .unwrap();
    let f = &findings[0];
    assert_eq!(f.id, "egress.mediation");
    // Metadata reachability is always probed now — there is no opt-in flag.
    assert_eq!(f.evidence["metadata"]["checked"], true);
    assert!(!contains_secret_shaped(&ev(f)));
}
