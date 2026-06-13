//! Normalization + side-by-side diff (§13). Compares ambient metrics, which are
//! identical by construction; `CurrentRepo` deltas are demoted to a footnote.

use serde::Serialize;
use std::collections::BTreeSet;

use crate::finding::{Finding, FindingScope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value")]
pub enum ComparableValue {
    Bool(bool),
    Count(u64),
    StringSet(BTreeSet<String>),
    Status(String),
}

impl ComparableValue {
    pub fn display(&self) -> String {
        match self {
            ComparableValue::Bool(b) => {
                if *b {
                    "yes".into()
                } else {
                    "no".into()
                }
            }
            ComparableValue::Count(n) => n.to_string(),
            ComparableValue::StringSet(s) => s.iter().cloned().collect::<Vec<_>>().join(", "),
            ComparableValue::Status(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingSummary {
    pub metric: String,
    pub scope: FindingScope,
    pub value: ComparableValue,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonRow {
    pub metric: String,
    pub ambient: bool,
    pub left: ComparableValue,
    pub right: ComparableValue,
    pub equal: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    /// True iff every ambient-relevant row is equal across contexts.
    pub ambient_unchanged: bool,
    pub rows: Vec<ComparisonRow>,
}

impl Comparison {
    pub fn ambient_rows(&self) -> impl Iterator<Item = &ComparisonRow> {
        self.rows.iter().filter(|r| r.ambient)
    }
    pub fn current_repo_rows(&self) -> impl Iterator<Item = &ComparisonRow> {
        self.rows.iter().filter(|r| !r.ambient)
    }
}

/// Pull a number from a finding's evidence by key.
fn ev_u64(f: &Finding, key: &str) -> u64 {
    f.evidence.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// Extract the normalized comparable metrics from one context's findings (§13).
pub fn normalize(findings: &[Finding]) -> Vec<FindingSummary> {
    let mut out = Vec::new();
    let find = |id: &str| findings.iter().find(|f| f.id == id);

    if let Some(f) = find("aws.credentials.profiles") {
        out.push(FindingSummary {
            metric: "AWS profiles".into(),
            scope: FindingScope::Ambient,
            value: ComparableValue::Count(ev_u64(f, "profile_count")),
        });
    }
    if let Some(f) = find("ssh.private_keys") {
        out.push(FindingSummary {
            metric: "SSH private keys".into(),
            scope: FindingScope::Ambient,
            value: ComparableValue::Count(ev_u64(f, "key_count")),
        });
    }
    if let Some(f) = find("env.secret_names") {
        out.push(FindingSummary {
            metric: "secret-like env vars".into(),
            scope: FindingScope::Ambient,
            value: ComparableValue::Count(ev_u64(f, "count")),
        });
    }
    if let Some(f) = find("github.token_source") {
        let present = f
            .evidence
            .get("hosts")
            .and_then(|h| h.as_array())
            .map(|a| {
                a.iter()
                    .any(|h| h.get("token_present").and_then(|v| v.as_bool()) == Some(true))
            })
            .unwrap_or(false);
        out.push(FindingSummary {
            metric: "GitHub auth source".into(),
            scope: FindingScope::Ambient,
            value: ComparableValue::Status(if present {
                "present".into()
            } else {
                "absent".into()
            }),
        });
    }
    if let Some(f) = find("git.credential_store") {
        let n = f
            .evidence
            .get("stored_hosts")
            .and_then(|h| h.as_array())
            .map(|a| a.len() as u64)
            .unwrap_or(0);
        out.push(FindingSummary {
            metric: "git credential hosts".into(),
            scope: FindingScope::Ambient,
            value: ComparableValue::Count(n),
        });
    }
    if let Some(f) = find("cross_repo.sibling_repos") {
        out.push(FindingSummary {
            metric: "sibling repos readable".into(),
            scope: FindingScope::SiblingRepos,
            value: ComparableValue::Count(ev_u64(f, "count")),
        });
    }
    if let Some(f) = find("cross_repo.lateral_secrets") {
        out.push(FindingSummary {
            metric: "sibling repos with secrets".into(),
            scope: FindingScope::SiblingRepos,
            value: ComparableValue::Count(ev_u64(f, "repos_with_secret_like_files")),
        });
    }
    if let Some(f) = find("egress.connectivity") {
        let status = if f.evidence.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            "disabled"
        } else if f
            .evidence
            .get("tls_handshake")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            "open"
        } else {
            "blocked"
        };
        out.push(FindingSummary {
            metric: "outbound connectivity".into(),
            scope: FindingScope::Network,
            value: ComparableValue::Status(status.into()),
        });
    }
    // CurrentRepo footnote metric.
    if let Some(f) = find("cross_repo.dotenv.current") {
        out.push(FindingSummary {
            metric: "current-repo .env files".into(),
            scope: FindingScope::CurrentRepo,
            value: ComparableValue::Count(ev_u64(f, "file_count")),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{FindingClass, FindingScope};
    use crate::severity::{Confidence, Severity};
    use serde_json::json;

    #[test]
    fn disabled_egress_normalizes_as_disabled_not_blocked() {
        let f = Finding::new(
            "egress.connectivity",
            FindingClass::Egress,
            FindingScope::Network,
            "egress probe disabled",
            Severity::Info,
            Confidence::Confirmed,
        )
        .evidence(json!({ "enabled": false }));

        let rows = normalize(&[f]);
        let row = rows
            .iter()
            .find(|r| r.metric == "outbound connectivity")
            .expect("egress row");
        assert_eq!(row.value, ComparableValue::Status("disabled".into()));
    }
}

/// Build the comparison between a left (repo-root) and right (worktree) context.
pub fn compare(left: &[Finding], right: &[Finding]) -> Comparison {
    let left_n = normalize(left);
    let right_n = normalize(right);

    let mut rows = Vec::new();
    let mut ambient_unchanged = true;

    for ls in &left_n {
        let rs = right_n.iter().find(|r| r.metric == ls.metric);
        let right_val = rs.map(|r| r.value.clone()).unwrap_or(ls.value.clone());
        let equal = right_val == ls.value;
        let ambient = ls.scope.is_ambient_relevant();
        if ambient && !equal {
            ambient_unchanged = false;
        }
        rows.push(ComparisonRow {
            metric: ls.metric.clone(),
            ambient,
            left: ls.value.clone(),
            right: right_val,
            equal,
        });
    }

    Comparison {
        ambient_unchanged,
        rows,
    }
}
