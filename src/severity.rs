//! Severity and confidence — reported independently (§7.3, §7.4).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// How exposed a finding is. Ordered so `Exposed > Notable > Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Context, not risk (no siblings, no egress, not a git repo, no `.env`).
    Info,
    /// Reachable; impact is context-dependent.
    Notable,
    /// A same-user process can reach something likely sensitive.
    Exposed,
}

impl Severity {
    pub fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Notable => 1,
            Severity::Exposed => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Notable => "notable",
            Severity::Exposed => "exposed",
        }
    }

    /// Parse a `--fail-on` threshold value.
    pub fn parse_threshold(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "info" => Some(Severity::Info),
            "notable" => Some(Severity::Notable),
            "exposed" => Some(Severity::Exposed),
            _ => None,
        }
    }
}

impl Ord for Severity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Inferred-capability confidence, reported separately from severity (§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Confirmed,
    Likely,
    Possible,
    Unknown,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Confirmed => "confirmed",
            Confidence::Likely => "likely",
            Confidence::Possible => "possible",
            Confidence::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
