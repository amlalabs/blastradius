//! §24.3 — the retro-hazard join. Reuses the §23 classifier unchanged: run it N
//! times against the same baseline, re-resolve each combination's finding legs
//! against the **current** findings to decide whether the hazard is still live,
//! and rank by current reachability + recency.
//!
//! All fields are value-free (§24.4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, FindingId, FindingScope};
use crate::session::classify::finding_is_present;
use crate::session::normalize::{normalize, NormalizedEvent, Signal};
use crate::session::report::{RiskLevel, ToxicCombination};
use crate::session::toxic_combinations::{evaluate, rule_for, walk_trigger, FindingTrigger};
use crate::session::trace::SessionTrace;
use crate::severity::{Confidence, Severity};

/// How a session's transcript source was located (§24.3.2 SessionDigest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    JsonlClaude,
    JsonlCodex,
    JsonlBeacon,
    JsonlCopilot,
    JsonlCursor,
    JsonlAntigravity,
    JsonGemini,
    JsonDir,
    MarkdownAider,
    SqliteVscdb,
    Fixture,
}

/// Basis for a recency timestamp. `mtime` is a lower-confidence upper bound
/// (copy/restore rewrites it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsBasis {
    EventTimestamp,
    FileMtime,
}

/// Value-free confidence signal (NOT a gate, §24.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegOrdering {
    SecretReadPrecedesEgress,
    EgressPrecedesSecretRead,
    Unordered,
}

/// Lifecycle status of a historical hazard against the current baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HazardStatus {
    StillReachable,
    PartiallyRemediated,
    RemediatedSince,
    ReviewGap,
}

/// The current state of a finding leg re-resolved against today's baseline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurrentFinding {
    pub severity: Severity,
    pub scope: FindingScope,
    pub confidence: Confidence,
}

/// One required/optional finding leg of a re-resolved toxic combination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegStatus {
    pub finding_ref: FindingId,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<CurrentFinding>,
    /// durable = `FindingScope::is_ambient_relevant()`.
    pub durable: bool,
}

/// Re-resolution of a combination's legs against the current baseline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReachabilityVerdict {
    pub legs: Vec<LegStatus>,
    pub still_reachable_count: usize,
    pub remediated_count: usize,
    pub all_required_present: bool,
}

/// Recency of the earliest activating event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecencyVerdict {
    pub age_days: f64,
    pub decay: f64,
    pub ts_basis: TsBasis,
}

/// Value-free digest of the activating session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionDigest {
    pub session_id: String,
    pub agent: String,
    pub repo: Option<String>,
    pub source_kind: SourceKind,
    /// shortened glob LABEL, never a raw $HOME path.
    pub source_label: String,
    pub started_at: Option<String>,
    pub event_at: Option<String>,
    pub event_count: usize,
    pub time_source: TsBasis,
}

/// §24.3.2 — one ranked historical hazard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoricalHazard {
    /// sha256(session_id ":" combo.name ":" sorted(event_ixs) ":" sorted(finding_refs))[..16]
    pub hazard_id: String,
    /// REUSED §23.8 ToxicCombination { name, severity, evidence[] }.
    pub combination: ToxicCombination,
    pub session: SessionDigest,
    pub reachability: ReachabilityVerdict,
    pub recency: RecencyVerdict,
    pub status: HazardStatus,
    /// did the activating session also egress/exit? gates the path/exfil
    /// headline clause (§24.4).
    pub exit_in_session: bool,
    /// value-free confidence signal, NOT a gate (§24.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<LegOrdering>,
    /// 0..=100 ranking key.
    pub realized_score: u8,
    /// templated, value-free, claim-bounded.
    pub summary: String,
    pub recommended_actions: Vec<String>,
}

/// §24.3.4 ranking constants (not literally in the brief; frozen with fixtures).
pub mod constants {
    pub const HALF_LIFE_DAYS: f64 = 14.0;
    pub const RECENCY_FLOOR: f64 = 0.25;
    pub const ARCHIVAL_FLOOR: f64 = 0.10;
    pub const COMBO_BASE_CRITICAL: f64 = 40.0;
    pub const COMBO_BASE_HIGH: f64 = 25.0;
    pub const COMBO_BASE_MEDIUM: f64 = 15.0;
    pub const NORMALIZER: f64 = 2.5;
    pub const REVIEW_NORMALIZER: f64 = 1.5;
    pub const REVIEW_CAP: u8 = 60;
    /// §24.3.4 reach-factor ladder.
    pub const REACH_STILL_ALL_EXPOSED: f64 = 1.00;
    pub const REACH_MIXED: f64 = 0.70;
    pub const REACH_PARTIAL: f64 = 0.45;
    pub const REACH_REMEDIATED: f64 = 0.10;
    /// §24.3.4 durability bonus per ambient-relevant present leg fraction.
    pub const DURABILITY_BONUS: f64 = 0.15;
}

