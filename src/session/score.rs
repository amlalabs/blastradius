//! §23.6 / §23.10 — the deterministic scoring engine + containment simulator.
//!
//! Additive base weights × multipliers (+ escalation amplifier), with
//! toxic-combination PATH weights added to the base sum. The deterministic
//! engine computes the score; the `--ai` layer only explains it.
//!
//! The containment simulator is **pure arithmetic over already-collected
//! evidence** — a counterfactual recompute, never an action. Both the headline
//! score and every containment toggle route through the single `compute_score`
//! function so they can never drift.

use std::collections::BTreeSet;

use crate::finding::{Finding, FindingId};
use crate::session::classify::Classification;
use crate::session::normalize::{NormalizedEvent, Signal};
use crate::session::report::{
    ContainmentControl, ContainmentResult, ContainmentSimulation, ContainmentStep, RiskLevel,
    ToxicCombination,
};
use crate::session::toxic_combinations::rule_for;
use crate::session::trace::SessionTrace;

/// §23.6 base weights (additive; a signal contributes only when it joins).
pub mod weights {
    pub const READ_SECRET: i32 = 30;
    pub const MODIFIED_PRODUCTION_DEPLOY: i32 = 25;
    pub const SHELL_COMMAND: i32 = 10;
    pub const NETWORK_ACCESS: i32 = 15;
    pub const EDITED_AUTH_PAYMENT_SECURITY: i32 = 20;
    pub const DANGEROUS_SHELL_PATTERN: i32 = 25;
    pub const MODIFIED_DEPENDENCY_MANIFEST: i32 = 15;
    pub const EXTERNAL_MCP_CALL: i32 = 15;
    pub const HUMAN_APPROVED_RISKY_ACTION: i32 = -10;
}

/// §23.6 multipliers.
pub mod multipliers {
    pub const PRODUCTION_REPO: f64 = 1.4;
    pub const PRIVILEGED_USER: f64 = 1.2;
    pub const UNAPPROVED: f64 = 1.3;
    pub const MULTI_SENSITIVE_DOMAIN: f64 = 1.25;
    pub const AFTER_HOURS: f64 = 1.1;
    /// escalation amplifier range; 1.5 when escalation reachable AND a shell ran.
    pub const ESCALATION_MIN: f64 = 1.0;
    pub const ESCALATION_MAX: f64 = 1.5;
}

/// §23.6 toxic-combination PATH weights added to the base sum.
pub mod path_weights {
    pub const CRITICAL: i32 = 40;
    pub const HIGH: i32 = 25;
    pub const MEDIUM: i32 = 15;
}

/// Findings that drive the escalation amplifier (§23.7) — the actual presence of
/// these in the baseline, never toxic-rule activation.
const ESCALATION_FINDINGS: &[&str] = &["host.privilege_escalation", "host.privileged_reachability"];

/// Clamp a raw score into the `0..=100` band.
pub fn clamp_score(raw: f64) -> u8 {
    raw.round().clamp(0.0, 100.0) as u8
}

/// Path weight contribution for a combination's severity.
fn combo_path_weight(severity: RiskLevel) -> i32 {
    match severity {
        RiskLevel::Critical => path_weights::CRITICAL,
        RiskLevel::High => path_weights::HIGH,
        RiskLevel::Medium => path_weights::MEDIUM,
        RiskLevel::Low => 0,
    }
}

/// All inputs the deterministic score needs, pre-extracted so both the headline
/// and every containment toggle recompute from the *same* evidence.
pub struct ScoreInputs<'a> {
    pub trace: &'a SessionTrace,
    pub events: &'a [NormalizedEvent],
    pub classification: &'a Classification,
    pub toxic: &'a [ToxicCombination],
    /// finding ids present in the baseline at the §23.8(b) gate.
    pub present_ids: BTreeSet<String>,
}

impl<'a> ScoreInputs<'a> {
    pub fn new(
        trace: &'a SessionTrace,
        events: &'a [NormalizedEvent],
        classification: &'a Classification,
        toxic: &'a [ToxicCombination],
        baseline: &[Finding],
    ) -> ScoreInputs<'a> {
        let present_ids = baseline
            .iter()
            .filter(|f| crate::session::classify::finding_is_present(f))
            .map(|f| f.id.clone())
            .collect();
        ScoreInputs {
            trace,
            events,
            classification,
            toxic,
            present_ids,
        }
    }
}

