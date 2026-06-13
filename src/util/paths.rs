//! Path helpers: home-relative shortening and sibling discovery roots.

use std::path::{Path, PathBuf};

/// Shorten a path for display: replace the home prefix with `~` (§4.2 allows
/// shortened paths). Never reveals anything beyond the path string itself.
pub fn shorten(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Candidate sibling-repo roots, deduped by canonical path (§10).
///
/// Anchored to the MAIN repo root (`dirname(repo_root)`) plus well-known
/// code directories under home. The MAIN repo root must be passed in — never
/// recomputed from a worktree cwd (§12.8). `home_wide` additionally adds `$HOME`.
pub fn sibling_discovery_roots(
    main_repo_root: Option<&Path>,
    home: Option<&Path>,
    home_wide: bool,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(root) = main_repo_root {
        if let Some(parent) = root.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    if let Some(home) = home {
        for sub in [
            "code",
            "Code",
            "src",
            "projects",
            "Projects",
            "dev",
            "work",
            "repos",
            "workspace",
        ] {
            candidates.push(home.join(sub));
        }
        if home_wide {
            candidates.push(home.to_path_buf());
        }
    }

    // Keep only existing directories, deduped by canonical path.
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for cand in candidates {
        if !cand.is_dir() {
            continue;
        }
        let canon = cand.canonicalize().unwrap_or_else(|_| cand.clone());
        if seen.contains(&canon) {
            continue;
        }
        seen.push(canon);
        out.push(cand);
    }
    out
}

/// Directory basenames that traversal always skips (§10).
pub fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".cache"
            | "Caches"
            | "Applications"
            | "venv"
            | ".venv"
            | "__pycache__"
            | ".DS_Store"
            | "Trash"
            | "objects" // inside .git
    )
}
