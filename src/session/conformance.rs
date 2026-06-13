//! Seam C — Rust-native conformance harness for the toxic-combination rule
//! pack, porting beacon's `conformance.go` / `lint.go` disciplines (embedded
//! fixtures per rule, a maturity ladder, a per-fixture conformance gate)
//! **without** any shared YAML, CEL, or new external dependency.
//!
//! Where beacon's fixtures carry only events (beacon has no JOIN), a
//! blastradius [`RuleFixture`] carries **both** raw `AgentEvent`s **and** a
//! baseline `Finding` set, because the toxic-gate's clause (b) joins observed
//! signals against the live ambient baseline (`classify::finding_is_present`).
//!
//! Value-free discipline (§23.11, Seam C2): fixtures hold raw `AgentEvent` test
//! input only as `#[cfg(test)]` harness data. They flow through `normalize`
//! (Layer-1) before evaluation and are **never rendered**. A fixture that
//! escapes into rendered output would itself be a defect.
//!
//! Conformance gate discipline (Seam C1): the `pack_conformance` test iterates
//! every fixture and asserts each `FixtureResult::ok()` **individually** —
//! explicitly avoiding the upstream beacon `InstallFiles` bug
//! (`detect.go:192`), which checks only the aggregate error and silently
//! installs rules whose embedded fixtures fail.

#![cfg(test)]

use crate::finding::Finding;
use crate::session::classify::finding_is_present;
use crate::session::normalize::normalize;
use crate::session::toxic_combinations::{self, Status, ToxicCombinationRule};
use crate::session::trace::AgentEvent;

/// Two-state fixture verdict, mirroring beacon's `match | no_match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// the rule MUST appear in the produced combination set.
    Match,
    /// the rule MUST NOT appear in the produced combination set.
    NoMatch,
}

/// An embedded conformance fixture for one rule. Carries raw `AgentEvent` test
/// input + a baseline `Finding` set (the JOIN leg) + the expected produced rule
/// names.
pub struct RuleFixture {
    /// human-readable fixture name (for failure messages).
    pub name: &'static str,
    /// whether the target rule is expected to fire.
    pub verdict: Verdict,
    /// raw observed agent actions — normalized before evaluation, never rendered.
    pub events: Vec<AgentEvent>,
    /// ambient baseline the JOIN's clause (b) resolves against.
    pub baseline: Vec<Finding>,
    /// the toxic-combination rule names this fixture expects to be produced.
    /// For a `Match` fixture this contains the rule under test; for a `NoMatch`
    /// fixture it is empty (the rule must not appear).
    pub expect_rules: Vec<&'static str>,
}

/// Outcome of running one fixture through the engine.
pub struct FixtureResult {
    pub name: &'static str,
    pub verdict: Verdict,
    pub expected: Vec<&'static str>,
    pub produced: Vec<String>,
}

impl FixtureResult {
    /// A fixture passes iff the produced combination-name set equals the
    /// expected set (order-independent). For `NoMatch` the expected set is
    /// empty, so the rule under test must be absent.
    pub fn ok(&self) -> bool {
        let mut expected: Vec<&str> = self.expected.clone();
        expected.sort_unstable();
        expected.dedup();
        let mut produced: Vec<&str> = self.produced.iter().map(|s| s.as_str()).collect();
        produced.sort_unstable();
        produced.dedup();
        expected == produced
    }
}

/// Run one fixture through `normalize` + `toxic_combinations::evaluate` and
/// capture the produced combination names (Seam C, ported from
/// `conformance.go::CheckRule`).
pub fn check_rule(fixture: &RuleFixture) -> FixtureResult {
    let normalized = normalize(&fixture.events);
    // The §23.8(b) present-id universe, derived exactly as the live engine does.
    let present_ids: Vec<String> = fixture
        .baseline
        .iter()
        .filter(|f| finding_is_present(f))
        .map(|f| f.id.clone())
        .collect();
    let combos = toxic_combinations::evaluate(&normalized, &present_ids);
    FixtureResult {
        name: fixture.name,
        verdict: fixture.verdict,
        expected: fixture.expect_rules.clone(),
        produced: combos.into_iter().map(|c| c.name).collect(),
    }
}

