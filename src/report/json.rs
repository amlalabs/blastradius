//! JSON renderer (§14) — stable schema 1.0 so the future matrix mode needs no
//! core rewrite. Swept before return.

use serde_json::json;

use crate::report::redaction::sweep;
use crate::report::sanitize;
use crate::report::RunReport;
use crate::util::paths::shorten;

pub fn render(report: &RunReport) -> String {
    let contexts: Vec<serde_json::Value> = report
        .contexts
        .iter()
        .map(|cr| {
            let ctx = &cr.context;
            json!({
                "label": ctx.label.as_str(),
                "cwd": shorten(&ctx.cwd, ctx.home.as_deref()),
                "repo_root": ctx.repo_root.as_ref().map(|r| shorten(r, ctx.home.as_deref())),
                "git": {
                    "is_repo": ctx.git.is_repo,
                    "branch": ctx.git.current_branch,
                    "head_sha_short": ctx.git.head_sha_short,
                },
            })
        })
        .collect();

    let findings: Vec<serde_json::Value> = report
        .contexts
        .iter()
        .flat_map(|cr| {
            let label = cr.context.label.as_str().to_string();
            cr.findings.iter().map(move |f| {
                json!({
                    "context": label,
                    "id": f.id,
                    "class": f.class.to_string(),
                    "scope": f.scope.to_string(),
                    "title": f.title,
                    "summary": f.summary,
                    "severity": f.severity,
                    "confidence": f.confidence,
                    "evidence": f.evidence,
                    "remediation": f.remediation,
                })
            })
        })
        .collect();

    let comparison = match &report.comparison {
        Some(cmp) => json!({
            "ambient_unchanged": cmp.ambient_unchanged,
            "rows": cmp.rows,
        }),
        None => json!({ "ambient_unchanged": null, "rows": [] }),
    };

    let mut doc = json!({
        "schema_version": "1.0",
        "tool": { "name": "blastradius", "version": report.version },
        "run": {
            "id": null,
            "timestamp": report.timestamp,
            "mode": report.mode,
            "offline": report.offline,
            "egress_enabled": report.egress_enabled,
        },
        "contexts": contexts,
        "findings": findings,
        "comparison": comparison,
    });

    sanitize::json_value(&mut doc);
    let text = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string());
    sweep(&text)
}
