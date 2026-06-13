//! Probe trait + deterministic runner (§9.3). Catches probe-level errors and
//! continues; never panics on malformed local files.

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::probes;
use crate::severity::{Confidence, Severity};

pub trait Probe {
    fn id(&self) -> &'static str;
    fn class(&self) -> FindingClass;
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>>;
}

/// The default probe battery. The concrete list lives in
/// [`probes::registry`] — see that module for how to add a detection. Findings
/// are re-sorted deterministically by [`sort_findings`], so the registry's order
/// is for readability only.
pub fn default_probes() -> Vec<Box<dyn Probe>> {
    probes::registry::all()
}

/// Run every probe against `ctx`, collecting findings. Probe errors are caught
/// and surfaced as `Info`-level findings rather than aborting the run (§19).
pub fn run_all(ctx: &Context, probes: &[Box<dyn Probe>]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for probe in probes {
        match probe.run(ctx) {
            Ok(mut fs) => findings.append(&mut fs),
            Err(e) => {
                findings.push(
                    Finding::new(
                        format!("{}.error", probe.id()),
                        probe.class(),
                        FindingScope::Ambient,
                        format!("{} probe degraded", probe.id()),
                        Severity::Info,
                        Confidence::Unknown,
                    )
                    .summary(format!("probe error: {e}")),
                );
            }
        }
    }
    sort_findings(&mut findings);
    findings
}

/// Deterministic ordering: by class, then severity (desc), then id (§9.3, §14).
pub fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        a.class
            .order()
            .cmp(&b.class.order())
            .then(b.severity.rank().cmp(&a.severity.rank()))
            .then(a.id.cmp(&b.id))
    });
}
