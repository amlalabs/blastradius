//! Attack-scenario analysis (opt-in, `dashboard --ai`).
//!
//! Turns the **value-free** finding inventory into a defender-oriented blast-radius
//! narrative using the OpenAI API: given which credential stores, identities, and
//! egress routes are reachable, what could an attacker chain, what's the impact,
//! and how is it contained? This is the same "what would contain this" framing the
//! tool already ships — illustrated for the specific machine.
//!
//! ## Privacy contract
//!
//! This is the ONE feature that sends anything off-machine, and it is opt-in. It
//! transmits ONLY the value-free inventory (finding ids, classes, severities,
//! titles, summaries) — the exact same metadata the local report already prints,
//! and which carries NO secret values by construction (§4.2). Before the request
//! leaves, [`redaction_guard`] re-asserts that the serialized payload contains no
//! secret-shaped string; if it somehow did, the send aborts.

pub mod openai;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::finding::{Finding, FindingClass};
use crate::report::redaction::contains_secret_shaped;
use crate::severity::Severity;

/// A single reachable surface, distilled value-free for the model.
#[derive(Debug, Clone, Serialize)]
pub struct ReachableSurface {
    pub id: String,
    pub class: String,
    pub severity: String,
    pub confidence: String,
    pub title: String,
    pub summary: String,
}

/// The value-free exposure profile sent to the model.
#[derive(Debug, Clone, Serialize)]
pub struct ExposureProfile {
    pub platform: String,
    pub sandboxed: Option<String>,
    pub reachable: Vec<ReachableSurface>,
}

/// One AI-generated attack scenario (defender-oriented).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttackScenario {
    pub title: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub narrative: String,
    #[serde(default)]
    pub attack_path: Vec<String>,
    #[serde(default)]
    pub reachable_used: Vec<String>,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub containment: Vec<String>,
}

/// The full AI analysis result.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Analysis {
    pub model: String,
    pub overall: String,
    pub scenarios: Vec<AttackScenario>,
}

/// Build the value-free profile from a context's findings. Includes everything
/// at `Notable`/`Exposed` (the reachable surface) plus the sandbox verdict.
pub fn profile_from_findings(platform: &str, findings: &[Finding]) -> ExposureProfile {
    let mut reachable = Vec::new();
    let mut sandboxed = None;

    for f in findings {
        if f.id == "process.sandbox_detect" {
            sandboxed = f
                .evidence
                .get("verdict")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
        // Reachable surface = anything above Info; Info is "no exposure / context".
        if f.severity.rank() >= Severity::Notable.rank() {
            reachable.push(ReachableSurface {
                id: f.id.clone(),
                class: f.class.to_string(),
                severity: f.severity.label().to_string(),
                confidence: f.confidence.label().to_string(),
                title: f.title.clone(),
                summary: f.summary.clone(),
            });
        }
    }

    // Highest-severity, then credential-class first, for prompt salience.
    reachable.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(class_rank(&a.class).cmp(&class_rank(&b.class)))
    });

    ExposureProfile {
        platform: platform.to_string(),
        sandboxed,
        reachable,
    }
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "exposed" => 2,
        "notable" => 1,
        _ => 0,
    }
}

fn class_rank(s: &str) -> u8 {
    match s {
        "Credentials" => 0,
        "CrossRepo" => 1,
        "GitWrite" => 2,
        "Egress" => 3,
        "Process" => 4,
        "HostPersistence" => 5,
        _ => 6,
    }
}

/// Defense-in-depth: refuse to transmit a payload that contains a secret shape.
/// The profile is value-free by construction; this guards against a regression.
pub fn redaction_guard(payload: &str) -> Result<()> {
    if contains_secret_shaped(payload) {
        bail!("refusing to send AI payload: a secret-shaped string was detected in the value-free profile (this is a Layer-1 bug)");
    }
    Ok(())
}

const SYSTEM_PROMPT: &str = r#"You are a defensive security analyst helping a developer understand the BLAST RADIUS of running a coding agent on their own machine. You are given a value-free inventory of which credential stores, identities, repositories, and egress routes are REACHABLE by code running as this user (no secret values — only what is reachable).

Produce a concise, realistic, defender-oriented analysis of how an attacker who achieved code execution as this user (e.g. via prompt injection or a malicious dependency pulled in by the agent) could chain these reachable assets, and how to contain it.

