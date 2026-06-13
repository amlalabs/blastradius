//! §23.2 / §23.5 — THE JOIN. Joins each [`NormalizedEvent`] against the
//! baseline of real `Finding`s and emits [`ActivatedCapability`] + [`Reason`].
//!
//! This is the heart of the product: a reachable finding that no event
//! activates stays in the denominator and does not score; only activated
//! (joined) findings enter the numerator. The classifier reuses §11 entirely
//! and adds no detection regex — every candidate finding id below is a **real**
//! id emitted by an existing probe (verified against `src/probes`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, FindingId};
use crate::session::normalize::{NormalizedEvent, Signal};
use crate::session::report::Reason;
use crate::severity::{Confidence, Severity};

/// One or more `Activates` edges collapsed into a named activated capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivatedCapability {
    /// e.g. "production deployment mutation possible".
    pub capability: String,
    /// observed events that activated it.
    pub event_ixs: Vec<usize>,
    /// the REAL ambient findings it joins against.
    pub finding_refs: Vec<FindingId>,
}

/// Result of the JOIN over a normalized event stream against a baseline.
#[derive(Debug, Clone, Default)]
pub struct Classification {
    pub activated: Vec<ActivatedCapability>,
    pub reasons: Vec<Reason>,
}

/// §23.8(b) presence gate: a finding counts as "present" when confidence ≥
/// `Likely` OR severity ≥ `Notable`.
pub fn finding_is_present(f: &Finding) -> bool {
    let conf_ok = matches!(f.confidence, Confidence::Confirmed | Confidence::Likely);
    let sev_ok = f.severity.rank() >= Severity::Notable.rank();
    conf_ok || sev_ok
}

/// Candidate ambient finding ids a signal may join against (§23.5 table). All
/// ids are real shipped probe ids.
fn candidate_ids(signal: Signal) -> &'static [&'static str] {
    match signal {
        Signal::ReadSecret => &[
            "aws.credentials.profiles",
            "ssh.private_keys",
            "git.credential_store",
            "cross_repo.dotenv",
            "env.secret_names",
            "github.token_source",
            "browser.session_stores",
        ],
        Signal::ModifiedProductionDeploy => &["git.push_likelihood"],
        Signal::ShellCommand => &[
            "process.sandbox_reach",
            "process.proc_environ",
            "process.cmdline_secrets",
            "process.memory_introspection",
            "process.afunix_docker_sock",
        ],
        Signal::NetworkAccess => &["egress.connectivity", "egress.mediation"],
        Signal::EditedAuthOrPaymentOrSecurityCode => &["git.push_likelihood"],
        Signal::DangerousShellPattern => &[
            "process.sandbox_reach",
            "process.proc_environ",
            "process.cmdline_secrets",
        ],
        Signal::ModifiedDependencyManifest => &[
            "cross_repo.dotenv",
            "cross_repo.sibling_repos",
            "cross_repo.lateral_secrets",
        ],
        Signal::ExternalMcpCall => &["egress.connectivity", "egress.mediation"],
        // modifier-only signal; no ambient join (§23.5).
        Signal::HumanApprovedRiskyAction => &[],
    }
}

/// Base weight for a signal (§23.6), used to populate `Reason.weight` so the
/// decomposed score is auditable at the JOIN.
fn signal_weight(signal: Signal) -> i32 {
    use crate::session::score::weights;
    match signal {
        Signal::ReadSecret => weights::READ_SECRET,
        Signal::ModifiedProductionDeploy => weights::MODIFIED_PRODUCTION_DEPLOY,
        Signal::ShellCommand => weights::SHELL_COMMAND,
        Signal::NetworkAccess => weights::NETWORK_ACCESS,
        Signal::EditedAuthOrPaymentOrSecurityCode => weights::EDITED_AUTH_PAYMENT_SECURITY,
        Signal::DangerousShellPattern => weights::DANGEROUS_SHELL_PATTERN,
        Signal::ModifiedDependencyManifest => weights::MODIFIED_DEPENDENCY_MANIFEST,
        Signal::ExternalMcpCall => weights::EXTERNAL_MCP_CALL,
        Signal::HumanApprovedRiskyAction => weights::HUMAN_APPROVED_RISKY_ACTION,
    }
}

/// A value-free, human capability label for a joined signal.
fn capability_label(signal: Signal) -> &'static str {
    match signal {
        Signal::ReadSecret => "credential store read against reachable secrets",
        Signal::ModifiedProductionDeploy => "production deployment mutation possible",
        Signal::ShellCommand => "shell execution within reachable process surface",
        Signal::NetworkAccess => "outbound network egress reachable",
        Signal::EditedAuthOrPaymentOrSecurityCode => "sensitive-code edit reaches push surface",
        Signal::DangerousShellPattern => "dangerous shell pattern within reachable surface",
        Signal::ModifiedDependencyManifest => "dependency manifest mutation reaches cross-repo surface",
        Signal::ExternalMcpCall => "external MCP call over reachable egress",
        Signal::HumanApprovedRiskyAction => "human-approved risky action",
    }
}