/// Which sensitive domain (if any) a reason belongs to (§23.6 multi-domain).
fn sensitive_domain(signal: &str) -> Option<&'static str> {
    match signal {
        "read_secret" => Some("creds"),
        "modified_production_deploy" => Some("deploy"),
        "edited_auth_payment_security_code" => Some("auth-payment"),
        "network_access" | "external_mcp_call" => Some("network"),
        _ => None,
    }
}

/// The single deterministic recompute. `suppressed` is the set of ambient
/// finding ids a containment control removes; pass an empty set for the headline
/// score. Returns the clamped `0..=100` score.
///
/// Recompute rules (§23.10): drop any reason whose `finding_ref ∈ suppressed`;
/// drop any combination with any required leg ∈ suppressed; drop the escalation
/// amplifier when its driving finding ∈ suppressed; event-intrinsic signals
/// (no ambient anchor) survive every control.
pub fn compute_score(inputs: &ScoreInputs, suppressed: &BTreeSet<String>) -> u8 {
    // --- surviving reasons & base sum ---
    // Group surviving reasons BY SIGNAL with diminishing returns: the first
    // occurrence of a signal counts full weight; each repeat adds only 20% of the
    // weight, capped at +1× (so a signal maxes at 2× its base). This stops raw
    // event COUNT from dominating — e.g. 551 shell commands no longer contribute
    // +5510 (it caps at +20), so the score reflects WHICH distinct dangerous
    // capabilities a session exercised, not how many times. Negative weights
    // (credits like human_approved_risky_action) apply once.
    let mut domains: BTreeSet<&'static str> = BTreeSet::new();
    let mut any_unapproved_risky = false;
    // signal -> (base weight, occurrence count), insertion order not needed.
    let mut by_signal: std::collections::BTreeMap<&str, (i32, i32)> =
        std::collections::BTreeMap::new();

    for r in &inputs.classification.reasons {
        // Drop a reason whose ambient anchor was suppressed.
        if let Some(fid) = &r.finding_ref {
            if suppressed.contains(fid) {
                continue;
            }
        }
        let e = by_signal.entry(r.signal.as_str()).or_insert((r.weight, 0));
        e.1 += 1;
        if let Some(d) = sensitive_domain(&r.signal) {
            domains.insert(d);
            if r.signal != "human_approved_risky_action" {
                any_unapproved_risky = true;
            }
        }
    }

    let mut base_sum: i32 = 0;
    for (weight, count) in by_signal.values() {
        if *weight < 0 {
            base_sum += *weight; // credits apply once, no repeat scaling
            continue;
        }
        let repeats = (*count - 1).max(0) as f64;
        let bonus = (repeats * 0.20 * (*weight as f64)).min(*weight as f64);
        base_sum += *weight + bonus.round() as i32;
    }

    // --- surviving toxic-combination path weights ---
    for combo in inputs.toxic {
        let Some(rule) = rule_for(&combo.name) else {
            continue;
        };
        // A path needs ALL its required ambient legs; if any required leg is
        // suppressed, the path collapses.
        let legs = crate::session::toxic_combinations::walk_trigger(
            rule.finding_triggers(),
            &inputs.present_ids.iter().cloned().collect::<Vec<_>>(),
        );
        let collapsed = legs
            .iter()
            .any(|(fid, required)| *required && suppressed.contains(fid));
        if collapsed {
            continue;
        }
        base_sum += combo_path_weight(combo.severity);
    }

    if base_sum <= 0 {
        // negative/zero base (e.g. only a credit) clamps to 0.
        return clamp_score(base_sum as f64);
    }

    // --- multipliers ---
    let mut mult = 1.0_f64;

    // production_repo: keyed on reachable push surface (git.push_likelihood),
    // suppressible by controls that remove its basis.
    if inputs.present_ids.contains("git.push_likelihood")
        && !suppressed.contains("git.push_likelihood")
    {
        mult *= multipliers::PRODUCTION_REPO;
    }

    if inputs.trace.privileged_user {
        mult *= multipliers::PRIVILEGED_USER;
    }

    // unapproved: a risky sensitive action ran without a covering approval.
    let has_approval = inputs
        .events
        .iter()
        .any(|e| e.signal == Signal::HumanApprovedRiskyAction);
    if any_unapproved_risky && !has_approval {
        mult *= multipliers::UNAPPROVED;
    }

    if domains.len() >= 2 {
        mult *= multipliers::MULTI_SENSITIVE_DOMAIN;
    }

    if inputs.trace.after_hours {
        mult *= multipliers::AFTER_HOURS;
    }

    // escalation amplifier (§23.7): driven by ACTUAL presence of an escalation
    // finding (not suppressed) AND a shell command having run.
    let escalation_present = ESCALATION_FINDINGS
        .iter()
        .any(|id| inputs.present_ids.contains(*id) && !suppressed.contains(*id));
    let shell_ran = inputs
        .events
        .iter()
        .any(|e| e.signal == Signal::ShellCommand);
    let escalation = if escalation_present && shell_ran {
        multipliers::ESCALATION_MAX
    } else {
        multipliers::ESCALATION_MIN
    };
    mult *= escalation;

    clamp_score(base_sum as f64 * mult)
}

