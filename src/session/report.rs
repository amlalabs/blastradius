//! §23.9 / §23.10 — the frozen OUTPUT contract: `SessionReport` (+ nested
//! types) and the containment simulator. `score.rs` fills the numbers,
//! `toxic_combinations.rs` fills `toxic_combinations`, `report.rs` assembles and
//! renders (Layer-2 swept).
//!
//! All evidence is value-free: shortened paths, command shapes, `host:port`,
//! MCP `server`/`tool` names, counts, and finding ids/titles only.

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, FindingId};
use crate::session::trace::SessionTrace;

/// Session risk level. A **session** concept, deliberately separate from the
/// §7.3 `Finding` `Severity` scale (§23.9 severity-scale note).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    /// §23.6 bands: 0-24 low · 25-49 medium · 50-74 high · 75-100 critical.
    pub fn from_score(score: u8) -> RiskLevel {
        match score {
            0..=24 => RiskLevel::Low,
            25..=49 => RiskLevel::Medium,
            50..=74 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

/// §23.6 policy decision derived from the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Block,
    RequireReview,
    Allow,
}

impl PolicyDecision {
    /// critical → block · high → require_review · otherwise allow.
    pub fn from_level(level: RiskLevel) -> PolicyDecision {
        match level {
            RiskLevel::Critical => PolicyDecision::Block,
            RiskLevel::High => PolicyDecision::RequireReview,
            _ => PolicyDecision::Allow,
        }
    }
}

/// A named, activated security path (§23.8 output form).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToxicCombination {
    /// stable snake_case id from §23.8.
    pub name: String,
    pub severity: RiskLevel,
    pub evidence: Vec<String>,
}

/// One decomposed contribution to the score, with its real `finding_ref`
/// back-pointer (the JSON-level proof the numerator came from the denominator).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reason {
    pub signal: String,
    pub weight: i32,
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_ref: Option<FindingId>,
}

/// §23.10 containment controls (labels on suppression sets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentControl {
    RepoOnlyFilesystem,
    NoEgress,
    NoSshAgent,
    ScopedTempCloudCreds,
    ProcessIsolation,
    AllControls,
}

/// §23.10 — one independent control recomputed from baseline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainmentResult {
    pub control: ContainmentControl,
    /// §15 category label.
    pub category: String,
    pub score: u8,
    /// baseline_score - score (>= 0).
    pub reduction: u8,
    pub risk_level: RiskLevel,
    /// REAL probe ids.
    pub suppressed_findings: Vec<FindingId>,
    /// same ids as toxic_combinations[].name.
    pub suppressed_combinations: Vec<String>,
}

/// §23.10 — one rung of the cumulative ladder (the headline).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainmentStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ContainmentControl>,
    pub score: u8,
    pub reduction: u8,
}

/// §23.10 — the quantified containment simulator output.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContainmentSimulation {
    pub baseline_score: u8,
    /// INDEPENDENT: each control recomputed from baseline.
    pub controls: Vec<ContainmentResult>,
    /// CUMULATIVE ladder (the headline).
    pub stacked: Vec<ContainmentStep>,
    /// == all_controls score.
    pub residual_floor: u8,
    /// why isolation can't reach 0 (signal ids).
    pub residual_reasons: Vec<String>,
}

/// §23.9 — the frozen session output contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionReport {
    pub session_id: String,
    pub agent: String,
    pub repo: Option<String>,
    /// 0..=100 (capped).
    pub risk_score: u8,
    pub risk_level: RiskLevel,
    pub policy_decision: PolicyDecision,
    pub summary: String,
    /// capability names; full join detail lives in `reasons[]`.
    pub activated_capabilities: Vec<String>,
    pub toxic_combinations: Vec<ToxicCombination>,
    pub reasons: Vec<Reason>,
    pub recommended_actions: Vec<String>,
    pub containment_simulation: ContainmentSimulation,
}