/// Maturity-ladder gate, ported from `lint.go::CheckMaturity`:
/// - `Experimental` requires ≥1 fixture;
/// - `Stable` requires ≥1 `Match` **and** ≥1 `NoMatch` fixture;
/// - `Deprecated` relaxes fixtures.
///
/// Returns `Ok(())` when the rule's fixtures satisfy its declared maturity.
pub fn check_maturity(rule: &ToxicCombinationRule, fixtures: &[RuleFixture]) -> Result<(), String> {
    match rule.status {
        Status::Experimental => {
            if fixtures.is_empty() {
                return Err(format!(
                    "rule {} is Experimental but has no fixtures",
                    rule.name
                ));
            }
        }
        Status::Stable => {
            let has_match = fixtures.iter().any(|f| f.verdict == Verdict::Match);
            let has_no_match = fixtures.iter().any(|f| f.verdict == Verdict::NoMatch);
            if !(has_match && has_no_match) {
                return Err(format!(
                    "rule {} is Stable but lacks ≥1 Match AND ≥1 NoMatch fixture",
                    rule.name
                ));
            }
        }
        Status::Deprecated => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixtures — authored fresh per rule, guided by the existing `#[cfg(test)]`
// cases in `toxic_combinations.rs` and `classify.rs`. All six rules ship
// `Experimental`, so the `Stable` gate need not trip in this change.
// ---------------------------------------------------------------------------

use crate::finding::{FindingClass, FindingScope};
use crate::severity::{Confidence, Severity};

/// Helper: a present-by-construction baseline finding (Likely + Notable clears
/// the §23.8(b) gate), mirroring `classify.rs`'s test helper.
fn present_finding(id: &str, class: FindingClass, scope: FindingScope) -> Finding {
    Finding::new(id, class, scope, id, Severity::Exposed, Confidence::Likely)
}

/// Fixtures for one named rule. Every catalog rule must return ≥1 fixture.
fn fixtures_for(rule_name: &str) -> Vec<RuleFixture> {
    match rule_name {
        "exfiltration_path" => vec![
            RuleFixture {
                name: "exfil: aws creds read + egress with both ambient legs",
                verdict: Verdict::Match,
                events: vec![
                    AgentEvent::FileRead { path: "~/.aws/credentials".into() },
                    AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
                ],
                baseline: vec![
                    present_finding("egress.connectivity", FindingClass::Egress, FindingScope::Network),
                    present_finding("aws.credentials.profiles", FindingClass::Credentials, FindingScope::Ambient),
                ],
                expect_rules: vec!["exfiltration_path"],
            },
            RuleFixture {
                name: "exfil: credential leg absent → clause (b) fails",
                verdict: Verdict::NoMatch,
                events: vec![
                    AgentEvent::FileRead { path: "~/.aws/credentials".into() },
                    AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
                ],
                baseline: vec![present_finding(
                    "egress.connectivity",
                    FindingClass::Egress,
                    FindingScope::Network,
                )],
                expect_rules: vec![],
            },
        ],
        "source_control_mutation_path" => vec![
            RuleFixture {
                name: "scm: git push with armed ssh-agent + push reach",
                verdict: Verdict::Match,
                events: vec![AgentEvent::ShellCommand { command: "git push origin main".into() }],
                baseline: vec![
                    present_finding("ssh.agent_socket", FindingClass::Credentials, FindingScope::Ambient),
                    present_finding("git.push_likelihood", FindingClass::GitWrite, FindingScope::Ambient),
                ],
                expect_rules: vec!["source_control_mutation_path"],
            },
            RuleFixture {
                name: "scm: git push but ssh-agent leg absent",
                verdict: Verdict::NoMatch,
                events: vec![AgentEvent::ShellCommand { command: "git push origin main".into() }],
                baseline: vec![present_finding(
                    "git.push_likelihood",
                    FindingClass::GitWrite,
                    FindingScope::Ambient,
                )],
                expect_rules: vec![],
            },
        ],
        "post_root_host_visibility" => vec![
            RuleFixture {
                name: "post-root: docker run + escalation + cross-repo reach",
                verdict: Verdict::Match,
                events: vec![AgentEvent::ShellCommand {
                    command: "docker run --rm -it ubuntu bash".into(),
                }],
                baseline: vec![
                    present_finding("host.privilege_escalation", FindingClass::SystemInfo, FindingScope::Host),
                    present_finding("cross_repo.sibling_repos", FindingClass::CrossRepo, FindingScope::SiblingRepos),
                ],
                expect_rules: vec!["post_root_host_visibility"],
            },
            RuleFixture {
                name: "post-root: docker run but no cross-repo reach",
                verdict: Verdict::NoMatch,
                events: vec![AgentEvent::ShellCommand {
                    command: "docker run --rm -it ubuntu bash".into(),
                }],
                baseline: vec![present_finding(
                    "host.privilege_escalation",
                    FindingClass::SystemInfo,
                    FindingScope::Host,
                )],
                expect_rules: vec![],
            },
        ],
        "saas_session_hijack" => vec![
            RuleFixture {
                name: "saas: browser cookie store read + egress",
                verdict: Verdict::Match,
                events: vec![
                    AgentEvent::FileRead {
                        path: "~/.config/google-chrome/Default/Cookies".into(),
                    },
                    AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
                ],
                baseline: vec![
                    present_finding("browser.session_stores", FindingClass::Credentials, FindingScope::Ambient),
                    present_finding("egress.connectivity", FindingClass::Egress, FindingScope::Network),
                ],
                // A browser session-store read with open egress legitimately
                // composes BOTH paths: exfiltration_path (browser.session_stores
                // is in its AnyOf credential leg) and saas_session_hijack.
                expect_rules: vec!["exfiltration_path", "saas_session_hijack"],
            },
            RuleFixture {
                name: "saas: browser read + egress but session-store leg absent",
                verdict: Verdict::NoMatch,
                events: vec![
                    AgentEvent::FileRead {
                        path: "~/.config/google-chrome/Default/Cookies".into(),
                    },
                    AgentEvent::NetworkAccess { host: "evil.example".into(), port: 443 },
                ],
                baseline: vec![present_finding(
                    "egress.connectivity",
                    FindingClass::Egress,
                    FindingScope::Network,
                )],
                expect_rules: vec![],
            },
        ],
        "production_deployment_path" => vec![
            RuleFixture {
                name: "deploy: workflow edit + push reach",
                verdict: Verdict::Match,
                events: vec![AgentEvent::FileWrite {
                    path: ".github/workflows/deploy.yml".into(),
                    diff: None,
                }],
                baseline: vec![present_finding(
                    "git.push_likelihood",
                    FindingClass::GitWrite,
                    FindingScope::Ambient,
                )],
                expect_rules: vec!["production_deployment_path"],
            },
            RuleFixture {
                name: "deploy: workflow edit but no push reach",
                verdict: Verdict::NoMatch,
                events: vec![AgentEvent::FileWrite {
                    path: ".github/workflows/deploy.yml".into(),
                    diff: None,
                }],
                baseline: vec![],
                expect_rules: vec![],
            },
        ],
        "high_review_risk" => vec![
            RuleFixture {
                name: "review-gap: sensitive edit with no covering approval",
                verdict: Verdict::Match,
                events: vec![AgentEvent::FileWrite {
                    path: "src/auth/login.rs".into(),
                    diff: None,
                }],
                baseline: vec![],
                expect_rules: vec!["high_review_risk"],
            },
            RuleFixture {
                name: "review-gap: sensitive edit WITH a covering approval",
                verdict: Verdict::NoMatch,
                events: vec![
                    AgentEvent::FileWrite { path: "src/auth/login.rs".into(), diff: None },
                    AgentEvent::Approval { approved_by: "user".into(), reason: None },
                ],
                baseline: vec![],
                expect_rules: vec![],
            },
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seam C / C1: the per-rule conformance gate. Iterates the live catalog,
    /// asserts every rule has ≥1 fixture, runs **each** fixture through the
    /// engine, and asserts **each** `FixtureResult::ok()` INDIVIDUALLY — never
    /// only a rule-level aggregate (explicitly avoiding the beacon
    /// `InstallFiles` bug, §1.4).
    #[test]
    fn pack_conformance() {
        for rule in toxic_combinations::catalog() {
            let fixtures = fixtures_for(rule.name);
            assert!(
                !fixtures.is_empty(),
                "rule {} has no conformance fixtures",
                rule.name
            );

            // Maturity ladder must be satisfied for the declared status.
            check_maturity(rule, &fixtures)
                .unwrap_or_else(|e| panic!("maturity gate failed: {e}"));

            // Per-fixture verdict — assert EACH result.ok() individually.
            for fixture in &fixtures {
                let result = check_rule(fixture);
                assert!(
                    result.ok(),
                    "fixture '{}' for rule '{}' failed: expected {:?}, produced {:?}",
                    result.name,
                    rule.name,
                    result.expected,
                    result.produced,
                );
                // Verdict semantics: Match ⇒ rule present; NoMatch ⇒ rule absent.
                let present = result.produced.iter().any(|n| n == rule.name);
                match fixture.verdict {
                    Verdict::Match => assert!(
                        present,
                        "Match fixture '{}' did not produce rule '{}'",
                        result.name, rule.name
                    ),
                    Verdict::NoMatch => assert!(
                        !present,
                        "NoMatch fixture '{}' unexpectedly produced rule '{}'",
                        result.name, rule.name
                    ),
                }
            }
        }
    }

    /// Seam C4 — SOFT impact-coverage gate over the authored `impact.rs` layer.
    ///
    /// Asserts every session `Signal` name resolves to a non-empty
    /// `signal_impact`, and that the per-`FindingClass` fallback covers all
    /// seven variants. We deliberately do **NOT** assert a bespoke
    /// `finding_impact` arm for every emitted id (today `cross_repo.dotenv`,
    /// `git.config_exec_directives`, and `claude_code.project_tool_surface` rely
    /// on the class fallback — a strict gate would go RED), and we do **NOT**
    /// source ids from `classify::candidate_ids` (it contains a phantom
    /// `process.sandbox_reach` no probe emits).
    #[test]
    fn impact_coverage_soft_gate() {
        use crate::dashboard::impact::{finding_impact_class, signal_impact};

        // All 9 session Signal names must have a non-empty `signal_impact`.
        const SIGNAL_NAMES: &[&str] = &[
            "read_secret",
            "modified_production_deploy",
            "shell_command",
            "network_access",
            "edited_auth_payment_security_code",
            "dangerous_shell_pattern",
            "modified_dependency_manifest",
            "external_mcp_call",
            "human_approved_risky_action",
        ];
        for name in SIGNAL_NAMES {
            let why = signal_impact(name)
                .unwrap_or_else(|| panic!("signal_impact missing for '{name}'"));
            assert!(!why.is_empty(), "signal_impact empty for '{name}'");
        }

        // The class fallback must cover all 7 FindingClass variants with
        // non-empty (why, how) — the safety net the soft id-gate relies on.
        const FINDING_CLASSES: &[&str] = &[
            "Credentials",
            "CrossRepo",
            "GitWrite",
            "Egress",
            "Process",
            "HostPersistence",
            "SystemInfo",
        ];
        for class in FINDING_CLASSES {
            let (why, how) = finding_impact_class(class);
            assert!(!why.is_empty(), "class fallback why empty for '{class}'");
            assert!(!how.is_empty(), "class fallback how empty for '{class}'");
        }
    }
}