/// Compute the headline deterministic risk score for a session.
pub fn score_session(inputs: &ScoreInputs) -> u8 {
    compute_score(inputs, &BTreeSet::new())
}

// ---------------------------------------------------------------------------
// §23.10 containment simulator
// ---------------------------------------------------------------------------

/// The suppression set + §15 category label for a control (§23.10 table). Ids
/// are real shipped probe ids; ids matching nothing suppress nothing.
fn suppression_set(control: ContainmentControl) -> (&'static str, Vec<&'static str>) {
    match control {
        ContainmentControl::ScopedTempCloudCreds => (
            "Credential substitution",
            vec![
                "aws.credentials.profiles",
                "github.token_source",
                "git.credential_store",
                "env.secret_names",
            ],
        ),
        ContainmentControl::RepoOnlyFilesystem => (
            "Filesystem isolation",
            vec![
                "cross_repo.dotenv",
                "cross_repo.lateral_secrets",
                "cross_repo.sibling_repos",
                "browser.session_stores",
                "credentials.shell_history",
            ],
        ),
        ContainmentControl::NoEgress => (
            "Egress control",
            vec!["egress.connectivity", "egress.mediation"],
        ),
        ContainmentControl::NoSshAgent => (
            "Credential substitution (ssh-agent)",
            vec!["ssh.agent_socket"],
        ),
        ContainmentControl::ProcessIsolation => (
            "Process isolation",
            vec![
                "process.proc_environ",
                "process.memory_introspection",
                "process.cmdline_secrets",
                "host.privilege_escalation",
                "host.privileged_reachability",
            ],
        ),
        ContainmentControl::AllControls => ("All controls", Vec::new()),
    }
}

/// The fixed stacked ladder order (§23.10).
const LADDER: &[ContainmentControl] = &[
    ContainmentControl::RepoOnlyFilesystem,
    ContainmentControl::NoEgress,
    ContainmentControl::NoSshAgent,
    ContainmentControl::ScopedTempCloudCreds,
    ContainmentControl::ProcessIsolation,
];

/// Union of every control's suppression set (the `all_controls` set).
fn all_controls_set() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for c in LADDER {
        for id in suppression_set(*c).1 {
            set.insert(id.to_string());
        }
    }
    set
}

/// Which toxic combinations a suppression set neutralizes (any required leg in
/// the set), and which surviving suppressed findings were actually present.
fn neutralized(
    inputs: &ScoreInputs,
    suppressed: &BTreeSet<String>,
) -> (Vec<FindingId>, Vec<String>) {
    let present: Vec<String> = inputs.present_ids.iter().cloned().collect();

    let suppressed_findings: Vec<FindingId> = suppressed
        .iter()
        .filter(|id| inputs.present_ids.contains(*id))
        .cloned()
        .collect();

    let mut suppressed_combos = Vec::new();
    for combo in inputs.toxic {
        if let Some(rule) = rule_for(&combo.name) {
            let legs = crate::session::toxic_combinations::walk_trigger(rule.finding_triggers(), &present);
            if legs
                .iter()
                .any(|(fid, required)| *required && suppressed.contains(fid))
            {
                suppressed_combos.push(combo.name.clone());
            }
        }
    }
    (suppressed_findings, suppressed_combos)
}

