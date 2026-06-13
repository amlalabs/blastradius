//! Scan context (§9.1): process/cwd/home/platform/git plus shared discovery roots.
//! Carries NO raw env values — only `EnvVarMeta { key, value_len }`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::util::git;
use crate::util::paths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextLabel {
    Cwd,
    RepoRoot,
    Worktree,
    Ax,
    Custom(String),
}

impl ContextLabel {
    pub fn as_str(&self) -> &str {
        match self {
            ContextLabel::Cwd => "cwd",
            ContextLabel::RepoRoot => "repo-root",
            ContextLabel::Worktree => "worktree",
            ContextLabel::Ax => "ax",
            ContextLabel::Custom(s) => s,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    MacOS,
    Linux,
    Other,
}

impl Platform {
    pub fn detect() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOS
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Other
        }
    }
}

/// Metadata about a single environment variable — never the value (§4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarMeta {
    pub key: String,
    pub value_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSnapshot {
    pub vars: Vec<EnvVarMeta>,
}

impl EnvSnapshot {
    /// Capture the current process environment as key + length pairs only.
    pub fn capture() -> EnvSnapshot {
        let mut vars: Vec<EnvVarMeta> = std::env::vars_os()
            .map(|(k, v)| EnvVarMeta {
                // value_len in bytes; we never retain the value itself.
                key: k.to_string_lossy().to_string(),
                value_len: v.len(),
            })
            .collect();
        vars.sort_by(|a, b| a.key.cmp(&b.key));
        EnvSnapshot { vars }
    }

    pub fn get(&self, key: &str) -> Option<&EnvVarMeta> {
        self.vars.iter().find(|v| v.key == key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.vars.iter().any(|v| v.key == key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRemote {
    pub name: String,
    /// Redacted: any `user:pass@` userinfo is stripped before storage.
    pub raw_url_redacted: String,
    pub host: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitContext {
    pub is_repo: bool,
    /// MAIN repo root, anchored via git-common-dir (§12.8). Used for discovery.
    pub repo_root: Option<PathBuf>,
    /// Literal `git rev-parse --show-toplevel` of cwd — the actual checkout
    /// (a linked worktree's own path). Used for CurrentRepo-scoped scans.
    pub worktree_toplevel: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub current_branch: Option<String>,
    pub head_sha_short: Option<String>,
    pub default_branch_guess: Option<String>,
    pub remotes: Vec<GitRemote>,
}

/// Traversal limits (§10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanLimits {
    pub max_depth_home_roots: usize,
    pub max_sibling_repos: usize,
    pub max_files_examined_per_repo: usize,
    pub max_history_bytes_per_file: u64,
    pub max_dotenv_bytes: u64,
    pub follow_symlinks: bool,
    pub cross_filesystems: bool,
    pub home_wide: bool,
}

impl Default for ScanLimits {
    fn default() -> ScanLimits {
        ScanLimits {
            max_depth_home_roots: 4,
            max_sibling_repos: 200,
            max_files_examined_per_repo: 5000,
            max_history_bytes_per_file: 50 * 1024 * 1024,
            max_dotenv_bytes: 2 * 1024 * 1024,
            follow_symlinks: false,
            cross_filesystems: false,
            home_wide: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Whether the egress probe is allowed to run at all.
    pub egress_enabled: bool,
    /// Whether we are fully offline (implies egress disabled).
    pub offline: bool,
    /// Target host:port for the egress probe (§12.11).
    pub egress_target: String,
    /// Whether broad env-name heuristics are enabled (`--env-broad`, §12.5).
    pub env_broad: bool,
    /// Whether to list env/dotenv key names (`--verbose`).
    pub verbose: bool,
    /// Opt-in: also probe cloud-metadata reachability (a 2nd outbound connect,
    /// off by default to honor the "exactly one egress check" promise, §3).
    pub check_metadata: bool,
}

impl Default for NetworkPolicy {
    fn default() -> NetworkPolicy {
        NetworkPolicy {
            egress_enabled: true,
            offline: false,
            egress_target: "1.1.1.1:443".to_string(),
            env_broad: false,
            verbose: false,
            check_metadata: false,
        }
    }
}

/// The fully-resolved scan context handed to every probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub label: ContextLabel,
    pub cwd: PathBuf,
    pub repo_root: Option<PathBuf>,
    /// The cwd's own checkout root (worktree toplevel); for CurrentRepo scans.
    pub checkout_root: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub platform: Platform,
    pub env: EnvSnapshot,
    pub git: GitContext,
    pub limits: ScanLimits,
    pub network: NetworkPolicy,
    /// Sibling-repo search roots, resolved from the MAIN repo (§12.8). In
    /// `compare` these are computed once and shared across contexts.
    pub discovery_roots: Vec<PathBuf>,
}

impl Context {
    /// Build a context from the live process, anchored at `cwd`.
    pub fn build(
        label: ContextLabel,
        cwd: PathBuf,
        limits: ScanLimits,
        network: NetworkPolicy,
    ) -> Context {
        let home = dirs::home_dir();
        let git = git::discover(&cwd);
        let repo_root = git.repo_root.clone();
        let checkout_root = git.worktree_toplevel.clone();
        let discovery_roots =
            paths::sibling_discovery_roots(repo_root.as_deref(), home.as_deref(), limits.home_wide);

        Context {
            label,
            cwd,
            repo_root,
            checkout_root,
            home,
            platform: Platform::detect(),
            env: EnvSnapshot::capture(),
            git,
            limits,
            network,
            discovery_roots,
        }
    }
}
