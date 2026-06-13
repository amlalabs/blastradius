//! Markdown renderer (§14). No values; swept before return.

use std::fmt::Write as _;

use crate::report::redaction::sweep;
use crate::report::sanitize::{markdown_code_span, markdown_text};
use crate::report::{group_by_class, RunReport, CONTAINMENT};

pub fn render(report: &RunReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# blastradius report\n");
    let _ = writeln!(
        out,
        "- **tool:** blastradius {}",
        markdown_text(&report.version)
    );
    let _ = writeln!(out, "- **timestamp:** {}", markdown_text(&report.timestamp));
    let _ = writeln!(out, "- **platform:** {:?}", report.platform);
    let _ = writeln!(out, "- **mode:** {}", markdown_text(&report.mode));
    let _ = writeln!(
        out,
        "- **command:** {}",
        markdown_code_span(&report.command)
    );

    let _ = writeln!(
        out,
        "\n> Privacy: no telemetry, no findings leave this machine, secret values are never printed. \
         This tool proves *reachability*, not intent.\n"
    );

    for cr in &report.contexts {
        if report.contexts.len() > 1 {
            let _ = writeln!(
                out,
                "\n## Context: {}\n",
                markdown_text(cr.context.label.as_str())
            );
        }
        for (class, findings) in group_by_class(&cr.findings) {
            let _ = writeln!(out, "\n### {}\n", class.section_title());
            for f in findings {
                let _ = writeln!(
                    out,
                    "- **{}** ({} / {}) — {}",
                    markdown_text(&f.title),
                    f.severity,
                    f.confidence,
                    markdown_text(&f.summary)
                );
            }
        }
    }

    if let Some(cmp) = &report.comparison {
        let _ = writeln!(out, "\n## Worktree comparison\n");
        let _ = writeln!(
            out,
            "Ambient blast radius is **{}** across the worktree.\n",
            if cmp.ambient_unchanged {
                "unchanged"
            } else {
                "changed"
            }
        );
        let _ = writeln!(out, "| metric | scope | repo root | worktree |");
        let _ = writeln!(out, "|---|---|---|---|");
        for r in &cmp.rows {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                markdown_text(&r.metric),
                if r.ambient { "ambient" } else { "current-repo" },
                markdown_text(&r.left.display()),
                markdown_text(&r.right.display())
            );
        }
        let _ = writeln!(
            out,
            "\n> Note: current-repo-local files differ because the temporary worktree is checked \
             out at HEAD. This does not affect the ambient-authority comparison."
        );
    }

    let _ = writeln!(out, "\n## What would contain this\n");
    for (title, desc) in CONTAINMENT {
        let _ = writeln!(out, "- **{title}** — {desc}");
    }

    let _ = writeln!(
        out,
        "\n## Limitations\n\nReachability is not validity. A reachable credential may be expired, \
         scoped, or rejected server-side. This scan does not verify push acceptance, branch \
         protection, or token scopes."
    );

    sweep(&out)
}