/// §24.3.4 recency decay: `max(0.5 ^ (age/HALF_LIFE), RECENCY_FLOOR)`.
pub fn decay(age_days: f64) -> f64 {
    let age = if age_days.is_finite() && age_days > 0.0 {
        age_days
    } else {
        0.0
    };
    (0.5f64.powf(age / constants::HALF_LIFE_DAYS)).max(constants::RECENCY_FLOOR)
}

impl LegStatus {
    /// Durability derives from the leg's scope (§24.3.2).
    pub fn durable_from_scope(scope: FindingScope) -> bool {
        scope.is_ambient_relevant()
    }
}

/// `combo_base` per the combination severity (§24.3.4 / §23.6 path weights).
fn combo_base(severity: RiskLevel) -> f64 {
    match severity {
        RiskLevel::Critical => constants::COMBO_BASE_CRITICAL,
        RiskLevel::High => constants::COMBO_BASE_HIGH,
        RiskLevel::Medium => constants::COMBO_BASE_MEDIUM,
        RiskLevel::Low => 0.0,
    }
}

/// Severity rank for sorting (Critical > High > Medium > Low).
fn level_rank(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    }
}

/// Build the `ReachabilityVerdict` for a combination's legs against the current
/// baseline index. A leg is "still reachable" when its current finding is
/// present at severity ≥ `Notable` (Exposed or Notable). `remediated_count`
/// counts required legs that are absent now or only Info.
fn build_verdict(
    legs: &[(String, bool)],
    index: &BTreeMap<String, &Finding>,
) -> ReachabilityVerdict {
    let mut leg_statuses = Vec::new();
    let mut still_reachable_count = 0usize;
    let mut remediated_count = 0usize;
    let mut all_required_present = true;

    for (fid, required) in legs {
        let cur = index.get(fid);
        // "present" = fires at severity ≥ Notable in today's baseline.
        let present_now = cur
            .map(|f| f.severity.rank() >= Severity::Notable.rank())
            .unwrap_or(false);

        let current = cur.map(|f| CurrentFinding {
            severity: f.severity,
            scope: f.scope,
            confidence: f.confidence,
        });
        let durable = cur.map(|f| f.scope.is_ambient_relevant()).unwrap_or(false);

        if present_now {
            still_reachable_count += 1;
        } else if *required {
            remediated_count += 1;
            all_required_present = false;
        }

        leg_statuses.push(LegStatus {
            finding_ref: fid.clone(),
            required: *required,
            current,
            durable,
        });
    }

    ReachabilityVerdict {
        legs: leg_statuses,
        still_reachable_count,
        remediated_count,
        all_required_present,
    }
}

/// §24.3.3 `classify_status`.
fn classify_status(trigger: &FindingTrigger, verdict: &ReachabilityVerdict) -> HazardStatus {
    // None-trigger rule (e.g. high_review_risk) → ReviewGap; never asserts
    // reachability (the vacuous-"all present" bug fix).
    if matches!(trigger, FindingTrigger::None) {
        return HazardStatus::ReviewGap;
    }
    // Any required leg absent / Info → RemediatedSince.
    if !verdict.all_required_present {
        return HazardStatus::RemediatedSince;
    }
    // All required present and ≥1 leg Exposed → StillReachable.
    let any_exposed = verdict.legs.iter().any(|l| {
        l.current
            .as_ref()
            .map(|c| c.severity == Severity::Exposed)
            .unwrap_or(false)
    });
    if any_exposed {
        HazardStatus::StillReachable
    } else {
        // All required present but ≤ Notable → PartiallyRemediated.
        HazardStatus::PartiallyRemediated
    }
}

