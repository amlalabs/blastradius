//! §24.1/§24.7 — AUTO-SLURP passive discovery of agent session transcripts.
//!
//! Discovery is read-only and value-free at the source: it globs well-known
//! on-disk locations (no hooks), parses each agent's native format via Layer-0
//! extractors, and normalizes to the frozen `SessionTrace` contract. A
//! detectable-but-unparsable source is reported as `DetectedUnparsed(reason)`,
//! never a silent gap (§24.1).

pub mod extract;
pub mod locate;
pub mod parse;
pub mod registry;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::session::discovery::locate::{expand_glob_with, source_label, GlobLimits, Root};
use crate::session::retro::SourceKind;
use crate::session::trace::SessionTrace;

/// Configuration for a discovery run (§24.5 flags resolved to a struct).
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Recency window in seconds; files older than this are skipped-and-counted.
    pub max_age_secs: Option<u64>,
    /// Agent tags to include; empty = all.
    pub agents: Vec<String>,
    /// Optional repo label filter.
    pub repo: Option<String>,
    /// Honor `ScanLimits` for byte caps / symlink policy (held by the caller).
    pub max_bytes_per_file: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            // §24.2.6 discovery default 90 days.
            max_age_secs: Some(90 * 86_400),
            agents: Vec::new(),
            repo: None,
            max_bytes_per_file: 50 * 1024 * 1024,
        }
    }
}

/// Why a detected source could not be parsed (§24.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceStatus {
    /// Successfully parsed into one or more `SessionTrace`s.
    Parsed,
    /// Detected on disk but not passively readable; carries an honest reason.
    DetectedUnparsed(String),
    /// Config dir exists but yielded zero parsed transcripts.
    ConfiguredButEmpty,
}

/// One discovered session source: a located transcript (or a detected-unparsed
/// marker), value-free.
#[derive(Debug, Clone)]
pub struct SessionSource {
    pub agent_tag: String,
    pub source_kind: SourceKind,
    /// shortened glob LABEL, never a raw $HOME path.
    pub source_label: String,
    pub status: SourceStatus,
    /// parsed traces (empty unless `status == Parsed`).
    pub traces: Vec<SessionTrace>,
}

/// A row in the discovery agent registry (§24.1 table).
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub agent_tag: &'static str,
    /// the root each `*-relative` path is resolved against (§24.1 Root enum).
    pub roots: &'static [crate::session::discovery::locate::Root],
    /// discovery marker (root-relative), e.g. ".claude/settings.json".
    pub discovery_marker: &'static str,
    /// transcript glob (root-relative), or "" when there is no fixed glob.
    pub transcript_glob: &'static str,
    pub source_kind: SourceKind,
    /// honest reason this source is detect-only (e.g. "sqlite feature off"),
    /// or "" when the source is parseable in MVP.
    pub unparsed_reason: &'static str,
}

/// The result of a discovery run: the located sources plus value-free
/// diagnostics so a silent-empty discovery is never read as safety (§24.1).
#[derive(Debug, Clone, Default)]
pub struct DiscoveryResult {
    pub sources: Vec<SessionSource>,
    /// Honest, value-free lines: "agent <x> configured but 0 transcripts parsed",
    /// recency-skip counts, byte-cap truncations.
    pub diagnostics: Vec<String>,
    /// Count of files skipped by the recency window (surfaced so absence is
    /// never read as safety).
    pub recency_skipped: usize,
}

