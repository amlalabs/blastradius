//! Terminal renderer (§7, §13.1, §14). Deterministic ordering; 80–100 cols.
//! Output is swept (Layer 2) before return.

use std::fmt::Write as _;

use crate::compare::diff::Comparison;
use crate::report::redaction::sweep;
use crate::report::sanitize::inline;
use crate::report::{group_by_class, ContextReport, RunReport, CONTAINMENT, TRUST_BANNER};
use crate::severity::Severity;
use crate::util::paths::shorten;

fn sev_tag(sev: Severity) -> &'static str {
    match sev {
        Severity::Exposed => "[exposed]",
        Severity::Notable => "[notable]",
        Severity::Info => "[ info  ]",
    }
}

fn render_findings(out: &mut String, cr: &ContextReport) {
    for (class, findings) in group_by_class(&cr.findings) {
        let _ = writeln!(out, "\n{}", class.section_title());
        for f in findings {
            let _ = writeln!(out, "  {} {}", sev_tag(f.severity), inline(&f.title));
            if !f.summary.is_empty() {
                let _ = writeln!(out, "            {}", inline(&f.summary));
            }
            // Confidence is reported separately from severity (§7.4).
            if !matches!(f.confidence, crate::severity::Confidence::Confirmed) {
                let _ = writeln!(out, "            confidence: {}", f.confidence);
            }
        }
    }
}

fn render_comparison(out: &mut String, cmp: &Comparison) {
    let _ = writeln!(
        out,
        "\n══ worktree comparison ════════════════════════════════════════\n"
    );
    let _ = writeln!(
        out,
        "  AMBIENT BLAST RADIUS                  repo root      worktree"
    );
    let _ = writeln!(
        out,
        "  ───────────────────────────────────────────────────────────"
    );
    for row in cmp.ambient_rows() {
        let _ = writeln!(
            out,
            "  {:<36}  {:<13}  {}",
            inline(&row.metric),
            inline(&row.left.display()),
            inline(&row.right.display())
        );
    }

    let _ = writeln!(
        out,
        "\n  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
    );
    let verdict = if cmp.ambient_unchanged {
        "ambient blast radius UNCHANGED."
    } else {
        "ambient blast radius differs (see rows above)."
    };
    let _ = writeln!(out, "  ►  working directory changed.  {verdict}");
    let _ = writeln!(
        out,
        "     A git worktree is a directory-level convenience, not a"
    );
    let _ = writeln!(out, "     security boundary.");

    // CurrentRepo deltas are a quiet footnote (§13.1, Fix 4).
    let cr_rows: Vec<_> = cmp.current_repo_rows().collect();
    if cr_rows.iter().any(|r| !r.equal) {
        let _ = writeln!(out);
        for r in cr_rows.iter().filter(|r| !r.equal) {
            let _ = writeln!(
                out,
                "  (footnote) current-repo-local checkout differs as expected — {} {}→{}",
                inline(&r.metric),
                inline(&r.left.display()),
                inline(&r.right.display())
            );
        }
        let _ = writeln!(
            out,
            "  because the worktree is at HEAD. This is not part of the ambient comparison."
        );
    }
}

fn render_containment(out: &mut String) {
    let _ = writeln!(out, "\nWhat would contain this:");
    for (title, desc) in CONTAINMENT {
        let _ = writeln!(out, "  • {title} — {desc}");
    }
}

pub fn render(report: &RunReport) -> String {
    let mut out = String::new();
    out.push_str(TRUST_BANNER);

    let _ = writeln!(
        out,
        "\n══ blastradius {} ══ {} ══",
        inline(&report.mode),
        inline(&report.timestamp)
    );

    for cr in &report.contexts {
        if report.contexts.len() > 1 {
            let _ = writeln!(
                out,
                "\n── context: {} ({}) ──",
                inline(cr.context.label.as_str()),
                inline(&shorten(&cr.context.cwd, cr.context.home.as_deref()))
            );
        }
        render_findings(&mut out, cr);
    }

    if let Some(cmp) = &report.comparison {
        render_comparison(&mut out, cmp);
    }

    render_containment(&mut out);

    sweep(&out)
}