/// The signals/combos that survive `all_controls` and therefore make the
/// residual floor non-zero (§23.10) — a reason whose ambient anchor is not in
/// any suppression set, or which has no anchor at all (event-intrinsic).
fn residual_reasons(inputs: &ScoreInputs, all_set: &BTreeSet<String>) -> Vec<String> {
    let mut reasons = BTreeSet::new();
    for r in &inputs.classification.reasons {
        if r.weight <= 0 {
            continue;
        }
        let survives = match &r.finding_ref {
            None => true, // no ambient anchor → event-intrinsic.
            Some(fid) => !all_set.contains(fid),
        };
        if survives {
            reasons.insert(r.signal.clone());
        }
    }
    // A toxic-combination path survives when none of its required ambient legs
    // is suppressed (the `None`-trigger `high_review_risk` always survives).
    let present: Vec<String> = inputs.present_ids.iter().cloned().collect();
    for combo in inputs.toxic {
        if let Some(rule) = rule_for(&combo.name) {
            let legs = crate::session::toxic_combinations::walk_trigger(rule.finding_triggers(), &present);
            let collapsed = legs
                .iter()
                .any(|(fid, required)| *required && all_set.contains(fid));
            if !collapsed {
                reasons.insert(format!("toxic:{}", combo.name));
            }
        }
    }
    reasons.into_iter().collect()
}

