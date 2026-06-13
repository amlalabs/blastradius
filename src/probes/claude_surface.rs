//! §extra — writable Claude Code control & instruction surface (doc §4 / §4.1).
//!
//! `sandbox_posture` READS Claude Code settings; `write_reach` checks generic
//! dotfiles. Neither asks the pointed question this probe answers: can code
//! running as the agent REWRITE the very files that are supposed to constrain or
//! instruct it? Two distinct risks:
//!   - **Self-weakening:** a writable `settings.json` lets an agent disable its
//!     own sandbox, add an `excludedCommands` entry, or register a hook.
//!   - **Prompt-injection persistence:** a writable `CLAUDE.md` / `AGENTS.md` (or
//!     a creatable skills/commands/agents entry) rides every future session.
//!
//! Like `write_reach`, writability is inferred from host DAC (mode + ownership vs
//! the home owner) — the surface a sandbox would lock down — and NOTHING is
//! written. Own-user-writable policy/instruction files are `Notable` (a sandbox
//! should deny the agent write to them); non-owner-writable is `Exposed`.

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct ClaudeSurfaceProbe;

impl Probe for ClaudeSurfaceProbe {
    fn id(&self) -> &'static str {
        "claude_code.writable_control_surface"
    }
    fn class(&self) -> FindingClass {
        FindingClass::HostPersistence
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        platform_run(self, ctx)
    }
}

/// What kind of trust a target carries — drives the summary framing.
#[derive(Clone, Copy)]
enum Kind {
    /// Policy file/dir that constrains the agent (settings, sandbox config).
    Control,
    /// Instruction/skill surface loaded into the agent (CLAUDE.md, skills).
    Instruction,
}

#[cfg(unix)]
fn platform_run(probe: &ClaudeSurfaceProbe, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    use crate::util::fsperm::{check, dir_writable_searchable, home_identity};
    use crate::util::paths::shorten;
    use std::path::Path;

    let home = match &ctx.home {
        Some(h) => h.clone(),
        None => return Ok(vec![info(probe, "home unknown; cannot resolve write identity")]),
    };
    let (uid, gid) = match home_identity(&home) {
        Some(x) => x,
        None => return Ok(vec![info(probe, "home unreadable; cannot resolve write identity")]),
    };

    // Assess one path; returns (json, writable_now, nonowner, plantable).
    let assess = |path: &Path, kind: Kind, is_dir: bool| -> (serde_json::Value, bool, bool, bool) {
        let c = check(path, uid, gid);
        // For a dir we also treat "writable+searchable" as plantable-into.
        let dir_plantable = is_dir && c.exists && dir_writable_searchable(path, uid, gid);
        let writable_now = c.writable || dir_plantable;
        // Absent control/instruction file that can be created in a writable dir.
        let plantable = !c.exists && c.creatable;
        let kind_str = match kind {
            Kind::Control => "control",
            Kind::Instruction => "instruction",
        };
        (
            json!({
                "path": shorten(path, Some(&home)),
                "kind": kind_str,
                "exists": c.exists,
                "is_dir": is_dir,
                "writable": writable_now,
                "creatable": plantable,
                "nonowner_writable": c.nonowner_writable,
            }),
            writable_now,
            c.nonowner_writable,
            plantable,
        )
    };

    // (relative path, kind, is_dir)
    let home_targets: &[(&str, Kind, bool)] = &[
        (".claude/settings.json", Kind::Control, false),
        (".claude/settings.local.json", Kind::Control, false),
        (".claude.json", Kind::Control, false),
        (".claude/CLAUDE.md", Kind::Instruction, false),
        (".claude/skills", Kind::Instruction, true),
        (".claude/commands", Kind::Instruction, true),
        (".claude/agents", Kind::Instruction, true),
    ];
    let repo_targets: &[(&str, Kind, bool)] = &[
        (".claude/settings.json", Kind::Control, false),
        (".claude/settings.local.json", Kind::Control, false),
        (".mcp.json", Kind::Control, false),
        ("CLAUDE.md", Kind::Instruction, false),
        ("AGENTS.md", Kind::Instruction, false),
        (".claude/commands", Kind::Instruction, true),
        (".claude/agents", Kind::Instruction, true),
        (".claude/skills", Kind::Instruction, true),
    ];

    let mut findings = Vec::new();

    // --- Ambient: home-level control + instruction surface. ---
    findings.push(build_finding(
        probe.id(),
        FindingScope::Ambient,
        "home",
        home_targets.iter().map(|(rel, k, d)| {
            let (j, w, n, p) = assess(&home.join(rel), *k, *d);
            (j, w, n, p)
        }),
    ));

    // --- CurrentRepo: project-level control + instruction surface. ---
    if let Some(root) = ctx.checkout_root.clone().or_else(|| ctx.repo_root.clone()) {
        findings.push(build_finding(
            "claude_code.writable_control_surface.repo",
            FindingScope::CurrentRepo,
            "project",
            repo_targets.iter().map(|(rel, k, d)| {
                let (j, w, n, p) = assess(&root.join(rel), *k, *d);
                (j, w, n, p)
            }),
        ));
    }

    Ok(findings)
}