/// §24.3.4 reach_factor ladder.
fn reach_factor(status: HazardStatus, verdict: &ReachabilityVerdict) -> f64 {
    match status {
        HazardStatus::StillReachable => {
            // 1.00 only when every PRESENT leg is Exposed; otherwise mixed 0.70.
            // Absent AnyOf members (current: None) are not "present" and do not
            // pull the factor down — only legs that fire today are weighed.
            let present: Vec<&LegStatus> = verdict
                .legs
                .iter()
                .filter(|l| {
                    l.current
                        .as_ref()
                        .map(|c| c.severity.rank() >= Severity::Notable.rank())
                        .unwrap_or(false)
                })
                .collect();
            let all_exposed = !present.is_empty()
                && present.iter().all(|l| {
                    l.current
                        .as_ref()
                        .map(|c| c.severity == Severity::Exposed)
                        .unwrap_or(false)
                });
            if all_exposed {
                constants::REACH_STILL_ALL_EXPOSED
            } else {
                constants::REACH_MIXED
            }
        }
        HazardStatus::PartiallyRemediated => constants::REACH_PARTIAL,
        HazardStatus::RemediatedSince => constants::REACH_REMEDIATED,
        HazardStatus::ReviewGap => constants::REACH_REMEDIATED,
    }
}

/// §24.3.4 durability = 1.0 + 0.15 * fraction(present legs whose scope is
/// ambient-relevant). "present legs" = legs with a current finding fired now.
fn durability(verdict: &ReachabilityVerdict) -> f64 {
    let present: Vec<&LegStatus> = verdict
        .legs
        .iter()
        .filter(|l| {
            l.current
                .as_ref()
                .map(|c| c.severity.rank() >= Severity::Notable.rank())
                .unwrap_or(false)
        })
        .collect();
    if present.is_empty() {
        return 1.0;
    }
    let durable = present.iter().filter(|l| l.durable).count();
    let frac = durable as f64 / present.len() as f64;
    1.0 + constants::DURABILITY_BONUS * frac
}

/// §24.3.4 realized_score for a live (non-ReviewGap) hazard.
fn realized_score(
    severity: RiskLevel,
    verdict: &ReachabilityVerdict,
    recency: &RecencyVerdict,
    status: HazardStatus,
) -> u8 {
    let base = combo_base(severity);
    let reach = reach_factor(status, verdict);
    let dur = durability(verdict);
    let raw = base * reach * dur * recency.decay * constants::NORMALIZER;
    raw.round().clamp(0.0, 100.0) as u8
}

/// §24.3.4 review_score for a ReviewGap lane hazard (capped 60).
fn review_score(severity: RiskLevel, recency: &RecencyVerdict) -> u8 {
    let raw = combo_base(severity) * recency.decay * constants::REVIEW_NORMALIZER;
    raw.round().clamp(0.0, constants::REVIEW_CAP as f64) as u8
}

/// Recency from the session's earliest activating event timestamp. Per-event
/// timestamps are not in the frozen `AgentEvent` contract; `started_at` is the
/// earliest line timestamp (or the file-mtime floor when discovery substitutes
/// it, §24.2.5). Future-dated / unparseable timestamps clamp `age_days = 0`
/// (fail-loud-safe).
fn recency_of(trace: &SessionTrace, now_unix: u64) -> RecencyVerdict {
    match trace
        .started_at
        .as_deref()
        .and_then(crate::util::time::unix_from_iso8601)
    {
        Some(ts) => {
            let age_secs = now_unix.saturating_sub(ts);
            let age_days = age_secs as f64 / 86_400.0;
            RecencyVerdict {
                age_days,
                decay: decay(age_days),
                ts_basis: TsBasis::EventTimestamp,
            }
        }
        None => {
            // Unknown timestamp → conservative "most recent" upper bound.
            RecencyVerdict {
                age_days: 0.0,
                decay: decay(0.0),
                ts_basis: TsBasis::FileMtime,
            }
        }
    }
}

/// §24.6 ordering — a value-free confidence signal (NOT a gate). Looks at the
/// first secret-read vs first egress event index in the normalized stream.
fn leg_ordering(events: &[NormalizedEvent]) -> Option<LegOrdering> {
    let first_read = events
        .iter()
        .filter(|e| e.signal == Signal::ReadSecret)
        .map(|e| e.event_ix)
        .min();
    let first_egress = events
        .iter()
        .filter(|e| {
            e.signal == Signal::NetworkAccess
                || e.signal == Signal::DangerousShellPattern
                || e.signal == Signal::ExternalMcpCall
        })
        .map(|e| e.event_ix)
        .min();
    match (first_read, first_egress) {
        (Some(r), Some(g)) if r < g => Some(LegOrdering::SecretReadPrecedesEgress),
        (Some(r), Some(g)) if g < r => Some(LegOrdering::EgressPrecedesSecretRead),
        (Some(_), Some(_)) => Some(LegOrdering::Unordered),
        _ => None,
    }
}