IMPORTANT CONSTRAINTS:
- This is for the machine owner's own defensive awareness.
- Describe attack PATHS and IMPACT at a conceptual level. Do NOT output working exploit code, specific commands, payloads, or step-by-step operational instructions.
- Ground every scenario ONLY in surfaces present in the provided inventory; do not invent reachable assets.
- Always pair each scenario with concrete containment.

Respond with STRICT JSON of the form:
{"overall": "<one-paragraph blast-radius assessment>",
 "scenarios": [{"title": "<short name>", "severity": "critical|high|medium|low",
   "narrative": "<2-4 sentences, conceptual>",
   "attack_path": ["<conceptual stage>", "..."],
   "reachable_used": ["<finding title it relies on>", "..."],
   "impact": "<business/security impact>",
   "containment": ["<specific mitigation>", "..."]}]}
Return 3-6 scenarios ordered by severity. JSON only."#;

/// Run the AI analysis. `api_key` is used only as the bearer token.
pub fn analyze(profile: &ExposureProfile, api_key: &str, model: &str) -> Result<Analysis> {
    let payload = serde_json::to_string_pretty(profile)?;
    redaction_guard(&payload)?;

    let user = format!(
        "Reachable-surface inventory for this host (value-free):\n\n{payload}\n\n\
         Analyze the blast radius per the instructions and return JSON only."
    );

    let content = openai::chat_json(api_key, model, SYSTEM_PROMPT, &user)?;
    parse_analysis(&content, model)
}

/// Tolerant parse of the model's JSON into an `Analysis`.
fn parse_analysis(content: &str, model: &str) -> Result<Analysis> {
    let v: serde_json::Value = serde_json::from_str(content.trim())
        .map_err(|e| anyhow::anyhow!("model did not return valid JSON: {e}"))?;

    let overall = v
        .get("overall")
        .and_then(|o| o.as_str())
        .unwrap_or("")
        .to_string();

    let scenarios = v
        .get("scenarios")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| serde_json::from_value::<AttackScenario>(s.clone()).ok())
                .filter(|s| !s.title.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(Analysis {
        model: model.to_string(),
        overall,
        scenarios,
    })
}

/// Count of reachable surfaces by class — used for the dashboard summary tiles.
pub fn class_counts(findings: &[Finding]) -> Vec<(FindingClass, usize)> {
    let mut counts: Vec<(FindingClass, usize)> = Vec::new();
    for f in findings {
        if f.severity.rank() < Severity::Notable.rank() {
            continue;
        }
        if let Some(e) = counts.iter_mut().find(|(c, _)| *c == f.class) {
            e.1 += 1;
        } else {
            counts.push((f.class, 1));
        }
    }
    counts.sort_by_key(|(c, _)| c.order());
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_guard_blocks_secret_shapes() {
        assert!(redaction_guard("clean inventory: 3 profiles").is_ok());
        assert!(redaction_guard("oops ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345").is_err());
    }

    #[test]
    fn parse_handles_model_json() {
        let content = r#"{"overall":"broad","scenarios":[
            {"title":"Cloud pivot","severity":"high","narrative":"n","attack_path":["a","b"],
             "reachable_used":["AWS credentials reachable"],"impact":"i","containment":["c"]}]}"#;
        let a = parse_analysis(content, "gpt-4o-mini").unwrap();
        assert_eq!(a.scenarios.len(), 1);
        assert_eq!(a.scenarios[0].title, "Cloud pivot");
        assert_eq!(a.scenarios[0].attack_path.len(), 2);
    }

    #[test]
    fn profile_includes_only_reachable() {
        use crate::finding::{Finding, FindingClass, FindingScope};
        use crate::severity::{Confidence, Severity};
        let findings = vec![
            Finding::new("a", FindingClass::Credentials, FindingScope::Ambient, "reachable cred", Severity::Exposed, Confidence::Confirmed),
            Finding::new("b", FindingClass::Credentials, FindingScope::Ambient, "nothing", Severity::Info, Confidence::Confirmed),
        ];
        let p = profile_from_findings("Linux", &findings);
        assert_eq!(p.reachable.len(), 1);
        assert_eq!(p.reachable[0].title, "reachable cred");
    }
}