/// Rank `recommended_actions[]` by the biggest single-control win (§23.10): the
/// independent (not stacked) deltas, descending, so "biggest win first" is
/// stable regardless of stack order.
fn recommended_actions(sim: &ContainmentSimulation) -> Vec<String> {
    let mut ranked: Vec<&ContainmentResult> =
        sim.controls.iter().filter(|c| c.reduction > 0).collect();
    ranked.sort_by(|a, b| {
        b.reduction
            .cmp(&a.reduction)
            // stable tiebreak by control discriminant order.
            .then_with(|| control_order(a.control).cmp(&control_order(b.control)))
    });
    let mut out: Vec<String> = ranked
        .iter()
        .map(|c| {
            format!(
                "{} ({}) would reduce the score by {} (to {})",
                control_label(c.control),
                c.category,
                c.reduction,
                c.score
            )
        })
        .collect();
    if sim.residual_floor > 0 {
        out.push(format!(
            "irreducible residual {} needs human review / server-side enforcement (§15)",
            sim.residual_floor
        ));
    }
    out
}

fn control_order(c: ContainmentControl) -> u8 {
    match c {
        ContainmentControl::RepoOnlyFilesystem => 0,
        ContainmentControl::NoEgress => 1,
        ContainmentControl::NoSshAgent => 2,
        ContainmentControl::ScopedTempCloudCreds => 3,
        ContainmentControl::ProcessIsolation => 4,
        ContainmentControl::AllControls => 5,
    }
}

fn control_label(c: ContainmentControl) -> &'static str {
    match c {
        ContainmentControl::RepoOnlyFilesystem => "repo-only filesystem",
        ContainmentControl::NoEgress => "no egress",
        ContainmentControl::NoSshAgent => "no ssh-agent",
        ContainmentControl::ScopedTempCloudCreds => "scoped temp cloud creds",
        ContainmentControl::ProcessIsolation => "process isolation",
        ContainmentControl::AllControls => "all controls",
    }
}

/// A value-free, claim-bounded one-line summary (§23.11 wording boundary).
fn build_summary(
    report_level: RiskLevel,
    score: u8,
    n_caps: usize,
    n_combos: usize,
) -> String {
    if n_caps == 0 {
        return format!(
            "{} risk ({score}): no reachable capability was activated by this session",
            report_level.label()
        );
    }
    format!(
        "{} risk ({score}): {n_caps} reachable capabilit{} activated; {n_combos} toxic-combination path{} composed",
        report_level.label(),
        if n_caps == 1 { "y" } else { "ies" },
        if n_combos == 1 { "" } else { "s" }
    )
}

/// Assemble a `SessionReport` from a trace + baseline by running the full
/// deterministic pipeline: normalize → classify → toxic_combinations → score →
/// containment simulation.
pub fn build_session_report(trace: &SessionTrace, baseline: &[Finding]) -> SessionReport {
    use crate::session::classify::{classify, finding_is_present};
    use crate::session::normalize::normalize;
    use crate::session::score::{score_session, simulate_containment, ScoreInputs};
    use crate::session::toxic_combinations::evaluate;

    let normalized = normalize(&trace.events);
    let classification = classify(&normalized, baseline);

    let present_ids: Vec<String> = baseline
        .iter()
        .filter(|f| finding_is_present(f))
        .map(|f| f.id.clone())
        .collect();
    let toxic = evaluate(&normalized, &present_ids);

    let inputs = ScoreInputs::new(trace, &normalized, &classification, &toxic, baseline);
    let risk_score = score_session(&inputs);
    let risk_level = RiskLevel::from_score(risk_score);
    let policy_decision = PolicyDecision::from_level(risk_level);
    let containment_simulation = simulate_containment(&inputs);

    let activated_capabilities: Vec<String> = classification
        .activated
        .iter()
        .map(|c| c.capability.clone())
        .collect();

    let recommended = recommended_actions(&containment_simulation);
    let summary = build_summary(
        risk_level,
        risk_score,
        activated_capabilities.len(),
        toxic.len(),
    );

    SessionReport {
        session_id: trace.session_id.clone(),
        agent: trace.agent.clone(),
        repo: trace.repo.clone(),
        risk_score,
        risk_level,
        policy_decision,
        summary,
        activated_capabilities,
        toxic_combinations: toxic,
        reasons: classification.reasons,
        recommended_actions: recommended,
        containment_simulation,
    }
}