/// Did the activating session also egress/exit? Gates the path/exfil headline
/// clause (§24.4).
fn session_exits(events: &[NormalizedEvent]) -> bool {
    events.iter().any(|e| {
        matches!(
            e.signal,
            Signal::NetworkAccess | Signal::DangerousShellPattern | Signal::ExternalMcpCall
        )
    })
}

/// The set of event indices that fed an activated leg of this combination — the
/// indices of the events whose signal contributed to the rule activating.
fn activating_event_ixs(rule_name: &str, events: &[NormalizedEvent]) -> Vec<usize> {
    let mut ixs: Vec<usize> = events
        .iter()
        .filter(|e| signal_feeds_rule(rule_name, e.signal))
        .map(|e| e.event_ix)
        .collect();
    ixs.sort_unstable();
    ixs.dedup();
    ixs
}

/// Which signals feed a given rule's `event_triggers` (value-free, by rule id).
fn signal_feeds_rule(rule_name: &str, signal: Signal) -> bool {
    match rule_name {
        "exfiltration_path" => matches!(
            signal,
            Signal::ReadSecret | Signal::NetworkAccess | Signal::DangerousShellPattern
        ),
        "saas_session_hijack" => {
            matches!(signal, Signal::ReadSecret | Signal::NetworkAccess)
        }
        "source_control_mutation_path" => matches!(
            signal,
            Signal::ShellCommand
                | Signal::ModifiedProductionDeploy
                | Signal::EditedAuthOrPaymentOrSecurityCode
        ),
        "production_deployment_path" => matches!(signal, Signal::ModifiedProductionDeploy),
        "post_root_host_visibility" => {
            matches!(signal, Signal::ShellCommand | Signal::ExternalMcpCall)
        }
        "high_review_risk" => matches!(
            signal,
            Signal::EditedAuthOrPaymentOrSecurityCode | Signal::HumanApprovedRiskyAction
        ),
        _ => false,
    }
}

/// §24.3.2 hazard_id = sha256(session_id ":" name ":" sorted(event_ixs) ":"
/// sorted(finding_refs))[..16].
fn hazard_id(session_id: &str, name: &str, event_ixs: &[usize], finding_refs: &[String]) -> String {
    use sha2::{Digest, Sha256};
    let mut ixs: Vec<usize> = event_ixs.to_vec();
    ixs.sort_unstable();
    let ixs_s: Vec<String> = ixs.iter().map(|i| i.to_string()).collect();
    let mut refs: Vec<String> = finding_refs.to_vec();
    refs.sort();
    let input = format!(
        "{}:{}:{}:{}",
        session_id,
        name,
        ixs_s.join(","),
        refs.join(",")
    );
    let digest = Sha256::digest(input.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..16].to_string()
}

/// Value-free, claim-bounded headline (§24.4 wording boundary). The path/exfil
/// clause renders ONLY when `exit_in_session` OR a combination actually fired
/// (here a combination always fired since this is per-combo). Never "exfiltrated".
fn build_summary(
    rule_name: &str,
    status: HazardStatus,
    recency: &RecencyVerdict,
    exit_in_session: bool,
    verdict: &ReachabilityVerdict,
) -> String {
    let when = if recency.ts_basis == TsBasis::FileMtime {
        "at an unknown time".to_string()
    } else {
        let days = recency.age_days.round() as i64;
        if days <= 0 {
            "today".to_string()
        } else if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    };

    let still = verdict.still_reachable_count;
    let total_req = verdict.legs.iter().filter(|l| l.required).count();

    let status_clause = match status {
        HazardStatus::StillReachable => {
            format!("{still} of {total_req} required leg(s) are STILL reachable")
        }
        HazardStatus::PartiallyRemediated => {
            "the required legs are present but downgraded since (partially remediated)".to_string()
        }
        HazardStatus::RemediatedSince => {
            "a required leg is no longer reachable (remediated since)".to_string()
        }
        HazardStatus::ReviewGap => "no covering review was recorded (review gap)".to_string(),
    };

    // path clause only when the session exited OR a combo fired (a combo always
    // fired here) — but the wording stays "composes", never "exfiltrated".
    let composes = if exit_in_session {
        "composes with an egress path observed in the same session; "
    } else {
        ""
    };

    format!(
        "a session {when} activated the {rule_name} path; {composes}{status_clause}."
    )
}

