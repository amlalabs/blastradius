//! §23.8 — the toxic-combination catalog: event(s) × ambient finding(s) → a
//! named security PATH. Each entry is a deterministic, value-free
//! [`ToxicCombinationRule`]. The engine — never the `--ai` layer — evaluates
//! rules and emits [`crate::session::report::ToxicCombination`].
//!
//! The six MVP rules plus the frozen accessors `finding_triggers()` and
//! `rule_for()` (required by §24.3.3 retro) are defined here. No new rules and
//! no new probe surface.

use crate::session::normalize::{is_browser_session_path, NormalizedEvent, Signal};
use crate::session::report::{RiskLevel, ToxicCombination};

/// An observed `AgentEvent` class predicate that a rule's `event_triggers`
/// requires. Kept as a stable `&'static str` tag so the catalog is a pure data
/// table; the engine maps tags to predicates over [`NormalizedEvent`].
pub type EventPredicate = &'static str;

/// Ambient `FindingId`(s) that must be present for a rule to activate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingTrigger {
    None,
    All(Vec<&'static str>),
    AnyOf(Vec<&'static str>),
    AllOf(Vec<FindingTrigger>),
}

/// A deterministic, value-free toxic-combination rule (§23.8).
#[derive(Debug, Clone)]
pub struct ToxicCombinationRule {
    /// stable snake_case id (e.g. "exfiltration_path").
    pub name: &'static str,
    /// human label (e.g. "Credential exfiltration path").
    pub title: &'static str,
    /// observed AgentEvent classes that must ALL match.
    pub event_triggers: Vec<EventPredicate>,
    /// ambient FindingId(s) that must be present.
    pub finding_triggers: FindingTrigger,
    /// medium | high | critical (never low).
    pub severity: RiskLevel,
    /// what the JOIN means, in reachability terms.
    pub derived_path: &'static str,
    /// value-free (§23.11).
    pub evidence_template: &'static str,
    /// trigger set includes an escalation finding (§23.7).
    pub escalation: bool,
}

impl ToxicCombinationRule {
    /// Frozen accessor (§24.3.3): the rule's ambient `FindingTrigger`.
    pub fn finding_triggers(&self) -> &FindingTrigger {
        &self.finding_triggers
    }
}

/// Frozen accessor (§24.3.3): look up a rule by its stable id.
pub fn rule_for(name: &str) -> Option<&'static ToxicCombinationRule> {
    catalog().iter().find(|r| r.name == name)
}

/// The 6 MVP rules per the §23.8 table. Built once and cached.
pub fn catalog() -> &'static [ToxicCombinationRule] {
    use std::sync::OnceLock;
    static CATALOG: OnceLock<Vec<ToxicCombinationRule>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

fn build_catalog() -> Vec<ToxicCombinationRule> {
    vec![
        ToxicCombinationRule {
            name: "exfiltration_path",
            title: "Credential exfiltration path",
            event_triggers: vec!["read_secret", "network_or_dangerous_egress"],
            finding_triggers: FindingTrigger::AllOf(vec![
                FindingTrigger::All(vec!["egress.connectivity"]),
                FindingTrigger::AnyOf(vec![
                    "aws.credentials.profiles",
                    "env.secret_names",
                    "ssh.private_keys",
                    "git.credential_store",
                    "browser.session_stores",
                ]),
            ]),
            severity: RiskLevel::Critical,
            derived_path: "a credential store read composes with an open egress path",
            evidence_template: "read a credential store and an egress path is reachable",
            escalation: false,
        },
        ToxicCombinationRule {
            name: "source_control_mutation_path",
            title: "Source-control mutation path",
            event_triggers: vec!["git_write"],
            finding_triggers: FindingTrigger::All(vec!["ssh.agent_socket", "git.push_likelihood"]),
            severity: RiskLevel::High,
            derived_path: "a git-write action composes with armed ssh-agent + push reach",
            evidence_template: "git-write action with ssh-agent and push reachability",
            escalation: false,
        },
        ToxicCombinationRule {
            name: "post_root_host_visibility",
            title: "Post-root host visibility",
            event_triggers: vec!["container_runtime"],
            finding_triggers: FindingTrigger::AllOf(vec![
                FindingTrigger::AnyOf(vec![
                    "host.privilege_escalation",
                    "process.afunix_docker_sock",
                ]),
                FindingTrigger::AnyOf(vec![
                    "cross_repo.sibling_repos",
                    "cross_repo.lateral_secrets",
                ]),
            ]),
            severity: RiskLevel::Critical,
            derived_path: "a container-runtime action composes with escalation + cross-repo reach",
            evidence_template: "container-runtime action with escalation and cross-repo reach",
            escalation: true,
        },
        ToxicCombinationRule {
            name: "saas_session_hijack",
            title: "SaaS session hijack",
            event_triggers: vec!["read_browser_session", "network_access"],
            finding_triggers: FindingTrigger::All(vec![
                "browser.session_stores",
                "egress.connectivity",
            ]),
            severity: RiskLevel::High,
            derived_path: "a browser session-store read composes with an open egress path",
            evidence_template: "read a browser session store and an egress path is reachable",
            escalation: false,
        },
        ToxicCombinationRule {
            name: "production_deployment_path",
            title: "Production deployment path",
            event_triggers: vec!["modified_production_deploy"],
            finding_triggers: FindingTrigger::All(vec!["git.push_likelihood"]),
            severity: RiskLevel::Critical,
            derived_path: "a deploy-workflow edit composes with likely push reach",
            evidence_template: "edited a deploy workflow and push is likely",
            escalation: false,
        },
        ToxicCombinationRule {
            name: "high_review_risk",
            title: "Unreviewed sensitive-code change",
            event_triggers: vec!["edited_auth_payment_security", "absent_approval"],
            // review-control-gap exception: clause (b) is vacuous (§23.8).
            finding_triggers: FindingTrigger::None,
            severity: RiskLevel::High,
            derived_path: "a sensitive-code edit with no covering approval",
            evidence_template: "sensitive-code edit without a covering approval",
            escalation: false,
        },
    ]
}

