//! Report assembly + the three renderers (§14). Every renderer's output passes
//! through `redaction::sweep` (Layer 2) before it reaches a user.

pub mod json;
pub mod markdown;
pub mod redaction;
pub mod sanitize;
pub mod terminal;

use crate::compare::diff::Comparison;
use crate::context::{Context, Platform};
use crate::finding::Finding;

/// A single scanned context plus its findings.
pub struct ContextReport {
    pub context: Context,
    pub findings: Vec<Finding>,
}

/// The full result of a run, ready to render in any format.
pub struct RunReport {
    pub mode: String,
    pub timestamp: String,
    pub version: String,
    pub platform: Platform,
    pub command: String,
    pub contexts: Vec<ContextReport>,
    pub comparison: Option<Comparison>,
}

impl RunReport {
    /// Highest severity rank present across all contexts (for `--fail-on`).
    pub fn max_severity_rank(&self) -> u8 {
        self.contexts
            .iter()
            .flat_map(|c| c.findings.iter())
            .map(|f| f.severity.rank())
            .max()
            .unwrap_or(0)
    }
}

/// Capability-oriented containment guidance shared by all renderers (§15).
pub const CONTAINMENT: &[(&str, &str)] = &[
    (
        "Credential substitution",
        "scoped, short-lived creds per agent instead of inheriting your full shell, SSH, cloud, and git identity.",
    ),
    (
        "Filesystem isolation",
        "mount only the task repo + explicit deps; no broad $HOME or sibling-repo access.",
    ),
    (
        "Egress control",
        "default-deny outbound, then allowlist what the task needs.",
    ),
    (
        "Process isolation",
        "prevent same-user process inspection / access to other local dev tools.",
    ),
    (
        "Server-side enforcement",
        "branch protection, review, token scopes still matter; local worktrees don't enforce them.",
    ),
];

pub const TRUST_BANNER: &str = "\
blastradius — local reachability audit for coding-agent environments

Privacy:
  • no telemetry   • no findings leave this machine
  • secret values are never printed
  • the scan always runs one outbound TLS reachability check (no data sent)
";

/// Shared helper: group a context's findings by class, preserving sort order.
pub fn group_by_class(findings: &[Finding]) -> Vec<(crate::finding::FindingClass, Vec<&Finding>)> {
    let mut groups: Vec<(crate::finding::FindingClass, Vec<&Finding>)> = Vec::new();
    for f in findings {
        if let Some(entry) = groups.iter_mut().find(|(c, _)| *c == f.class) {
            entry.1.push(f);
        } else {
            groups.push((f.class, vec![f]));
        }
    }
    groups.sort_by_key(|(c, _)| c.order());
    groups
}