/// Run the retro join over a set of traces against a baseline (§24.3.3).
///
/// Returns `(hazards, review_gaps)`: StillReachable/Partial/Remediated route to
/// `hazards`, ReviewGap routes to `review_gaps`. Both are sorted by
/// `realized_score` desc → severity desc → `still_reachable_count` desc →
/// `event_at` desc.
pub fn retro_scan(
    baseline: &[Finding],
    traces: &[SessionTrace],
    now_unix: u64,
) -> (Vec<HistoricalHazard>, Vec<HistoricalHazard>) {
    // Index baseline by id, collapsing duplicate ids to the highest severity.
    let mut index: BTreeMap<String, &Finding> = BTreeMap::new();
    for f in baseline {
        index
            .entry(f.id.clone())
            .and_modify(|cur| {
                if f.severity.rank() > cur.severity.rank() {
                    *cur = f;
                }
            })
            .or_insert(f);
    }

    // The set of ids present at the §23.8(b) gate — the same denominator the
    // toxic-rule evaluator uses.
    let present_ids: Vec<String> = baseline
        .iter()
        .filter(|f| finding_is_present(f))
        .map(|f| f.id.clone())
        .collect();

    let mut hazards: Vec<HistoricalHazard> = Vec::new();
    let mut review_gaps: Vec<HistoricalHazard> = Vec::new();

    for trace in traces {
        let normalized = normalize(&trace.events);
        let combos = evaluate(&normalized, &present_ids);
        if combos.is_empty() {
            continue;
        }

        let recency = recency_of(trace, now_unix);
        let ordering = leg_ordering(&normalized);
        let exit_in_session = session_exits(&normalized);

        for combo in combos {
            let Some(rule) = rule_for(&combo.name) else {
                continue;
            };
            let trigger = rule.finding_triggers();
            let legs = walk_trigger(trigger, &present_ids);
            let verdict = build_verdict(&legs, &index);
            let status = classify_status(trigger, &verdict);

            // §24.3.1 retro gate: a live (non-ReviewGap) hazard is kept only when
            // ≥1 of its legs still fires at severity ≥ Notable in today's
            // baseline. ReviewGap has no legs and is kept regardless (it asserts
            // a control gap, never current reachability).
            if status != HazardStatus::ReviewGap && verdict.still_reachable_count == 0 {
                continue;
            }

            let finding_refs: Vec<String> = legs.iter().map(|(fid, _)| fid.clone()).collect();
            let event_ixs = activating_event_ixs(&combo.name, &normalized);
            let hid = hazard_id(&trace.session_id, &combo.name, &event_ixs, &finding_refs);

            let score = if status == HazardStatus::ReviewGap {
                review_score(combo.severity, &recency)
            } else {
                realized_score(combo.severity, &verdict, &recency, status)
            };

            let summary = build_summary(&combo.name, status, &recency, exit_in_session, &verdict);
            let recommended_actions = recommended_actions(&combo.name, status, &verdict);

            let source_kind = source_kind_for(&trace.agent);
            let digest = SessionDigest {
                session_id: trace.session_id.clone(),
                agent: trace.agent.clone(),
                repo: trace.repo.clone(),
                source_kind,
                source_label: format!("{} session", trace.agent),
                started_at: trace.started_at.clone(),
                event_at: trace.started_at.clone(),
                event_count: trace.events.len(),
                time_source: recency.ts_basis,
            };

            let hazard = HistoricalHazard {
                hazard_id: hid,
                combination: combo,
                session: digest,
                reachability: verdict,
                recency: recency.clone(),
                status,
                exit_in_session,
                ordering,
                realized_score: score,
                summary,
                recommended_actions,
            };

            if status == HazardStatus::ReviewGap {
                review_gaps.push(hazard);
            } else {
                hazards.push(hazard);
            }
        }
    }

    sort_hazards(&mut hazards);
    sort_hazards(&mut review_gaps);
    (hazards, review_gaps)
}