/// Does the set of present finding ids satisfy a `FindingTrigger` (§23.8(b))?
/// `present` is the set of ids present at the §23.8(b) gate (confidence ≥ Likely
/// OR severity ≥ Notable) — computed by the caller via `classify::finding_is_present`.
pub fn trigger_satisfied(trigger: &FindingTrigger, present: &[String]) -> bool {
    match trigger {
        FindingTrigger::None => true, // clause (b) vacuous (review-control-gap).
        FindingTrigger::All(ids) => ids.iter().all(|id| present.iter().any(|p| p == id)),
        FindingTrigger::AnyOf(ids) => ids.iter().any(|id| present.iter().any(|p| p == id)),
        FindingTrigger::AllOf(subs) => subs.iter().all(|t| trigger_satisfied(t, present)),
    }
}

/// Walk a `FindingTrigger` into `(finding_ref, required)` leg pairs (§24.3.3).
/// `All` → all required; `AnyOf` → each required only when it is the sole
/// present member; `AllOf` → recurse; `None` → no legs.
pub fn walk_trigger(trigger: &FindingTrigger, present: &[String]) -> Vec<(String, bool)> {
    match trigger {
        FindingTrigger::None => Vec::new(),
        FindingTrigger::All(ids) => ids.iter().map(|id| (id.to_string(), true)).collect(),
        FindingTrigger::AnyOf(ids) => {
            let present_members: Vec<&&str> =
                ids.iter().filter(|id| present.iter().any(|p| p == *id)).collect();
            let sole = present_members.len() == 1;
            ids.iter()
                .map(|id| {
                    let is_present = present.iter().any(|p| p == id);
                    // required only when it is the sole present member.
                    (id.to_string(), sole && is_present)
                })
                .collect()
        }
        FindingTrigger::AllOf(subs) => subs
            .iter()
            .flat_map(|t| walk_trigger(t, present))
            .collect(),
    }
}

/// Does a normalized event stream satisfy one event-trigger tag? Each tag maps
/// to a predicate over the value-free `Signal` + `join_key` shapes.
fn event_trigger_matched(tag: &str, events: &[NormalizedEvent]) -> bool {
    let any = |pred: &dyn Fn(&NormalizedEvent) -> bool| events.iter().any(pred);
    match tag {
        "read_secret" => any(&|e| e.signal == Signal::ReadSecret),
        "network_access" => any(&|e| e.signal == Signal::NetworkAccess),
        "network_or_dangerous_egress" => any(&|e| {
            e.signal == Signal::NetworkAccess || e.signal == Signal::DangerousShellPattern
        }),
        "modified_production_deploy" => any(&|e| e.signal == Signal::ModifiedProductionDeploy),
        "edited_auth_payment_security" => {
            any(&|e| e.signal == Signal::EditedAuthOrPaymentOrSecurityCode)
        }
        // absence of any covering approval in the session.
        "absent_approval" => !any(&|e| e.signal == Signal::HumanApprovedRiskyAction),
        // git-write: a shell shape that is a git mutation, OR a deploy/sensitive write.
        "git_write" => {
            any(&|e| {
                e.signal == Signal::ShellCommand
                    && e.join_key
                        .as_deref()
                        .map(is_git_write_shape)
                        .unwrap_or(false)
            }) || any(&|e| {
                matches!(
                    e.signal,
                    Signal::ModifiedProductionDeploy | Signal::EditedAuthOrPaymentOrSecurityCode
                )
            })
        }
        // container-runtime: docker/podman run/exec, or an external MCP docker call.
        "container_runtime" => any(&|e| {
            (e.signal == Signal::ShellCommand
                && e.join_key
                    .as_deref()
                    .map(is_container_runtime_shape)
                    .unwrap_or(false))
                || (e.signal == Signal::ExternalMcpCall
                    && e.join_key
                        .as_deref()
                        .map(|k| k.to_ascii_lowercase().contains("docker"))
                        .unwrap_or(false))
        }),
        // browser session/cookie store read.
        "read_browser_session" => any(&|e| {
            e.signal == Signal::ReadSecret
                && e.join_key
                    .as_deref()
                    .map(is_browser_session_path)
                    .unwrap_or(false)
        }),
        _ => false,
    }
}

