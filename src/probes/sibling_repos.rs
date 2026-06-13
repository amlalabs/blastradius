//! §12.8 — sibling-repo enumeration. Uses `discovery_roots` anchored to the MAIN
//! repo (shared across contexts in `compare`), NOT recomputed from cwd.

use serde_json::json;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::{is_ignored_dir, shorten};

pub struct SiblingReposProbe;

/// A directory is a git repo root if it contains a `.git` dir OR a `.git` file
/// (linked worktrees use a `.git` file pointing at a gitdir) — §12.8.
fn is_repo_root(dir: &Path) -> bool {
    let dot_git = dir.join(".git");
    dot_git.is_dir() || dot_git.is_file()
}

/// Enumerate sibling git repos reachable from the shared discovery roots.
/// Excludes the current MAIN repo; dedupes canonical paths; never runs repo code.
pub fn enumerate(ctx: &Context) -> Vec<PathBuf> {
    let exclude_canon: Vec<PathBuf> = ctx
        .repo_root
        .as_ref()
        .map(|r| vec![r.canonicalize().unwrap_or_else(|_| r.clone())])
        .unwrap_or_default();

    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen_canon: Vec<PathBuf> = Vec::new();

    'roots: for root in &ctx.discovery_roots {
        let walker = WalkDir::new(root)
            .max_depth(ctx.limits.max_depth_home_roots)
            .follow_links(ctx.limits.follow_symlinks)
            .into_iter()
            .filter_entry(|e| {
                // Skip ignored directories outright.
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !(e.file_type().is_dir() && is_ignored_dir(&name))
            });

        for entry in walker.flatten() {
            if !entry.file_type().is_dir() {
                continue;
            }
            let dir = entry.path();
            if !is_repo_root(dir) {
                continue;
            }
            let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
            if exclude_canon.contains(&canon) || seen_canon.contains(&canon) {
                continue;
            }
            seen_canon.push(canon);
            found.push(dir.to_path_buf());
            if found.len() >= ctx.limits.max_sibling_repos {
                break 'roots;
            }
        }
    }

    found.sort();
    found
}

impl Probe for SiblingReposProbe {
    fn id(&self) -> &'static str {
        "cross_repo.sibling_repos"
    }
    fn class(&self) -> FindingClass {
        FindingClass::CrossRepo
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let siblings = enumerate(ctx);
        let count = siblings.len();
        let truncated = count >= ctx.limits.max_sibling_repos;

        let shown: Vec<String> = siblings
            .iter()
            .take(10)
            .map(|p| shorten(p, ctx.home.as_deref()))
            .collect();

        let severity = if count > 0 {
            Severity::Notable
        } else {
            Severity::Info
        };

        let summary = if count == 0 {
            "no sibling repos reachable from discovery roots".to_string()
        } else if count > shown.len() {
            format!(
                "{count} sibling repos readable from here (showing {}, +{} more)",
                shown.len(),
                count - shown.len()
            )
        } else {
            format!("{count} sibling repos readable from here")
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::SiblingRepos,
            if count > 0 {
                "sibling repositories reachable"
            } else {
                "no sibling repositories reachable"
            },
            severity,
            Confidence::Confirmed,
        )
        .summary(summary)
        .evidence(json!({
            "count": count,
            "shown": shown,
            "truncated": truncated,
        }))
        .remediation(&[
            "Mount only the task repo into agent environments — not the parent code directory.",
        ]);

        Ok(vec![finding])
    }
}
