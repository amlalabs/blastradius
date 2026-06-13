//! §24.7 — root resolution + glob expansion for discovery.
//!
//! Reads `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`COPILOT_HOME`/`CURSOR_PROJECT_DIR`
//! from `std::env` directly (the `EnvSnapshot` stores only key+value_len). Env
//! paths are used transiently to resolve+glob and are **never** persisted into
//! any `SessionSource`/cursor/report.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Discovery root anchors (§24.1). `MacAppSupport` simply yields zero
/// candidates on Linux, so the table is portable with no `cfg!` forks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    Home,
    XdgConfig,
    XdgData,
    XdgState,
    MacAppSupport,
    CurrentRepo,
}

impl Root {
    /// Resolve this root to an absolute base directory, if it exists for this
    /// environment. Reads the XDG/COPILOT/CURSOR env overrides directly from
    /// `std::env` (never persisted).
    pub fn resolve(self) -> Option<PathBuf> {
        match self {
            Root::Home => dirs::home_dir(),
            Root::XdgConfig => env_path("XDG_CONFIG_HOME")
                .or_else(|| dirs::home_dir().map(|h| h.join(".config"))),
            Root::XdgData => env_path("XDG_DATA_HOME")
                .or_else(|| dirs::home_dir().map(|h| h.join(".local/share"))),
            Root::XdgState => env_path("XDG_STATE_HOME")
                .or_else(|| dirs::home_dir().map(|h| h.join(".local/state"))),
            // macOS-only base. On Linux this directory does not exist, so the
            // source yields zero candidates (portable, no `cfg!`).
            Root::MacAppSupport => dirs::home_dir().map(|h| h.join("Library/Application Support")),
            // The repo we are auditing — the CWD's repo root, or just the CWD.
            Root::CurrentRepo => std::env::current_dir().ok(),
        }
    }
}

/// Read an absolute path from an env var, transiently. Returns `None` for unset
/// or non-absolute values (a relative XDG override is invalid per the spec).
fn env_path(key: &str) -> Option<PathBuf> {
    let v = std::env::var_os(key)?;
    if v.is_empty() {
        return None;
    }
    let p = PathBuf::from(v);
    if p.is_absolute() {
        Some(p)
    } else {
        None
    }
}

/// Honors the subset of `ScanLimits` discovery needs (held by the caller so the
/// inventory of every directory opened stays auditable).
#[derive(Debug, Clone, Copy)]
pub struct GlobLimits {
    pub follow_symlinks: bool,
    /// Cap on directories walked per glob (defense against pathological trees).
    pub max_entries: usize,
}

impl Default for GlobLimits {
    fn default() -> Self {
        GlobLimits {
            follow_symlinks: false,
            max_entries: 50_000,
        }
    }
}

/// Expand a root-relative glob into concrete transcript file paths.
///
/// Supports `*` (single path segment, no separator) and `**` (zero or more
/// segments). Symlinks are NOT followed (`follow_symlinks=false` by default,
/// §24.2.6). An empty glob yields nothing (hook-supplied / detect-only agents).
pub fn expand_glob(root: Root, glob: &str) -> Vec<PathBuf> {
    expand_glob_with(root, glob, GlobLimits::default())
}

/// `expand_glob` with explicit limits (the caller passes the `ScanLimits`-derived
/// policy). Resolution is rooted at `root.resolve()`; if the root does not
/// resolve, the result is empty.
pub fn expand_glob_with(root: Root, glob: &str, limits: GlobLimits) -> Vec<PathBuf> {
    if glob.is_empty() {
        return Vec::new();
    }
    let base = match root.resolve() {
        Some(b) => b,
        None => return Vec::new(),
    };
    expand_under(&base, glob, limits)
}

/// Expand a root-relative glob under a concrete base dir. Exposed for tests so
/// fixtures can be rooted in a temp dir without env mutation.
pub fn expand_under(base: &Path, glob: &str, limits: GlobLimits) -> Vec<PathBuf> {
    let segments: Vec<&str> = glob.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Vec::new();
    }

    // Fast path: no wildcard anywhere → a single literal join.
    if !glob.contains('*') {
        let candidate = base.join(glob);
        return if exists_no_symlink_follow(&candidate, limits) {
            vec![candidate]
        } else {
            Vec::new()
        };
    }

    let mut out = Vec::new();
    let mut walked = 0usize;
    match_segments(base, &segments, limits, &mut walked, &mut out);
    out.sort();
    out
}

/// Recursive wildcard matcher over path segments.
fn match_segments(
    dir: &Path,
    segments: &[&str],
    limits: GlobLimits,
    walked: &mut usize,
    out: &mut Vec<PathBuf>,
) {
    if *walked > limits.max_entries {
        return;
    }
    let (seg, rest) = match segments.split_first() {
        Some(x) => x,
        None => {
            // No more segments: `dir` itself is a match if it is a file.
            if dir.is_file() {
                out.push(dir.to_path_buf());
            }
            return;
        }
    };

    if *seg == "**" {
        // `**` matches zero or more directories. First try matching the rest at
        // the current level (zero dirs consumed)…
        match_segments(dir, rest, limits, walked, out);
        // …then descend into every subdir and re-apply `**` + rest.
        for child in read_dir_entries(dir, limits, walked) {
            if child.is_dir() {
                match_segments(&child, segments, limits, walked, out);
            }
        }
        return;
    }

    let is_last = rest.is_empty();
    for child in read_dir_entries(dir, limits, walked) {
        let name = match child.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !wildcard_match(seg, &name) {
            continue;
        }
        if is_last {
            if child.is_file() {
                out.push(child);
            }
        } else if child.is_dir() {
            match_segments(&child, rest, limits, walked, out);
        }
    }
}