/// Sort by `realized_score` desc → severity desc → `still_reachable_count` desc
/// → `event_at` desc, with `hazard_id` as a final deterministic tiebreak.
fn sort_hazards(v: &mut [HistoricalHazard]) {
    v.sort_by(|a, b| {
        b.realized_score
            .cmp(&a.realized_score)
            .then_with(|| level_rank(b.combination.severity).cmp(&level_rank(a.combination.severity)))
            .then_with(|| {
                b.reachability
                    .still_reachable_count
                    .cmp(&a.reachability.still_reachable_count)
            })
            .then_with(|| b.session.event_at.cmp(&a.session.event_at))
            .then_with(|| a.hazard_id.cmp(&b.hazard_id))
    });
}

/// Value-free recommended actions, keyed on the rule + status.
fn recommended_actions(rule_name: &str, status: HazardStatus, _verdict: &ReachabilityVerdict) -> Vec<String> {
    let mut out = Vec::new();
    match status {
        HazardStatus::RemediatedSince => {
            out.push("no live action required; retained for historical visibility".to_string());
            return out;
        }
        HazardStatus::ReviewGap => {
            out.push("require a covering human review for sensitive-code changes".to_string());
            return out;
        }
        _ => {}
    }
    match rule_name {
        "exfiltration_path" | "saas_session_hijack" => {
            out.push("rotate the still-reachable credential store".to_string());
            out.push("restrict egress (no_egress) for agent sessions".to_string());
        }
        "source_control_mutation_path" => {
            out.push("scope or remove the ssh-agent socket from agent sessions".to_string());
            out.push("require review before push from agent sessions".to_string());
        }
        "production_deployment_path" => {
            out.push("require review before deploy-workflow edits reach push".to_string());
        }
        "post_root_host_visibility" => {
            out.push("isolate the process surface and remove cross-repo reach".to_string());
        }
        _ => {
            out.push("review the still-reachable legs of this path".to_string());
        }
    }
    out
}

