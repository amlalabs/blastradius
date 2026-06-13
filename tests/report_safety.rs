//! Renderer safety: local names/paths can be adversarial. Reports should not
//! emit terminal controls, bidi controls, or Markdown table-breaking text.

mod common;

use blastradius::finding::{Finding, FindingClass, FindingScope};
use blastradius::report::{self, ContextReport, RunReport};
use blastradius::severity::{Confidence, Severity};
use serde_json::json;

fn has_bad_controls(s: &str) -> bool {
    s.chars().any(|c| {
        (c.is_control() && c != '\n')
            || matches!(
                c,
                '\u{061C}'
                    | '\u{200E}'
                    | '\u{200F}'
                    | '\u{202A}'..='\u{202E}'
                    | '\u{2066}'..='\u{2069}'
            )
    })
}

fn malicious_report() -> RunReport {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = common::ctx_with(tmp.path(), tmp.path());
    let platform = ctx.platform;
    let finding = Finding::new(
        "test.renderer_safety",
        FindingClass::SystemInfo,
        FindingScope::Ambient,
        "bad\x1b[2J\nFORGED",
        Severity::Info,
        Confidence::Confirmed,
    )
    .summary("summary\rrewrite \u{202E} table|cell `code`")
    .evidence(json!({
        "path": "repo/\u{202E}evil",
        "note": "line1\nline2\x1b",
    }));

    RunReport {
        mode: "scan\x1b".to_string(),
        timestamp: "2026-06-08T12:00:00Z\r".to_string(),
        version: "0.1.0".to_string(),
        platform,
        command: "blastradius scan `oops`\n--flag".to_string(),
        contexts: vec![ContextReport {
            context: ctx,
            findings: vec![finding],
        }],
        comparison: None,
    }
}

#[test]
fn terminal_and_markdown_outputs_strip_controls() {
    let report = malicious_report();
    let terminal = report::terminal::render(&report);
    let markdown = report::markdown::render(&report);

    assert!(!has_bad_controls(&terminal), "terminal output has controls");
    assert!(!has_bad_controls(&markdown), "markdown output has controls");
    assert!(terminal.contains("bad?[2J?FORGED"));
    assert!(markdown.contains("table\\|cell"));
    assert!(!markdown.contains("`oops`"));
}

#[test]
fn json_output_sanitizes_nested_strings() {
    let rendered = report::json::render(&malicious_report());
    assert!(!has_bad_controls(&rendered), "json output has raw controls");

    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let finding = &parsed["findings"][0];
    assert_eq!(finding["title"], "bad?[2J?FORGED");
    assert_eq!(finding["evidence"]["path"], "repo/?evil");
    assert_eq!(finding["evidence"]["note"], "line1?line2?");
}

#[test]
fn reports_do_not_include_raw_command_arg_values() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("audit-secret-value");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args([
            "scan",
            "--max-depth",
            "0",
            "--max-repos",
            "0",
            "--output",
        ])
        .arg(&out)
        .current_dir(tmp.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();

    assert!(status.success());
    let markdown = std::fs::read_to_string(out.join("blastradius-report.md")).unwrap();
    let json = std::fs::read_to_string(out.join("blastradius-report.json")).unwrap();

    assert!(!markdown.contains("audit-secret-value"));
    assert!(!json.contains("audit-secret-value"));
    assert!(markdown.contains("--output [value]"));
}