/// Names of the directories that *would* be read for the configured agents — for
/// the opt-in first-run banner the handler prints BEFORE any read (§24.4).
/// Returns value-free root-relative markers (e.g. "~/.claude/projects/…"), never
/// a resolved absolute path.
pub fn banner_dirs(config: &DiscoveryConfig) -> Vec<String> {
    let mut out = Vec::new();
    for spec in registry::agents() {
        if !config.agents.is_empty() && !config.agents.iter().any(|a| a == spec.agent_tag) {
            continue;
        }
        if spec.transcript_glob.is_empty() {
            continue;
        }
        for root in spec.roots {
            out.push(format!("{} {}", root_prefix(*root), spec.transcript_glob));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A short, value-free prefix label for a root (never the resolved path).
fn root_prefix(root: Root) -> &'static str {
    match root {
        Root::Home => "~",
        Root::XdgConfig => "$XDG_CONFIG_HOME",
        Root::XdgData => "$XDG_DATA_HOME",
        Root::XdgState => "$XDG_STATE_HOME",
        Root::MacAppSupport => "~/Library/Application Support",
        Root::CurrentRepo => "<repo>",
    }
}

/// Discover sessions in the well-known passive locations. Read-only and
/// value-free at the source. Honors the recency window and per-file byte cap.
pub fn discover_sessions(config: &DiscoveryConfig) -> DiscoveryResult {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let limits = GlobLimits::default();

    let mut result = DiscoveryResult::default();

    for spec in registry::agents() {
        // Agent filter.
        if !config.agents.is_empty() && !config.agents.iter().any(|a| a == spec.agent_tag) {
            continue;
        }

        // Detect-only sources (SQLite off, no-glob, deferred) — honest status.
        if !spec.unparsed_reason.is_empty() || spec.transcript_glob.is_empty() {
            // Only report it if its config marker exists on disk (it "ran here").
            if marker_present(spec) {
                let reason = if spec.transcript_glob.is_empty() {
                    "no fixed glob; hook-supplied path only".to_string()
                } else {
                    spec.unparsed_reason.to_string()
                };
                result.sources.push(SessionSource {
                    agent_tag: spec.agent_tag.to_string(),
                    source_kind: spec.source_kind,
                    source_label: format!("{} (detected)", spec.agent_tag),
                    status: SourceStatus::DetectedUnparsed(reason),
                    traces: Vec::new(),
                });
            }
            continue;
        }

        // Parseable source: expand the glob across each candidate root.
        let mut files = Vec::new();
        for root in spec.roots {
            files.extend(expand_glob_with(*root, spec.transcript_glob, limits));
        }
        files.sort();
        files.dedup();

        let marker = marker_present(spec);
        if files.is_empty() {
            if marker {
                // Config dir exists but zero transcripts parsed (§24.1).
                result.diagnostics.push(format!(
                    "agent {} configured but 0 transcripts parsed",
                    spec.agent_tag
                ));
                result.sources.push(SessionSource {
                    agent_tag: spec.agent_tag.to_string(),
                    source_kind: spec.source_kind,
                    source_label: format!("{} (configured)", spec.agent_tag),
                    status: SourceStatus::ConfiguredButEmpty,
                    traces: Vec::new(),
                });
            }
            continue;
        }

        for file in files {
            // Recency window: skip-and-count older files (§24.2.6).
            if let Some(max_age) = config.max_age_secs {
                if let Some(mtime) = file_mtime_secs(&file) {
                    if now.saturating_sub(mtime) > max_age {
                        result.recency_skipped += 1;
                        continue;
                    }
                }
            }

            match read_and_parse(spec, &file, config.max_bytes_per_file) {
                Some((trace, label, truncated)) => {
                    if truncated {
                        result.diagnostics.push(format!(
                            "{}: transcript over byte cap; parsed truncated prefix",
                            label
                        ));
                    }
                    // Repo filter (label match only; never a join key).
                    if let Some(want) = &config.repo {
                        if trace.repo.as_deref() != Some(want.as_str()) {
                            continue;
                        }
                    }
                    result.sources.push(SessionSource {
                        agent_tag: spec.agent_tag.to_string(),
                        source_kind: spec.source_kind,
                        source_label: label,
                        status: SourceStatus::Parsed,
                        traces: vec![trace],
                    });
                }
                None => {
                    // Parse failure / format drift — honest, not silent.
                    let label = source_label(&file);
                    result.sources.push(SessionSource {
                        agent_tag: spec.agent_tag.to_string(),
                        source_kind: spec.source_kind,
                        source_label: label,
                        status: SourceStatus::DetectedUnparsed("format drift".to_string()),
                        traces: Vec::new(),
                    });
                }
            }
        }
    }

    if result.recency_skipped > 0 {
        result.diagnostics.push(format!(
            "{} transcript(s) skipped by recency window",
            result.recency_skipped
        ));
    }

    result
}

/// Does this agent's discovery marker exist under any of its roots? Resolves the
/// root transiently (never persisted).
fn marker_present(spec: &AgentSpec) -> bool {
    for root in spec.roots {
        if let Some(base) = root.resolve() {
            let candidate = base.join(spec.discovery_marker);
            if candidate.exists() {
                return true;
            }
        }
    }
    false
}

/// File mtime in unix seconds (read-only metadata; no follow).
fn file_mtime_secs(path: &Path) -> Option<u64> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Read a transcript (bounded by the byte cap), identify the agent by content
/// (path-primary, format-confirmed §24.2.1), and parse to a `SessionTrace`.
/// Returns `(trace, value_free_label, truncated)`.
fn read_and_parse(
    spec: &AgentSpec,
    path: &Path,
    max_bytes: u64,
) -> Option<(SessionTrace, String, bool)> {
    let (contents, truncated) = read_bounded(path, max_bytes)?;
    let label = source_label(path);
    let session_id = file_stem(path);

    let trace = identify_and_parse(spec, &session_id, &contents)?;
    // If the parser produced a trace with no events AND the file had content,
    // still keep it (an empty session is a valid, value-free fact).
    Some((trace, label, truncated))
}

/// Read at most `max_bytes` of a file (no symlink follow). Returns the content
/// and whether it was truncated.
fn read_bounded(path: &Path, max_bytes: u64) -> Option<(String, bool)> {
    use std::io::Read;
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        return None; // never follow a symlink transcript
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    let mut handle = (&mut f).take(max_bytes);
    handle.read_to_end(&mut buf).ok()?;
    let truncated = meta.len() > max_bytes;
    // Truncate at the last newline so we never parse a half line (and never lift
    // a partial value).
    let text = String::from_utf8_lossy(&buf);
    let trimmed = if truncated {
        match text.rfind('\n') {
            Some(ix) => text[..ix].to_string(),
            None => String::new(),
        }
    } else {
        text.into_owned()
    };
    Some((trimmed, truncated))
}

/// Path-primary, format-confirmed dispatch (§24.2.1): the glob implies the
/// agent's parser, but a copied/renamed Codex rollout in a Claude dir is
/// re-dispatched by content; an unrecognized shape downgrades to `None`
/// (→ `DetectedUnparsed("format drift")`).
fn identify_and_parse(spec: &AgentSpec, session_id: &str, contents: &str) -> Option<SessionTrace> {
    use parse::{jsonl_claude, jsonl_codex};
    match spec.source_kind {
        SourceKind::JsonlClaude => {
            // Re-dispatch a Codex rollout dropped into a Claude dir.
            if jsonl_codex::sniff(contents) {
                return jsonl_codex::parse(session_id, contents);
            }
            jsonl_claude::parse(session_id, spec.agent_tag, contents)
        }
        SourceKind::JsonlCodex => jsonl_codex::parse(session_id, contents),
        // Every other kind has no MVP parser; the caller already routed these to
        // DetectedUnparsed via `unparsed_reason`, so this is unreachable in
        // practice — return None to be safe.
        _ => None,
    }
}

/// File stem as a value-free session id (§24.2.5 `FileStem`/`RolloutFilename`).
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("session")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::trace::AgentEvent;
    use std::fs;

    #[test]
    fn banner_dirs_are_value_free_and_name_globs() {
        let cfg = DiscoveryConfig::default();
        let dirs = banner_dirs(&cfg);
        assert!(dirs.iter().any(|d| d.contains(".claude/projects")));
        assert!(dirs.iter().any(|d| d.contains("rollout-")));
        // never an absolute resolved path
        assert!(dirs.iter().all(|d| !d.starts_with('/')));
    }

    #[test]
    fn read_bounded_truncates_at_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.jsonl");
        fs::write(&p, "aaaa\nbbbb\ncccc\n").unwrap();
        let (text, truncated) = read_bounded(&p, 6).unwrap();
        assert!(truncated);
        assert_eq!(text, "aaaa");
    }

    #[test]
    fn discover_parses_a_claude_fixture_under_temp_home() {
        // Build a fake $HOME and point dirs at it via HOME env (resolve() reads
        // dirs::home_dir which honors $HOME on unix).
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let proj = home.join(".claude/projects/enc-cwd");
        fs::create_dir_all(&proj).unwrap();
        fs::write(home.join(".claude/settings.json"), "{}").unwrap();
        let jsonl = r#"{"type":"assistant","timestamp":"2026-06-12T09:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"~/.aws/credentials"}}]}}"#;
        fs::write(proj.join("3f2ae1.jsonl"), jsonl).unwrap();

        let _guard = EnvHomeGuard::set(home);

        let cfg = DiscoveryConfig {
            agents: vec!["claude-code".to_string()],
            ..DiscoveryConfig::default()
        };
        let res = discover_sessions(&cfg);
        let parsed: Vec<_> = res
            .sources
            .iter()
            .filter(|s| s.status == SourceStatus::Parsed)
            .collect();
        assert_eq!(parsed.len(), 1, "expected one parsed claude source");
        let trace = &parsed[0].traces[0];
        assert_eq!(trace.agent, "claude-code");
        assert!(trace
            .events
            .contains(&AgentEvent::FileRead { path: "~/.aws/credentials".into() }));
        // value-free label never leaks the temp home absolute path
        assert!(!parsed[0].source_label.contains(home.to_str().unwrap()));
    }

    /// Sets $HOME for the duration of a test and restores it on drop. Tests that
    /// mutate process env must not run concurrently with other env-mutating
    /// tests; this module's env test is the only one.
    struct EnvHomeGuard {
        prev: Option<std::ffi::OsString>,
    }
    impl EnvHomeGuard {
        fn set(home: &Path) -> Self {
            let prev = std::env::var_os("HOME");
            std::env::set_var("HOME", home);
            EnvHomeGuard { prev }
        }
    }
    impl Drop for EnvHomeGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}
