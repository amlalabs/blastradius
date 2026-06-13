//! §18 — redaction fixtures: known secret shapes must never survive a renderer.

mod common;

use std::sync::Mutex;

use blastradius::report::redaction::{contains_secret_shaped, sweep};

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

const FIXTURES: &[&str] = &[
    "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
    "github_pat_11ABCDEFG0abcdefghijklmnopqrstuvwxyz",
    "sk-test_ABCDEFGHIJKLMNOP",
    "AKIAIOSFODNN7EXAMPLE",
    "-----BEGIN RSA PRIVATE KEY-----",
    "https://user:hunter2@example.com/path",
    "HTTP_PROXY=user:hunter2@proxy.example:8080",
];

#[test]
fn all_fixtures_detected_and_swept() {
    for fx in FIXTURES {
        let blob = format!("noise before {fx} noise after");
        assert!(contains_secret_shaped(&blob), "not detected: {fx}");
        let cleaned = sweep(&blob);
        assert!(!contains_secret_shaped(&cleaned), "survived sweep: {fx}");
    }
}

#[test]
fn clean_metadata_is_untouched() {
    let clean = "GITHUB_TOKEN present in env — 40 chars; 2 profiles: default, prod";
    assert!(!contains_secret_shaped(clean));
    assert_eq!(sweep(clean), clean);
}

#[test]
fn self_test_redaction_passes() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    // The library canary self-test must succeed (§4.4).
    let code = blastradius::self_test_redaction().unwrap();
    assert_eq!(code, 0);
}

#[test]
fn self_test_redaction_restores_seeded_env_vars() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let original_canary = std::env::var_os("BLASTRADIUS_TEST_SECRET");
    let original_openai = std::env::var_os("OPENAI_API_KEY");

    std::env::set_var("BLASTRADIUS_TEST_SECRET", "original-canary");
    std::env::set_var("OPENAI_API_KEY", "original-openai");

    let code = blastradius::self_test_redaction().unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        std::env::var("BLASTRADIUS_TEST_SECRET").unwrap(),
        "original-canary"
    );
    assert_eq!(std::env::var("OPENAI_API_KEY").unwrap(), "original-openai");

    match original_canary {
        Some(value) => std::env::set_var("BLASTRADIUS_TEST_SECRET", value),
        None => std::env::remove_var("BLASTRADIUS_TEST_SECRET"),
    }
    match original_openai {
        Some(value) => std::env::set_var("OPENAI_API_KEY", value),
        None => std::env::remove_var("OPENAI_API_KEY"),
    }
}