/// Heuristic value-free check: a git-write command shape (`git push/commit/tag/
/// remote set-url`). Operates on the reduced shape, never a raw command.
fn is_git_write_shape(shape: &str) -> bool {
    let s = shape.to_ascii_lowercase();
    s.starts_with("git ")
        && (s.contains(" push")
            || s.contains(" commit")
            || s.contains(" tag")
            || s.contains(" remote set-url")
            || s.contains(" remote add"))
}

/// Heuristic value-free check: a container-runtime command shape.
fn is_container_runtime_shape(shape: &str) -> bool {
    let s = shape.to_ascii_lowercase();
    (s.starts_with("docker ") || s.starts_with("podman "))
        && (s.contains(" run") || s.contains(" exec"))
}

/// Evaluate the toxic-combination catalog over a normalized event stream + the
/// set of baseline finding ids present at the §23.8(b) gate.
///
/// A rule activates iff (a) every `event_trigger` matched ≥1 event AND (b) the
/// `finding_triggers` are satisfied. `high_review_risk` has `None` triggers, so
/// clause (b) is vacuous (the pure review-control-gap exception).
pub fn evaluate(events: &[NormalizedEvent], present_finding_ids: &[String]) -> Vec<ToxicCombination> {
    let mut out = Vec::new();
    for rule in catalog() {
        // (a) every event trigger matched at least one normalized event.
        let events_ok = rule
            .event_triggers
            .iter()
            .all(|tag| event_trigger_matched(tag, events));
        if !events_ok {
            continue;
        }
        // (b) finding triggers satisfied.
        if !trigger_satisfied(rule.finding_triggers(), present_finding_ids) {
            continue;
        }

        // Value-free evidence: the rule's templated derived path + the present
        // required legs (real finding ids).
        let mut evidence = vec![rule.evidence_template.to_string()];
        for (fid, required) in walk_trigger(rule.finding_triggers(), present_finding_ids) {
            if required && present_finding_ids.contains(&fid) {
                evidence.push(format!("ambient leg present: {fid}"));
            }
        }
        out.push(ToxicCombination {
            name: rule.name.to_string(),
            severity: rule.severity,
            evidence,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_six_rules_and_accessors_work() {
        assert_eq!(catalog().len(), 6);
        let names: Vec<&str> = catalog().iter().map(|r| r.name).collect();
        assert!(names.contains(&"exfiltration_path"));
        assert!(names.contains(&"high_review_risk"));

        let r = rule_for("exfiltration_path").expect("rule present");
        assert_eq!(r.severity, RiskLevel::Critical);
        assert!(matches!(r.finding_triggers(), FindingTrigger::AllOf(_)));

        // high_review_risk is the None-trigger review-control-gap exception.
        let hr = rule_for("high_review_risk").expect("rule present");
        assert!(matches!(hr.finding_triggers(), FindingTrigger::None));

        assert!(rule_for("does_not_exist").is_none());
    }

    #[test]
    fn evaluate_gate_requires_both_events_and_findings() {
        use crate::session::normalize::normalize;
        use crate::session::trace::AgentEvent;

        // read_secret + egress events, with both ambient legs present → exfil fires.
        let norm = normalize(&[
            AgentEvent::FileRead { path: "~/.aws/credentials".into() },
            AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
        ]);
        let present = vec![
            "egress.connectivity".to_string(),
            "aws.credentials.profiles".to_string(),
        ];
        let combos = evaluate(&norm, &present);
        assert!(combos.iter().any(|c| c.name == "exfiltration_path"));

        // Same events but the credential leg absent → clause (b) fails, no path.
        let present_missing = vec!["egress.connectivity".to_string()];
        let combos = evaluate(&norm, &present_missing);
        assert!(!combos.iter().any(|c| c.name == "exfiltration_path"));

        // Findings present but no observed events → clause (a) fails, no path.
        let combos = evaluate(&[], &present);
        assert!(combos.is_empty(), "no observed action, no path");
    }

    #[test]
    fn high_review_risk_fires_on_unreviewed_sensitive_edit() {
        use crate::session::normalize::normalize;
        use crate::session::trace::AgentEvent;

        let norm = normalize(&[AgentEvent::FileWrite {
            path: "src/auth/login.rs".into(),
            diff: None,
        }]);
        // None-trigger rule: no ambient findings required.
        let combos = evaluate(&norm, &[]);
        assert!(combos.iter().any(|c| c.name == "high_review_risk"));

        // With a covering approval, absent_approval is false → does not fire.
        let norm = normalize(&[
            AgentEvent::FileWrite { path: "src/auth/login.rs".into(), diff: None },
            AgentEvent::Approval { approved_by: "u".into(), reason: None },
        ]);
        let combos = evaluate(&norm, &[]);
        assert!(!combos.iter().any(|c| c.name == "high_review_risk"));
    }
}