/// List the immediate children of `dir`, honoring symlink policy + the walk cap.
fn read_dir_entries(dir: &Path, limits: GlobLimits, walked: &mut usize) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let walker = WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .follow_links(limits.follow_symlinks);
    for entry in walker.into_iter().flatten() {
        *walked += 1;
        if *walked > limits.max_entries {
            break;
        }
        // Never follow a symlink entry itself (defense beyond follow_links).
        if !limits.follow_symlinks && entry.path_is_symlink() {
            continue;
        }
        entries.push(entry.path().to_path_buf());
    }
    entries
}

/// Single-segment `*`-glob match (no `/`). `*` matches any run of chars within
/// one segment.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == name;
    }
    // Greedy split on `*`; each literal part must appear in order.
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut cursor = 0usize;
    let bytes = name.as_bytes();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // Must match at the start.
            if !name[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
        } else if i == parts.len() - 1 {
            // Final literal must match at the end.
            if !name.ends_with(part) {
                return false;
            }
            // ensure no overlap with what we've already consumed
            if name.len() < cursor + part.len() {
                return false;
            }
        } else if let Some(pos) = name[cursor..].find(part) {
            cursor += pos + part.len();
        } else {
            return false;
        }
        let _ = bytes;
    }
    true
}

fn exists_no_symlink_follow(path: &Path, limits: GlobLimits) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() && !limits.follow_symlinks {
                return false;
            }
            meta.is_file() || (limits.follow_symlinks && path.is_file())
        }
        Err(_) => false,
    }
}

/// Shorten an absolute transcript path to a value-free LABEL (never a raw
/// `$HOME` path), for `SessionDigest.source_label`.
///
/// Strategy: replace the home prefix with `~`, then keep only the trailing few
/// components so a per-user directory layout cannot leak. The label is for
/// operator orientation, never a join key.
pub fn source_label(path: &Path) -> String {
    let home = dirs::home_dir();
    let display: PathBuf = match &home {
        Some(h) if path.starts_with(h) => {
            let rest = path.strip_prefix(h).unwrap_or(path);
            PathBuf::from("~").join(rest)
        }
        _ => path.to_path_buf(),
    };

    // Keep at most the last 4 components so deep per-user trees collapse.
    let comps: Vec<_> = display.components().collect();
    if comps.len() <= 5 {
        return display.to_string_lossy().into_owned();
    }
    let tail: PathBuf = comps[comps.len() - 4..].iter().collect();
    format!(".../{}", tail.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn wildcard_segment_matching() {
        assert!(wildcard_match("*.jsonl", "rollout-abc.jsonl"));
        assert!(wildcard_match("rollout-*.jsonl", "rollout-2026.jsonl"));
        assert!(!wildcard_match("rollout-*.jsonl", "other.jsonl"));
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("msg_*.json", "msg_001.json"));
        assert!(!wildcard_match("msg_*.json", "thread.json"));
    }

    #[test]
    fn expand_single_star_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        fs::create_dir_all(base.join(".claude/projects/repo-a")).unwrap();
        fs::create_dir_all(base.join(".claude/projects/repo-b")).unwrap();
        fs::write(base.join(".claude/projects/repo-a/s1.jsonl"), "x").unwrap();
        fs::write(base.join(".claude/projects/repo-b/s2.jsonl"), "y").unwrap();
        fs::write(base.join(".claude/projects/repo-a/note.txt"), "z").unwrap();

        let mut found = expand_under(base, ".claude/projects/*/*.jsonl", GlobLimits::default());
        found.sort();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|p| p.extension().unwrap() == "jsonl"));
    }

    #[test]
    fn expand_double_star() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        fs::create_dir_all(base.join("Cursor/User/globalStorage/x")).unwrap();
        fs::write(base.join("Cursor/User/globalStorage/x/state.vscdb"), "x").unwrap();
        let found = expand_under(base, "Cursor/User/**/state.vscdb", GlobLimits::default());
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn literal_glob_no_wildcard() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        fs::write(base.join(".aider.chat.history.md"), "x").unwrap();
        let found = expand_under(base, ".aider.chat.history.md", GlobLimits::default());
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn source_label_never_contains_raw_home() {
        // A synthetic deep path; the label keeps only the tail.
        let p = Path::new("/home/someuser/.claude/projects/enc-cwd/3f2ae1.jsonl");
        let label = source_label(p);
        assert!(label.contains("3f2ae1.jsonl"));
        assert!(label.starts_with(".../"));
    }
}