/// The signal name string used in `Reason.signal` (matches the §23.7 keys).
pub fn signal_name(signal: Signal) -> &'static str {
    match signal {
        Signal::ReadSecret => "read_secret",
        Signal::ModifiedProductionDeploy => "modified_production_deploy",
        Signal::ShellCommand => "shell_command",
        Signal::NetworkAccess => "network_access",
        Signal::EditedAuthOrPaymentOrSecurityCode => "edited_auth_payment_security_code",
        Signal::DangerousShellPattern => "dangerous_shell_pattern",
        Signal::ModifiedDependencyManifest => "modified_dependency_manifest",
        Signal::ExternalMcpCall => "external_mcp_call",
        Signal::HumanApprovedRiskyAction => "human_approved_risky_action",
    }
}

/// Join normalized events against the baseline findings (§23.2/§23.5).
///
/// A signal fires (contributes a `Reason`) only when at least one of its §23.5
/// candidate finding ids is **present** in the baseline at the §23.8(b) gate.
/// `human_approved_risky_action` is a modifier with no ambient anchor, so it
/// fires whenever an `Approval` was observed (it always reduces the score).
pub fn classify(events: &[NormalizedEvent], baseline: &[Finding]) -> Classification {
    // Index baseline by id, collapsing duplicate ids to the highest severity.
    let mut index: BTreeMap<&str, &Finding> = BTreeMap::new();
    for f in baseline {
        index
            .entry(f.id.as_str())
            .and_modify(|cur| {
                if f.severity.rank() > cur.severity.rank() {
                    *cur = f;
                }
            })
            .or_insert(f);
    }

    let mut reasons: Vec<Reason> = Vec::new();
    // capability label -> (event_ixs set, finding_refs set), preserving order.
    let mut caps: Vec<ActivatedCapability> = Vec::new();

    let mut record_cap = |signal: Signal, ix: usize, fid: Option<&str>| {
        let label = capability_label(signal).to_string();
        if let Some(c) = caps.iter_mut().find(|c| c.capability == label) {
            if !c.event_ixs.contains(&ix) {
                c.event_ixs.push(ix);
            }
            if let Some(fid) = fid {
                if !c.finding_refs.iter().any(|r| r == fid) {
                    c.finding_refs.push(fid.to_string());
                }
            }
        } else {
            caps.push(ActivatedCapability {
                capability: label,
                event_ixs: vec![ix],
                finding_refs: fid.map(|f| vec![f.to_string()]).unwrap_or_default(),
            });
        }
    };

    for ev in events {
        let signal = ev.signal;

        // Modifier-only signal: no ambient join, always fires (negative weight).
        if signal == Signal::HumanApprovedRiskyAction {
            reasons.push(Reason {
                signal: signal_name(signal).to_string(),
                weight: signal_weight(signal),
                evidence: vec!["a covering approval was recorded for the session".to_string()],
                finding_ref: None,
            });
            record_cap(signal, ev.event_ix, None);
            continue;
        }

        // Find the first present candidate finding (deterministic by table order).
        let joined: Option<&Finding> = candidate_ids(signal)
            .iter()
            .filter_map(|id| index.get(*id).copied())
            .find(|f| finding_is_present(f));

        if let Some(f) = joined {
            let mut evidence =
                vec![format!("{} joins ambient finding {}", signal_name(signal), f.id)];
            if let Some(jk) = &ev.join_key {
                evidence.push(format!("observed shape: {jk}"));
            }
            reasons.push(Reason {
                signal: signal_name(signal).to_string(),
                weight: signal_weight(signal),
                evidence,
                finding_ref: Some(f.id.clone()),
            });
            record_cap(signal, ev.event_ix, Some(f.id.as_str()));
        }
        // No present candidate → the signal stays in the denominator; it does
        // not score (the whole point of the JOIN).
    }

    Classification {
        activated: caps,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{FindingClass, FindingScope};
    use crate::session::normalize::normalize;
    use crate::session::trace::AgentEvent;

    fn f(id: &str, class: FindingClass, scope: FindingScope, sev: Severity) -> Finding {
        Finding::new(id, class, scope, id, sev, Confidence::Likely)
    }

    fn full_baseline() -> Vec<Finding> {
        vec![
            f("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient, Severity::Exposed),
            f("git.push_likelihood", FindingClass::GitWrite, FindingScope::Ambient, Severity::Exposed),
            f("egress.connectivity", FindingClass::Egress, FindingScope::Network, Severity::Exposed),
            f("process.sandbox_reach", FindingClass::Process, FindingScope::Host, Severity::Notable),
        ]
    }

    #[test]
    fn join_fires_only_with_present_finding() {
        let events = normalize(&[AgentEvent::FileRead {
            path: "~/.aws/credentials".into(),
        }]);
        // No baseline → no join, no reason.
        let c = classify(&events, &[]);
        assert!(c.reasons.is_empty());

        // With the credential finding present → read_secret fires with the real ref.
        let c = classify(&events, &full_baseline());
        let r = c.reasons.iter().find(|r| r.signal == "read_secret").unwrap();
        assert_eq!(r.finding_ref.as_deref(), Some("aws.credentials.profiles"));
    }

    #[test]
    fn info_only_finding_does_not_satisfy_gate() {
        let events = normalize(&[AgentEvent::NetworkAccess {
            host: "evil.example".into(),
            port: 443,
        }]);
        let baseline = vec![Finding::new(
            "egress.connectivity",
            FindingClass::Egress,
            FindingScope::Network,
            "egress",
            Severity::Info,
            Confidence::Possible,
        )];
        assert!(classify(&events, &baseline).reasons.is_empty());
    }
}