/// Build the full §23.10 containment simulation.
pub fn simulate_containment(inputs: &ScoreInputs) -> ContainmentSimulation {
    let baseline_score = compute_score(inputs, &BTreeSet::new());

    // Independent controls — each recomputed from the headline baseline.
    let mut controls = Vec::new();
    for control in LADDER {
        let (category, ids) = suppression_set(*control);
        let suppressed: BTreeSet<String> = ids.iter().map(|s| s.to_string()).collect();
        let score = compute_score(inputs, &suppressed);
        let (suppressed_findings, suppressed_combinations) = neutralized(inputs, &suppressed);
        controls.push(ContainmentResult {
            control: *control,
            category: category.to_string(),
            score,
            reduction: baseline_score.saturating_sub(score),
            risk_level: RiskLevel::from_score(score),
            suppressed_findings,
            suppressed_combinations,
        });
    }

    // Stacked ladder — cumulative union in the fixed order.
    let mut stacked = vec![ContainmentStep {
        control: None,
        score: baseline_score,
        reduction: 0,
    }];
    let mut acc: BTreeSet<String> = BTreeSet::new();
    let mut prev = baseline_score;
    for control in LADDER {
        for id in suppression_set(*control).1 {
            acc.insert(id.to_string());
        }
        let score = compute_score(inputs, &acc);
        stacked.push(ContainmentStep {
            control: Some(*control),
            score,
            reduction: prev.saturating_sub(score),
        });
        prev = score;
    }

    let all_set = all_controls_set();
    let residual_floor = compute_score(inputs, &all_set);

    ContainmentSimulation {
        baseline_score,
        controls,
        stacked,
        residual_floor,
        residual_reasons: residual_reasons(inputs, &all_set),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{Finding, FindingClass, FindingScope};
    use crate::session::classify::classify;
    use crate::session::normalize::normalize;
    use crate::session::toxic_combinations::evaluate;
    use crate::session::trace::{AgentEvent, SessionTrace};
    use crate::severity::{Confidence, Severity};

    fn f(id: &str, class: FindingClass, scope: FindingScope, sev: Severity) -> Finding {
        Finding::new(id, class, scope, id, sev, Confidence::Likely)
    }

    fn rich_baseline() -> Vec<Finding> {
        vec![
            f("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient, Severity::Exposed),
            f("git.push_likelihood", FindingClass::GitWrite, FindingScope::Ambient, Severity::Exposed),
            f("egress.connectivity", FindingClass::Egress, FindingScope::Network, Severity::Exposed),
            f("process.sandbox_reach", FindingClass::Process, FindingScope::Host, Severity::Notable),
            f("ssh.agent_socket", FindingClass::Credentials, FindingScope::Ambient, Severity::Notable),
        ]
    }

    fn score_all(trace: &SessionTrace, baseline: &[Finding]) -> (u8, Vec<ToxicCombination>) {
        let norm = normalize(&trace.events);
        let cls = classify(&norm, baseline);
        let present: Vec<String> = baseline
            .iter()
            .filter(|f| crate::session::classify::finding_is_present(f))
            .map(|f| f.id.clone())
            .collect();
        let toxic = evaluate(&norm, &present);
        let inputs = ScoreInputs::new(trace, &norm, &cls, &toxic, baseline);
        (score_session(&inputs), toxic)
    }

    #[test]
    fn risky_session_is_high_or_critical() {
        let trace = SessionTrace {
            session_id: "r".into(),
            agent: "mock".into(),
            repo: Some("blastradius".into()),
            started_at: None,
            events: vec![
                AgentEvent::FileRead { path: "~/.aws/credentials".into() },
                AgentEvent::FileWrite { path: ".github/workflows/deploy.yml".into(), diff: None },
                AgentEvent::ShellCommand { command: "git push".into() },
                AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
            ],
            privileged_user: true,
            after_hours: true,
        };
        let (score, toxic) = score_all(&trace, &rich_baseline());
        assert!(score >= 75, "expected critical, got {score}");
        let names: Vec<&str> = toxic.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"exfiltration_path"));
        assert!(names.contains(&"production_deployment_path"));
    }

    #[test]
    fn benign_session_low_no_combos() {
        let trace = SessionTrace {
            session_id: "b".into(),
            agent: "mock".into(),
            repo: Some("blastradius".into()),
            started_at: None,
            events: vec![
                AgentEvent::FileRead { path: "src/lib.rs".into() },
                AgentEvent::ShellCommand { command: "cargo test".into() },
                AgentEvent::ShellCommand { command: "cargo build".into() },
            ],
            privileged_user: false,
            after_hours: false,
        };
        let (score, toxic) = score_all(&trace, &rich_baseline());
        assert!(score < 50, "expected low/medium, got {score}");
        assert!(toxic.is_empty());
    }

    #[test]
    fn containment_ladder_descends_to_nonzero_floor() {
        // A credential-exfil session whose floor is the event-intrinsic
        // dangerous-shell pattern (survives every ambient control).
        let trace = SessionTrace {
            session_id: "r".into(),
            agent: "mock".into(),
            repo: Some("blastradius".into()),
            started_at: None,
            events: vec![
                AgentEvent::FileRead { path: "~/.aws/credentials".into() },
                AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
                AgentEvent::ShellCommand { command: "rm -rf /tmp/scratch".into() },
            ],
            privileged_user: false,
            after_hours: false,
        };
        // Baseline without push reach / ssh-agent so production_repo & the
        // source-control path do not dominate the floor.
        let baseline = vec![
            f("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient, Severity::Exposed),
            f("egress.connectivity", FindingClass::Egress, FindingScope::Network, Severity::Exposed),
            f("process.sandbox_reach", FindingClass::Process, FindingScope::Host, Severity::Notable),
        ];
        let norm = normalize(&trace.events);
        let cls = classify(&norm, &baseline);
        let present: Vec<String> = baseline
            .iter()
            .filter(|f| crate::session::classify::finding_is_present(f))
            .map(|f| f.id.clone())
            .collect();
        let toxic = evaluate(&norm, &present);
        let inputs = ScoreInputs::new(&trace, &norm, &cls, &toxic, &baseline);
        let sim = simulate_containment(&inputs);

        // Ladder is monotone non-increasing.
        for w in sim.stacked.windows(2) {
            assert!(w[1].score <= w[0].score, "ladder must not increase");
        }
        // Floor is below baseline but non-zero (high_review_risk is intrinsic).
        assert!(sim.residual_floor < sim.baseline_score);
        assert!(sim.residual_floor > 0, "floor must be non-zero (intrinsic edit)");
        assert!(!sim.residual_reasons.is_empty());
    }
}
