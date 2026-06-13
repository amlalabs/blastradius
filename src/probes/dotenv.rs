//! §12.6 — `.env` discovery + key COUNTS (never values). Separates current-repo
//! from sibling-repo exposure. The scanner here is reused by lateral_secrets.

use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use walkdir::WalkDir;

use crate::context::{Context, ScanLimits};
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::probes::sibling_repos;
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::parse::dotenv_keys;
use crate::util::paths::is_ignored_dir;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct DotenvProbe;

#[derive(Default, Debug)]
pub struct DotenvScan {
    pub file_count: usize,
    pub key_count: usize,
    pub key_names: BTreeSet<String>,
}

/// Whether a filename is a non-example dotenv file (§12.6).
pub fn is_dotenv_file(name: &str) -> bool {
    // Exclude examples/templates first.
    for ex in [".example", ".sample", ".template", ".defaults"] {
        if name.ends_with(ex) {
            return false;
        }
    }
    name == ".env"
        || name == ".envrc"
        || name.starts_with(".env.")
        || (name.ends_with(".env") && name.len() > 4)
}

/// Scan a single repo dir for non-example dotenv files; count keys via Layer-1
/// metadata extraction (no values retained).
pub fn scan_dir_for_dotenvs(dir: &Path, limits: &ScanLimits) -> DotenvScan {
    let mut scan = DotenvScan::default();
    let mut files_examined = 0usize;

    let walker = WalkDir::new(dir)
        .max_depth(limits.max_depth_home_roots)
        .follow_links(limits.follow_symlinks)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && (is_ignored_dir(&name) || name == ".git"))
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        files_examined += 1;
        if files_examined > limits.max_files_examined_per_repo {
            break;
        }
        let name = entry.file_name().to_string_lossy();
        if !is_dotenv_file(&name) {
            continue;
        }
        // Bounded read; count oversized dotenv files without parsing values.
        match read_to_string_capped(entry.path(), limits.max_dotenv_bytes) {
            Ok(text) => {
                scan.file_count += 1;
                for key in dotenv_keys(&text) {
                    scan.key_count += 1;
                    scan.key_names.insert(key);
                }
            }
            Err(CappedReadError::TooLarge) => {
                scan.file_count += 1;
            }
            Err(_) => {}
        }
    }
    scan
}

impl Probe for DotenvProbe {
    fn id(&self) -> &'static str {
        "cross_repo.dotenv"
    }
    fn class(&self) -> FindingClass {
        FindingClass::CrossRepo
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        // Current repo scan root: the cwd's own checkout (worktree toplevel),
        // so a worktree at HEAD correctly shows untracked .env files as absent.
        let current_root = ctx
            .checkout_root
            .clone()
            .or_else(|| ctx.repo_root.clone())
            .unwrap_or_else(|| ctx.cwd.clone());
        let current = scan_dir_for_dotenvs(&current_root, &ctx.limits);

        // Sibling repos.
        let siblings = sibling_repos::enumerate(ctx);
        let mut sib_files = 0usize;
        let mut sib_keys = 0usize;
        let mut sib_repos_with = 0usize;
        for repo in &siblings {
            let s = scan_dir_for_dotenvs(repo, &ctx.limits);
            if s.file_count > 0 {
                sib_repos_with += 1;
                sib_files += s.file_count;
                sib_keys += s.key_count;
            }
        }

        let any = current.file_count > 0 || sib_files > 0;
        let severity = if any {
            Severity::Exposed
        } else {
            Severity::Info
        };

        let mut summary = format!(
            "current repo: {} file(s), {} keys; siblings: {} file(s), {} keys across {} repo(s)",
            current.file_count, current.key_count, sib_files, sib_keys, sib_repos_with
        );
        if !any {
            summary = "no non-example .env files reachable".to_string();
        }

        let mut evidence = json!({
            "current_repo": { "file_count": current.file_count, "key_count": current.key_count },
            "sibling_repos": {
                "repo_count": sib_repos_with,
                "file_count": sib_files,
                "key_count": sib_keys,
            },
        });
        // --verbose lists KEY NAMES only (never values).
        if ctx.options.verbose {
            let names: Vec<&String> = current.key_names.iter().collect();
            evidence["current_repo"]["key_names"] = json!(names);
        }

        // The current-repo portion is CurrentRepo scope (may differ in a worktree);
        // the sibling portion is SiblingRepos scope. We emit two findings so each
        // lands in the right comparison bucket.
        let current_finding = Finding::new(
            format!("{}.current", self.id()),
            self.class(),
            FindingScope::CurrentRepo,
            if current.file_count > 0 {
                ".env files in current repo"
            } else {
                "no .env files in current repo"
            },
            if current.file_count > 0 {
                Severity::Exposed
            } else {
                Severity::Info
            },
            Confidence::Confirmed,
        )
        .summary(format!(
            "current repo: {} file(s), {} keys",
            current.file_count, current.key_count
        ))
        .evidence(evidence["current_repo"].clone());

        let sibling_finding = Finding::new(
            format!("{}.siblings", self.id()),
            self.class(),
            FindingScope::SiblingRepos,
            if sib_files > 0 {
                ".env files in sibling repos"
            } else {
                "no .env files in sibling repos"
            },
            if sib_files > 0 {
                Severity::Exposed
            } else {
                Severity::Info
            },
            Confidence::Confirmed,
        )
        .summary(format!(
            "sibling repos: {} file(s), {} keys across {} repo(s)",
            sib_files, sib_keys, sib_repos_with
        ))
        .evidence(evidence["sibling_repos"].clone())
        .remediation(&[
            "Keep agent filesystem scope to the task repo so neighboring .env files aren't reachable.",
        ]);

        let _ = severity; // severity is computed per-finding above
        let _ = summary;

        Ok(vec![current_finding, sibling_finding])
    }
}