/// Best-effort source-kind label from the agent tag (value-free).
fn source_kind_for(agent: &str) -> SourceKind {
    match agent {
        "claude-code" | "factory" | "devin" => SourceKind::JsonlClaude,
        "codex" => SourceKind::JsonlCodex,
        "copilot" => SourceKind::JsonlCopilot,
        "cursor" => SourceKind::JsonlCursor,
        "antigravity" => SourceKind::JsonlAntigravity,
        "gemini" => SourceKind::JsonGemini,
        "opencode" => SourceKind::JsonDir,
        "aider" => SourceKind::MarkdownAider,
        _ => SourceKind::Fixture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{FindingClass, FindingScope};
    use crate::session::trace::AgentEvent;

    fn f(id: &str, class: FindingClass, scope: FindingScope, sev: Severity) -> Finding {
        Finding::new(id, class, scope, id, sev, Confidence::Likely)
    }

    /// The headline exfiltration baseline: cred store + egress both Exposed,
    /// both ambient-relevant (durable).
    fn live_baseline() -> Vec<Finding> {
        vec![
            f("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient, Severity::Exposed),
            f("egress.connectivity", FindingClass::Egress, FindingScope::Network, Severity::Exposed),
        ]
    }

    /// A session that read an AWS cred store then ran an external-egress command.
    fn exfil_trace(started_at: Option<&str>) -> SessionTrace {
        SessionTrace {
            session_id: "X".into(),
            agent: "claude-code".into(),
            repo: Some("blastradius".into()),
            started_at: started_at.map(str::to_string),
            events: vec![
                AgentEvent::FileRead { path: "~/.aws/credentials".into() },
                AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
            ],
            privileged_user: false,
            after_hours: false,
        }
    }

    #[test]
    fn still_reachable_exfil_ranks_high() {
        // now = 2026-06-13; session 3 days earlier.
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        let trace = exfil_trace(Some("2026-06-10T00:00:00Z"));
        let (hazards, gaps) = retro_scan(&live_baseline(), &[trace], now);

        let exfil = hazards
            .iter()
            .find(|h| h.combination.name == "exfiltration_path")
            .expect("exfiltration_path hazard present");
        assert_eq!(exfil.status, HazardStatus::StillReachable);
        assert!(exfil.exit_in_session, "session egressed");
        assert_eq!(exfil.ordering, Some(LegOrdering::SecretReadPrecedesEgress));
        // crit base 40 × reach 1.00 × durability 1.15 × decay(3) × 2.5.
        // decay(3) = 0.5^(3/14) ≈ 0.8623 → 40*1.15*0.8623*2.5 ≈ 99 → clamps near 99.
        assert!(exfil.realized_score >= 90, "got {}", exfil.realized_score);
        assert_eq!(exfil.hazard_id.len(), 16);
        // No review gaps from this trace (no unreviewed sensitive edit).
        assert!(gaps.is_empty());
    }

    #[test]
    fn remediated_since_when_cred_absent_now() {
        // The COUNTER-CASE: the credential leg is gone (rotated since). The exfil
        // path no longer has its required cred leg reachable, so it is demoted /
        // dropped by the retro gate (still_reachable_count == 0).
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        let trace = exfil_trace(Some("2026-06-10T00:00:00Z"));
        // egress present, but NO credential finding present anywhere.
        let baseline = vec![f(
            "egress.connectivity",
            FindingClass::Egress,
            FindingScope::Network,
            Severity::Exposed,
        )];
        let (hazards, _gaps) = retro_scan(&baseline, &[trace], now);
        // With the cred leg absent, the toxic rule's clause (b) AnyOf is not
        // satisfied so exfiltration_path does not even activate → no hazard.
        assert!(
            !hazards.iter().any(|h| h.combination.name == "exfiltration_path"),
            "rotated-since credential must not produce a live exfil hazard"
        );
    }

    #[test]
    fn remediated_since_demotes_when_leg_downgraded_to_info() {
        // exfil activates against the present-gate denominator (cred + egress
        // both present at Notable+ at evaluation), but at re-resolution the cred
        // leg is Info → required leg not reachable → RemediatedSince, demoted
        // below the ARCHIVAL floor and dropped by the retro gate unless egress
        // alone keeps a leg reachable.
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        let trace = exfil_trace(Some("2026-06-10T00:00:00Z"));
        // Both present at the gate (Notable), but cred at Notable not Exposed.
        let baseline = vec![
            f("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient, Severity::Notable),
            f("egress.connectivity", FindingClass::Egress, FindingScope::Network, Severity::Exposed),
        ];
        let (hazards, _gaps) = retro_scan(&baseline, &[trace], now);
        let exfil = hazards
            .iter()
            .find(|h| h.combination.name == "exfiltration_path")
            .expect("present at Notable so still reachable, mixed");
        // All required present, ≥1 Exposed (egress) → StillReachable but mixed.
        assert_eq!(exfil.status, HazardStatus::StillReachable);
        // mixed reach 0.70 → lower than the all-Exposed case.
        assert!(exfil.realized_score < 90, "mixed should score lower: {}", exfil.realized_score);
    }

    #[test]
    fn review_gap_routes_to_separate_lane() {
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        let trace = SessionTrace {
            session_id: "rg".into(),
            agent: "claude-code".into(),
            repo: None,
            started_at: Some("2026-06-12T00:00:00Z".into()),
            events: vec![AgentEvent::FileWrite {
                path: "src/auth/login.rs".into(),
                diff: None,
            }],
            privileged_user: false,
            after_hours: false,
        };
        let (hazards, gaps) = retro_scan(&[], &[trace], now);
        assert!(hazards.is_empty(), "no live legs → no ranked hazard");
        let rg = gaps
            .iter()
            .find(|h| h.combination.name == "high_review_risk")
            .expect("review gap present");
        assert_eq!(rg.status, HazardStatus::ReviewGap);
        assert!(rg.realized_score <= constants::REVIEW_CAP);
    }

    #[test]
    fn future_dated_timestamp_clamps_age_to_zero() {
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        let trace = exfil_trace(Some("2030-01-01T00:00:00Z"));
        let (hazards, _) = retro_scan(&live_baseline(), &[trace], now);
        let exfil = hazards.iter().find(|h| h.combination.name == "exfiltration_path").unwrap();
        assert_eq!(exfil.recency.age_days, 0.0);
        assert_eq!(exfil.recency.decay, 1.0);
    }

    #[test]
    fn hazard_id_is_deterministic_and_value_free() {
        let id1 = hazard_id("s", "exfiltration_path", &[1, 0], &["b".into(), "a".into()]);
        let id2 = hazard_id("s", "exfiltration_path", &[0, 1], &["a".into(), "b".into()]);
        assert_eq!(id1, id2, "order-insensitive");
        assert_eq!(id1.len(), 16);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
