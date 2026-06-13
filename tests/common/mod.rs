//! Shared test helpers: build a Context pointing at fixture dirs (no live env).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use blastradius::context::{
    Context, ContextLabel, EnvSnapshot, EnvVarMeta, GitContext, Platform, ScanLimits, ScanOptions,
};

/// Build a fully-controlled context for probe tests.
pub fn ctx_with(home: &Path, cwd: &Path) -> Context {
    Context {
        label: ContextLabel::Cwd,
        cwd: cwd.to_path_buf(),
        repo_root: Some(cwd.to_path_buf()),
        checkout_root: Some(cwd.to_path_buf()),
        home: Some(home.to_path_buf()),
        platform: Platform::detect(),
        env: EnvSnapshot { vars: Vec::new() },
        git: GitContext::default(),
        limits: ScanLimits::default(),
        options: ScanOptions::default(),
        discovery_roots: Vec::new(),
    }
}

/// Add an env var (name + length only) to a context.
pub fn with_env(mut ctx: Context, key: &str, value_len: usize) -> Context {
    ctx.env.vars.push(EnvVarMeta {
        key: key.to_string(),
        value_len,
    });
    ctx
}

/// Set the discovery roots (sibling search roots) directly.
pub fn with_roots(mut ctx: Context, roots: Vec<PathBuf>) -> Context {
    ctx.discovery_roots = roots;
    ctx
}
