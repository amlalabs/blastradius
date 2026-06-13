//! The `Finding` data model (§9.3). Probes return structured, redacted metadata only.

use crate::severity::{Confidence, Severity};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier for a finding, e.g. `aws.credentials.profiles`.
pub type FindingId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FindingClass {
    Credentials,
    CrossRepo,
    GitWrite,
    Egress,
    Process,
    HostPersistence,
    SystemInfo,
}

impl FindingClass {
    /// Display heading used to group the terminal/markdown sections (§14).
    pub fn section_title(self) -> &'static str {
        match self {
            FindingClass::Credentials => "CREDENTIALS",
            FindingClass::CrossRepo => "CROSS-REPO",
            FindingClass::GitWrite => "GIT WRITE",
            FindingClass::Egress => "EGRESS",
            FindingClass::Process => "PROCESS",
            FindingClass::HostPersistence => "HOST PERSISTENCE",
            FindingClass::SystemInfo => "SYSTEM INFO",
        }
    }

    /// Deterministic section ordering for rendering (§14).
    pub fn order(self) -> u8 {
        match self {
            FindingClass::Credentials => 0,
            FindingClass::CrossRepo => 1,
            FindingClass::GitWrite => 2,
            FindingClass::Egress => 3,
            FindingClass::Process => 4,
            FindingClass::HostPersistence => 5,
            FindingClass::SystemInfo => 6,
        }
    }
}

impl fmt::Display for FindingClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FindingClass::Credentials => "Credentials",
            FindingClass::CrossRepo => "CrossRepo",
            FindingClass::GitWrite => "GitWrite",
            FindingClass::Egress => "Egress",
            FindingClass::Process => "Process",
            FindingClass::HostPersistence => "HostPersistence",
            FindingClass::SystemInfo => "SystemInfo",
        };
        f.write_str(s)
    }
}

/// Where the finding lives — this is what makes `compare` honest (§9.3).
///
/// Blast-radius-relevant scopes — `Ambient`, `SiblingRepos`, `Network`, `Host` —
/// are expected to be identical across worktrees. `CurrentRepo` is *allowed to
/// differ* (untracked files won't exist in a HEAD worktree).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FindingScope {
    Ambient,
    CurrentRepo,
    SiblingRepos,
    Network,
    Host,
}

impl FindingScope {
    /// Whether this scope is part of the ambient-authority comparison (§13.1).
    /// `CurrentRepo` deltas are an expected footnote, not part of the punch.
    pub fn is_ambient_relevant(self) -> bool {
        !matches!(self, FindingScope::CurrentRepo)
    }
}

impl fmt::Display for FindingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FindingScope::Ambient => "Ambient",
            FindingScope::CurrentRepo => "CurrentRepo",
            FindingScope::SiblingRepos => "SiblingRepos",
            FindingScope::Network => "Network",
            FindingScope::Host => "Host",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub class: FindingClass,
    pub scope: FindingScope,
    pub title: String,
    pub summary: String,
    pub severity: Severity,
    pub confidence: Confidence,
    /// Structured, redacted evidence (counts / names / shortened paths only).
    pub evidence: serde_json::Value,
    pub remediation: Vec<String>,
}

impl Finding {
    /// Convenience constructor with empty remediation and `Null` evidence.
    pub fn new(
        id: impl Into<String>,
        class: FindingClass,
        scope: FindingScope,
        title: impl Into<String>,
        severity: Severity,
        confidence: Confidence,
    ) -> Finding {
        Finding {
            id: id.into(),
            class,
            scope,
            title: title.into(),
            summary: String::new(),
            severity,
            confidence,
            evidence: serde_json::Value::Null,
            remediation: Vec::new(),
        }
    }

    pub fn summary(mut self, s: impl Into<String>) -> Finding {
        self.summary = s.into();
        self
    }

    pub fn evidence(mut self, e: serde_json::Value) -> Finding {
        self.evidence = e;
        self
    }

    pub fn remediation(mut self, items: &[&str]) -> Finding {
        self.remediation = items.iter().map(|s| s.to_string()).collect();
        self
    }
}