/// Rank discovered sessions for the dashboard's top-N view as a **distinct-risk**
/// set: maximize both score *uniqueness* and the *top* scores. Many heavy sessions
/// tie at the 100 cap, so showing the literal top-N would render N identical "100"s.
/// Instead we collapse to ONE representative per distinct `risk_score` — the worst
/// example at that level (most toxic paths, then largest weight magnitude) — and
/// take the N highest distinct scores. The result is a strictly-descending spread
/// (e.g. 100, 92, 81, …), each card the scariest session at its risk level.
///
/// If fewer than `top_n` distinct scores exist, returns all of them (fewer cards)
/// rather than padding with duplicate-score sessions, which would defeat the point.
pub fn rank_sessions(
    traces: &[SessionTrace],
    baseline: &[Finding],
    top_n: usize,
) -> Vec<SessionReport> {
    let mut reports: Vec<SessionReport> = traces
        .iter()
        .map(|t| build_session_report(t, baseline))
        .collect();
    // Worst-first composite: score, then # toxic paths, then raw (un-clamped)
    // weight magnitude, then a deterministic id tiebreak. Within a score, the
    // first entry is the most illustrative session (becomes that score's rep).
    let key = |r: &SessionReport| -> (u8, usize, i64) {
        let raw: i64 = r.reasons.iter().map(|x| x.weight as i64).sum();
        (r.risk_score, r.toxic_combinations.len(), raw)
    };
    reports.sort_by(|a, b| {
        let (sa, ta, ra) = key(a);
        let (sb, tb, rb) = key(b);
        sb.cmp(&sa)
            .then(tb.cmp(&ta))
            .then(rb.cmp(&ra))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    // A "top risky sessions" view excludes sessions that scored nothing.
    reports.retain(|r| r.risk_score > 0);
    // Keep one representative per distinct score (equal scores are now adjacent,
    // so dedup_by_key keeps the first = worst at that score), then take the top N.
    reports.dedup_by_key(|r| r.risk_score);
    reports.truncate(top_n);
    reports
}

/// Render a `SessionReport` as a value-free terminal block (Layer-2 swept).
///
/// The number is **always** paired with the decomposed `reasons[]` and their
/// `finding_ref` back-pointers — never a bare `Risk: 87` (§7.2/§23.12).
pub fn render_terminal(report: &SessionReport) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();

    let _ = writeln!(
        s,
        "session {} ({}) — repo {}",
        report.session_id,
        report.agent,
        report.repo.as_deref().unwrap_or("(unknown)")
    );
    let _ = writeln!(
        s,
        "  blast radius: {} [{}]  policy: {:?}",
        report.risk_score,
        report.risk_level.label(),
        report.policy_decision
    );
    let _ = writeln!(s, "  {}", report.summary);

    if !report.reasons.is_empty() {
        let _ = writeln!(s, "\n  why this score (decomposed):");
        for r in &report.reasons {
            let reff = r
                .finding_ref
                .as_deref()
                .map(|f| format!(" → {f}"))
                .unwrap_or_default();
            let _ = writeln!(s, "    {:+} {}{}", r.weight, r.signal, reff);
            for e in &r.evidence {
                let _ = writeln!(s, "        · {e}");
            }
        }
    }

    if !report.toxic_combinations.is_empty() {
        let _ = writeln!(s, "\n  activated toxic-combination paths:");
        for c in &report.toxic_combinations {
            let _ = writeln!(s, "    [{}] {}", c.severity.label(), c.name);
            for e in &c.evidence {
                let _ = writeln!(s, "        · {e}");
            }
        }
    }

    let sim = &report.containment_simulation;
    if !sim.stacked.is_empty() {
        let _ = writeln!(s, "\n  blast radius under containment:");
        for step in &sim.stacked {
            match step.control {
                None => {
                    let _ = writeln!(s, "    baseline (no controls)        {:>3}", step.score);
                }
                Some(c) => {
                    let _ = writeln!(
                        s,
                        "    + {:<26} {:>3}   -{}",
                        control_label(c),
                        step.score,
                        step.reduction
                    );
                }
            }
        }
        let _ = writeln!(s, "    irreducible residual          {:>3}", sim.residual_floor);
        if !sim.residual_reasons.is_empty() {
            let _ = writeln!(s, "      └ survives: {}", sim.residual_reasons.join(", "));
        }
    }

    if !report.recommended_actions.is_empty() {
        let _ = writeln!(s, "\n  recommended (biggest single win first):");
        for a in &report.recommended_actions {
            let _ = writeln!(s, "    - {a}");
        }
    }

    crate::report::redaction::sweep(&s)
}

/// Render a `SessionReport` as value-free JSON (Layer-2 swept).
pub fn render_json(report: &SessionReport) -> String {
    let raw = serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    crate::report::redaction::sweep(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{FindingClass, FindingScope};
    use crate::session::trace::AgentEvent;
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
        ]
    }

    fn load_fixture(name: &str) -> SessionTrace {
        let path = format!("{}/traces/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap();
        SessionTrace::from_json_str(&text).unwrap()
    }

    #[test]
    fn benign_fixture_low_no_combos() {
        let report = build_session_report(&load_fixture("benign"), &rich_baseline());
        assert!(report.risk_score < 50, "benign got {}", report.risk_score);
        assert!(report.toxic_combinations.is_empty());
        assert_eq!(report.policy_decision, PolicyDecision::Allow);
    }

    #[test]
    fn risky_fixture_critical_with_paths_and_finding_refs() {
        let report = build_session_report(&load_fixture("risky"), &rich_baseline());
        assert!(report.risk_score >= 75, "risky got {}", report.risk_score);
        let names: Vec<&str> = report.toxic_combinations.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"exfiltration_path"));
        assert!(names.contains(&"production_deployment_path"));
        // every reason with an anchor carries a REAL finding ref from the baseline.
        for r in &report.reasons {
            if let Some(fr) = &r.finding_ref {
                assert!(rich_baseline().iter().any(|f| &f.id == fr), "phantom ref {fr}");
            }
        }
        assert_eq!(report.policy_decision, PolicyDecision::Block);
    }

    #[test]
    fn determinism_byte_identical() {
        let t = load_fixture("risky");
        let a = render_json(&build_session_report(&t, &rich_baseline()));
        let b = render_json(&build_session_report(&t, &rich_baseline()));
        assert_eq!(a, b);
    }

    /// §4.4 canary self-test for the session renderers: secrets planted in the
    /// DROPPED fields (`file_write.diff`, `mcp_call.input`) and the RETAINED
    /// field (`shell_command.command`) must be absent from terminal + JSON.
    #[test]
    fn canary_does_not_leak_through_session_renderers() {
        let canary = "br_test_SHOULD_NOT_LEAK";
        let trace = SessionTrace {
            session_id: "canary".into(),
            agent: "mock".into(),
            repo: Some("blastradius".into()),
            started_at: None,
            events: vec![
                AgentEvent::FileWrite {
                    path: "src/auth/login.rs".into(),
                    diff: Some(format!("+ secret = {canary}")),
                },
                AgentEvent::McpCall {
                    server: "external.example".into(),
                    tool: "exec".into(),
                    input: Some(serde_json::json!({ "token": canary })),
                },
                AgentEvent::ShellCommand {
                    command: format!("export BR_CANARY={canary} && curl ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"),
                },
            ],
            privileged_user: false,
            after_hours: false,
        };
        let report = build_session_report(&trace, &rich_baseline());
        let term = render_terminal(&report);
        let json = render_json(&report);
        for (name, rendered) in [("terminal", &term), ("json", &json)] {
            assert!(!rendered.contains(canary), "canary leaked in {name}");
            assert!(
                !crate::report::redaction::contains_secret_shaped(rendered),
                "secret shape survived {name}"
            );
        }
    }

    #[test]
    fn score_snapshot_risky_fixture() {
        // Pin the risky-fixture score so a scoring drift is caught (§18).
        let report = build_session_report(&load_fixture("risky"), &rich_baseline());
        assert_eq!(report.risk_score, EXPECTED_RISKY_SCORE);
    }

    /// Snapshot constant — the risky fixture sits in the soft-saturation tail
    /// (§23.6): a heavy session asymptotes toward 100 (here 99) rather than
    /// hard-pinning, which is what lets the ranking tell the worst sessions apart.
    const EXPECTED_RISKY_SCORE: u8 = 99;
}