/// Aggregate per-target assessments into one finding.
#[cfg(unix)]
fn build_finding<I>(id: &'static str, scope: FindingScope, label: &str, targets: I) -> Finding
where
    I: Iterator<Item = (serde_json::Value, bool, bool, bool)>,
{
    let mut entries = Vec::new();
    let mut writable = 0usize;
    let mut plantable = 0usize;
    let mut nonowner = 0usize;
    for (j, w, n, p) in targets {
        if w {
            writable += 1;
        }
        if p {
            plantable += 1;
        }
        if n {
            nonowner += 1;
        }
        entries.push(j);
    }

    let (severity, title, summary) = if nonowner > 0 {
        (
            Severity::Exposed,
            format!("{label} Claude control/instruction files writable by a non-owner"),
            format!("{nonowner} {label} policy/instruction target(s) writable by a principal other than you — shared-write tampering surface"),
        )
    } else if writable > 0 || plantable > 0 {
        (
            Severity::Notable,
            format!("{label} Claude control/instruction surface is agent-writable"),
            format!(
                "{writable} writable + {plantable} creatable {label} policy/instruction target(s) — an agent could weaken its own sandbox or plant durable instructions (a sandbox would deny these writes)"
            ),
        )
    } else {
        (
            Severity::Info,
            format!("{label} Claude control/instruction surface not writable"),
            format!("no agent-writable {label} settings or instruction files found"),
        )
    };

    Finding::new(id, FindingClass::HostPersistence, scope, title, severity, Confidence::Confirmed)
        .summary(summary)
        .evidence(json!({
            "writable_count": writable,
            "creatable_count": plantable,
            "nonowner_writable_count": nonowner,
            "targets": entries,
            "assumptions": "Writability inferred from host DAC mode bits + ownership (uid/gid vs home owner); reflects the agent-runs-as-user condition. ACLs, immutable attrs, and sandbox ro-binds are not consulted.",
            "note": "settings.json/.mcp.json are CONTROL files (writing them can disable the sandbox / add hooks); CLAUDE.md/AGENTS.md/skills are INSTRUCTION surface (writing them is durable prompt injection).",
        }))
        .remediation(&[
            "Run agents under a sandbox that denies writes to .claude/settings*.json, .mcp.json, and CLAUDE.md/AGENTS.md — the files that constrain or instruct the agent.",
            "Review agent edits to these as policy/instruction changes, not ordinary file edits.",
        ])
}

#[cfg(unix)]
fn info(probe: &ClaudeSurfaceProbe, why: &str) -> Finding {
    Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        "Claude control surface — not assessed",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary(why.to_string())
    .evidence(json!({ "assessed": false }))
}

#[cfg(not(unix))]
fn platform_run(probe: &ClaudeSurfaceProbe, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
    Ok(vec![Finding::new(
        probe.id(),
        probe.class(),
        FindingScope::Ambient,
        "Claude control surface — unsupported platform",
        Severity::Info,
        Confidence::Unknown,
    )
    .summary("writability inference is implemented for unix only")
    .evidence(json!({ "platform_supported": false }))])
}
