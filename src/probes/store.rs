//! Spec-driven credential/config-store probe engine.
//!
//! Most credential stores share one shape: "one or more files (or dirs) on disk;
//! if reachable, report the identifier NAMES / COUNTS we can extract — never the
//! secret values." Rather than hand-roll a struct per store (aws.rs is the
//! original template), this module turns that shape into DATA: each store is a
//! [`StoreSpec`] entry in [`STORES`], and one [`StoreProbe`] engine runs them all.
//!
//! Adding a new credential store is therefore a few lines of data in [`STORES`]
//! — no new file, no new `Probe` impl, no runner edit. See `registry.rs`.
//!
//! READ-ONLY and value-free: only identifier names (profile/context/registry/
//! host names), counts, shortened paths, and skip reasons ever leave a probe.
//! Path resolution reads the live process env for store-specific overrides
//! (e.g. `KUBECONFIG`, `DOCKER_CONFIG`) exactly as `aws.rs` reads
//! `AWS_SHARED_CREDENTIALS_FILE` — the path, never the value.

use serde_json::json;
use std::path::PathBuf;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::parse::ini_section_names;
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

/// Hard cap for any single store file (token caches/JSON can be a few MB).
const MAX_STORE_BYTES: u64 = 8 * 1024 * 1024;

/// How to derive value-free identifier names from one candidate path.
#[derive(Clone, Copy)]
pub enum Extract {
    /// The file's mere existence is the exposure (the file *is* the secret,
    /// e.g. `~/.vault-token`). Drives `Exposed` with no name list.
    Presence,
    /// INI/TOML `[section]` names (aws/pypi/cargo style).
    Ini,
    /// YAML or JSON document (serde_yaml parses both); the fn returns identifier
    /// names from the parsed value. Must never return secret-bearing fields.
    Structured(fn(&serde_yaml::Value) -> Vec<String>),
    /// Raw-text line scan returning identifier names (npmrc registries, pgpass
    /// hosts, terraformrc hostnames). Must never return a secret substring.
    Lines(fn(&str) -> Vec<String>),
    /// Count immediate directory entries whose file name passes the predicate
    /// (e.g. gnupg `private-keys-v1.d/*.key`). Count only — names are not
    /// emitted (they can be account emails / keygrips).
    DirCount(fn(&str) -> bool),
}

/// One file or directory to inspect for a store.
pub struct Candidate {
    pub path: PathBuf,
    pub extract: Extract,
}

impl Candidate {
    pub fn new(path: PathBuf, extract: Extract) -> Candidate {
        Candidate { path, extract }
    }
}

/// A declarative credential-store detection. One entry == one probe.
pub struct StoreSpec {
    /// Stable finding id, e.g. `gcp.credentials`.
    pub id: &'static str,
    /// Human label used to build titles, e.g. "GCP credentials".
    pub label: &'static str,
    /// Noun for extracted items, e.g. "context", "registry", "host".
    pub item_noun: &'static str,
    /// Resolve the candidate paths from the scan context (+ live env overrides).
    pub resolve: fn(&Context) -> Vec<Candidate>,
    pub remediation: &'static [&'static str],
}

/// The engine: wraps one `&'static StoreSpec` as a `Probe`.
pub struct StoreProbe(pub &'static StoreSpec);

impl Probe for StoreProbe {
    fn id(&self) -> &'static str {
        self.0.id
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        Ok(vec![run_store(self.0, ctx)])
    }
}

/// Result of scanning all of a store's candidates.
#[derive(Default)]
struct StoreScan {
    files_present: Vec<String>,
    items: Vec<String>,
    /// Items we counted but deliberately did not name (DirCount sources).
    unnamed_item_count: usize,
    skipped_files: Vec<serde_json::Value>,
    /// At least one `Presence` candidate exists (the file *is* a secret).
    secret_file_present: bool,
}

fn run_store(spec: &StoreSpec, ctx: &Context) -> Finding {
    let home = ctx.home.as_deref();
    let mut scan = StoreScan::default();

    for cand in (spec.resolve)(ctx) {
        match cand.extract {
            Extract::Presence => {
                if let Ok(meta) = std::fs::metadata(&cand.path) {
                    if meta.is_file() {
                        scan.secret_file_present = true;
                        push_unique(&mut scan.files_present, shorten(&cand.path, home));
                    }
                }
            }
            Extract::DirCount(pred) => {
                if let Ok(entries) = std::fs::read_dir(&cand.path) {
                    let mut n = 0usize;
                    for e in entries.flatten() {
                        let name = e.file_name();
                        if pred(&name.to_string_lossy()) {
                            n += 1;
                        }
                    }
                    if n > 0 {
                        scan.unnamed_item_count += n;
                        push_unique(&mut scan.files_present, shorten(&cand.path, home));
                    }
                }
            }
            _ => read_and_extract(&cand, home, &mut scan),
        }
    }

    let item_total = scan.items.len() + scan.unnamed_item_count;
    let has_creds = scan.secret_file_present || item_total > 0;

    let severity = if has_creds {
        Severity::Exposed
    } else if !scan.files_present.is_empty() {
        Severity::Notable
    } else {
        Severity::Info
    };

    let title = if has_creds {
        format!("{} reachable", spec.label)
    } else if !scan.files_present.is_empty() {
        format!("{} config present (no credentials parsed)", spec.label)
    } else {
        format!("no {} reachable", spec.label)
    };

    let summary = if item_total > 0 && !scan.items.is_empty() {
        format!(
            "{} {}(s): {}",
            item_total,
            spec.item_noun,
            scan.items.join(", ")
        )
    } else if item_total > 0 {
        format!("{} {}(s) reachable", item_total, spec.item_noun)
    } else if scan.secret_file_present {
        format!("{} present on disk", spec.label)
    } else if !scan.files_present.is_empty() {
        format!("{} config present; no credentials parsed", spec.label)
    } else {
        format!("no {} found", spec.label)
    };

    Finding::new(
        spec.id,
        FindingClass::Credentials,
        FindingScope::Ambient,
        title,
        severity,
        Confidence::Confirmed,
    )
    .summary(summary)
    .evidence(json!({
        "files_present": scan.files_present,
        "item_noun": spec.item_noun,
        "items": scan.items,
        "item_count": item_total,
        "skipped_files": scan.skipped_files,
        "note": "Identifier names and counts only; no secret values are read or emitted.",
    }))
    .remediation(spec.remediation)
}

/// Read a candidate file (capped) and apply its text/structured extractor.
fn read_and_extract(cand: &Candidate, home: Option<&std::path::Path>, scan: &mut StoreScan) {
    let text = match read_to_string_capped(&cand.path, MAX_STORE_BYTES) {
        Ok(t) => t,
        Err(CappedReadError::NotFound | CappedReadError::NotFile) => return,
        Err(e) => {
            push_unique(&mut scan.files_present, shorten(&cand.path, home));
            scan.skipped_files.push(json!({
                "path": shorten(&cand.path, home),
                "reason": e.reason(),
            }));
            return;
        }
    };
    push_unique(&mut scan.files_present, shorten(&cand.path, home));

    let names = match cand.extract {
        Extract::Ini => ini_section_names(&text),
        Extract::Lines(f) => f(&text),
        Extract::Structured(f) => match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(v) => f(&v),
            Err(_) => {
                scan.skipped_files.push(json!({
                    "path": shorten(&cand.path, home),
                    "reason": "parse error",
                }));
                Vec::new()
            }
        },
        // Presence / DirCount handled before this fn is called.
        Extract::Presence | Extract::DirCount(_) => Vec::new(),
    };
    for n in names {
        push_unique(&mut scan.items, n);
    }
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && !v.contains(&s) {
        v.push(s);
    }
}

// ---------------------------------------------------------------------------
// Path-resolution helpers shared by store specs.
// ---------------------------------------------------------------------------

/// Resolve a store base dir from an env override, else `~/<rel>`.
fn base_dir(ctx: &Context, env_override: &str, rel: &str) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os(env_override) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    ctx.home.as_ref().map(|h| h.join(rel))
}

/// Resolve a single file path from an env override, else `~/<rel>`.
fn file_path(ctx: &Context, env_override: &str, rel: &str) -> Option<PathBuf> {
    base_dir(ctx, env_override, rel)
}

// ---------------------------------------------------------------------------
// Value-free extractors.
// ---------------------------------------------------------------------------

/// Kubernetes: cluster + context NAMES from a kubeconfig (YAML). Never touches
/// `users[].user.token` / client-cert data.
fn kube_names(v: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["clusters", "contexts"] {
        if let Some(seq) = v.get(key).and_then(|x| x.as_sequence()) {
            for item in seq {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

/// Docker: registry hostnames are the keys of `auths` (and credential-helper
/// host keys). Values (base64 `auth`) are never read.
fn docker_registries(v: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["auths", "credHelpers"] {
        if let Some(map) = v.get(key).and_then(|x| x.as_mapping()) {
            for k in map.keys() {
                if let Some(s) = k.as_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// Terraform Cloud/Enterprise: host names are the keys of `credentials` in
/// `credentials.tfrc.json`. The `token` values are never read.
fn tf_credential_hosts(v: &serde_yaml::Value) -> Vec<String> {
    v.get("credentials")
        .and_then(|x| x.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// npm: registry hosts that carry an auth token. From `//host/:_authToken=...`
/// take the host; a bare `_auth`/`_authToken` (default registry) reports the
/// default. The token itself is never captured.
fn npmrc_registries(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let key = line.split('=').next().unwrap_or("").trim();
        let lower = key.to_ascii_lowercase();
        let is_auth = lower.ends_with(":_authtoken")
            || lower.ends_with(":_auth")
            || lower.ends_with(":_password")
            || lower == "_authtoken"
            || lower == "_auth"
            || lower == "_password";
        if !is_auth {
            continue;
        }
        if let Some(reg) = key.split(":_").next().filter(|s| s.starts_with("//")) {
            out.push(reg.trim_start_matches('/').to_string());
        } else {
            out.push("(default registry)".to_string());
        }
    }
    out
}

/// pgpass: the host field (column 0) of each entry. The password (column 4) is
/// never read.
fn pgpass_hosts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(host) = line.split(':').next() {
            if !host.is_empty() && host != "*" {
                out.push(host.to_string());
            }
        }
    }
    out
}

/// terraformrc (HCL): hostnames from `credentials "host" { ... }` blocks. The
/// `token = "..."` inside the block is never captured.
fn terraformrc_hosts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("credentials ") {
            let rest = rest.trim();
            if let Some(open) = rest.find('"') {
                if let Some(close) = rest[open + 1..].find('"') {
                    let host = &rest[open + 1..open + 1 + close];
                    if !host.is_empty() {
                        out.push(host.to_string());
                    }
                }
            }
        }
    }
    out
}

fn is_key_suffix(name: &str) -> bool {
    name.ends_with(".key")
}

fn is_json_suffix(name: &str) -> bool {
    name.ends_with(".json")
}

/// GNOME Keyring (`*.keyring`) / KWallet (`*.kwl`) secret-store files.
fn is_keyring_suffix(name: &str) -> bool {
    name.ends_with(".keyring") || name.ends_with(".kwl")
}

fn is_conf_suffix(name: &str) -> bool {
    name.ends_with(".conf")
}

fn is_gpg_suffix(name: &str) -> bool {
    name.ends_with(".gpg")
}

fn any_entry(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".."
}

/// Config-property KEY names (`key=value` / `key: value`) whose NAME looks
/// secret-bearing. Returns the key names only — never values. Used for
/// gradle.properties, ~/.my.cnf, etc.
fn secret_property_keys(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[')
        {
            continue;
        }
        let key = line
            .split_once('=')
            .or_else(|| line.split_once(':'))
            .map(|(k, _)| k.trim())
            .unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let l = key.to_ascii_lowercase();
        if ["password", "passwd", "token", "secret", "apikey", "api_key", "api-key", "credential", "auth", "private_key", "signing"]
            .iter()
            .any(|p| l.contains(p))
        {
            out.push(key.to_string());
        }
    }
    out
}

/// XML credential markers (maven settings.xml, NuGet.Config). Returns a marker
/// when a password/apikey element is present — never the element's value.
fn xml_credential_markers(text: &str) -> Vec<String> {
    let l = text.to_ascii_lowercase();
    let mut out = Vec::new();
    if l.contains("<password") || l.contains("cleartextpassword") {
        out.push("password-entry".to_string());
    }
    if l.contains("apikey") || l.contains("<apikeys") {
        out.push("apikey-entry".to_string());
    }
    out
}

/// Hostnames from URLs that embed userinfo (`scheme://user:pass@host`). Returns
/// the host only — never the userinfo. Used for pip.conf index URLs etc.
fn url_userinfo_hosts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(at) = line.find('@') {
            // Must look like `://...@host` to avoid emails / scp specs.
            if line[..at].contains("://") && line[..at].contains(':') {
                let after = &line[at + 1..];
                let host: String = after
                    .chars()
                    .take_while(|c| !matches!(c, '/' | ':' | ' ' | '"' | '\''))
                    .collect();
                if !host.is_empty() {
                    out.push(host);
                }
            }
        }
    }
    out
}

/// Composer auth.json: host keys under each auth type (github-oauth, http-basic,
/// gitlab-token, bearer). Never the token/password values.
fn composer_hosts(v: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["github-oauth", "gitlab-token", "gitlab-oauth", "http-basic", "bearer"] {
        if let Some(map) = v.get(key).and_then(|x| x.as_mapping()) {
            for k in map.keys() {
                if let Some(s) = k.as_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// dbt profiles.yml: profile names (top-level keys, minus `config`). The DB
/// passwords nested inside are never read.
fn dbt_profiles(v: &serde_yaml::Value) -> Vec<String> {
    v.as_mapping()
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str())
                .filter(|k| *k != "config")
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The store table — add a new credential store here.
// ---------------------------------------------------------------------------

const STORES: &[StoreSpec] = &[
    StoreSpec {
        id: "gcp.credentials",
        label: "GCP credentials",
        item_noun: "account",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(base) = base_dir(ctx, "CLOUDSDK_CONFIG", ".config/gcloud") {
                for f in [
                    "application_default_credentials.json",
                    "credentials.db",
                    "access_tokens.db",
                ] {
                    out.push(Candidate::new(base.join(f), Extract::Presence));
                }
                out.push(Candidate::new(
                    base.join("legacy_credentials"),
                    Extract::DirCount(|_| true),
                ));
            }
            out
        },
        remediation: &[
            "Use a dedicated, narrowly-scoped service account per agent; don't mount ~/.config/gcloud.",
            "Prefer short-lived credentials (workload identity / impersonation) over stored refresh tokens.",
        ],
    },
    StoreSpec {
        id: "azure.credentials",
        label: "Azure credentials",
        item_noun: "token cache",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(base) = base_dir(ctx, "AZURE_CONFIG_DIR", ".azure") {
                for f in [
                    "accessTokens.json",
                    "msal_token_cache.json",
                    "service_principal_entries.json",
                ] {
                    out.push(Candidate::new(base.join(f), Extract::Presence));
                }
            }
            out
        },
        remediation: &[
            "Use managed identity or a scoped service principal per agent; don't mount ~/.azure.",
            "Prefer short-lived tokens over a persisted MSAL/token cache.",
        ],
    },
    StoreSpec {
        id: "kube.config",
        label: "Kubernetes credentials",
        item_noun: "context",
        resolve: |ctx| {
            // $KUBECONFIG (PATH-separated) overrides the default ~/.kube/config.
            if let Some(v) = std::env::var_os("KUBECONFIG") {
                if !v.is_empty() {
                    return std::env::split_paths(&v)
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(|p| Candidate::new(p, Extract::Structured(kube_names)))
                        .collect();
                }
            }
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![Candidate::new(
                        h.join(".kube/config"),
                        Extract::Structured(kube_names),
                    )]
                })
                .unwrap_or_default()
        },
        remediation: &[
            "Give agents a scoped kubeconfig (least-privilege RBAC, short-lived token) — not your full ~/.kube/config.",
            "Filesystem-isolate ~/.kube so cluster-admin contexts aren't reachable.",
        ],
    },
    StoreSpec {
        id: "docker.registry_auth",
        label: "Docker registry credentials",
        item_noun: "registry",
        resolve: |ctx| {
            base_dir(ctx, "DOCKER_CONFIG", ".docker")
                .map(|base| {
                    vec![Candidate::new(
                        base.join("config.json"),
                        Extract::Structured(docker_registries),
                    )]
                })
                .unwrap_or_default()
        },
        remediation: &[
            "Use a scoped, short-lived registry token per agent; don't mount ~/.docker/config.json.",
            "Prefer a credential helper backed by the OS keychain over base64 auths in config.json.",
        ],
    },
    StoreSpec {
        id: "vault.token",
        label: "Vault token",
        item_noun: "token",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| vec![Candidate::new(h.join(".vault-token"), Extract::Presence)])
                .unwrap_or_default()
        },
        remediation: &[
            "Don't persist ~/.vault-token into agent environments; issue a scoped, short-TTL token per task.",
        ],
    },
    StoreSpec {
        id: "npm.token",
        label: "npm credentials",
        item_noun: "registry",
        resolve: |ctx| {
            file_path(ctx, "NPM_CONFIG_USERCONFIG", ".npmrc")
                .map(|p| vec![Candidate::new(p, Extract::Lines(npmrc_registries))])
                .unwrap_or_default()
        },
        remediation: &[
            "Inject a scoped, short-lived NPM_TOKEN per task instead of a global ~/.npmrc auth token.",
        ],
    },
    StoreSpec {
        id: "pypi.token",
        label: "PyPI credentials",
        item_noun: "index",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| vec![Candidate::new(h.join(".pypirc"), Extract::Ini)])
                .unwrap_or_default()
        },
        remediation: &[
            "Use a scoped PyPI API token per task; don't expose ~/.pypirc to agents.",
        ],
    },
    StoreSpec {
        id: "cargo.token",
        label: "Cargo credentials",
        item_noun: "registry",
        resolve: |ctx| {
            let base = base_dir(ctx, "CARGO_HOME", ".cargo");
            base.map(|b| {
                vec![
                    Candidate::new(b.join("credentials.toml"), Extract::Ini),
                    Candidate::new(b.join("credentials"), Extract::Ini),
                ]
            })
            .unwrap_or_default()
        },
        remediation: &[
            "Issue a scoped crates.io token per task; don't mount ~/.cargo/credentials.toml.",
        ],
    },
    StoreSpec {
        id: "terraform.token",
        label: "Terraform Cloud credentials",
        item_noun: "host",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(p) = file_path(ctx, "TF_CLI_CONFIG_FILE", ".terraformrc") {
                out.push(Candidate::new(p, Extract::Lines(terraformrc_hosts)));
            }
            if let Some(h) = &ctx.home {
                out.push(Candidate::new(
                    h.join(".terraform.d/credentials.tfrc.json"),
                    Extract::Structured(tf_credential_hosts),
                ));
            }
            out
        },
        remediation: &[
            "Use a scoped Terraform Cloud/Enterprise team token per task; don't expose ~/.terraform.d.",
        ],
    },
    StoreSpec {
        id: "postgres.pgpass",
        label: "Postgres passwords (.pgpass)",
        item_noun: "host",
        resolve: |ctx| {
            file_path(ctx, "PGPASSFILE", ".pgpass")
                .map(|p| vec![Candidate::new(p, Extract::Lines(pgpass_hosts))])
                .unwrap_or_default()
        },
        remediation: &[
            "Don't expose ~/.pgpass to agents; use a scoped DB role with a short-lived credential per task.",
        ],
    },
    StoreSpec {
        id: "gpg.private_keys",
        label: "GPG private keys",
        item_noun: "secret key",
        resolve: |ctx| {
            let base = base_dir(ctx, "GNUPGHOME", ".gnupg");
            base.map(|b| {
                vec![
                    Candidate::new(b.join("private-keys-v1.d"), Extract::DirCount(is_key_suffix)),
                    Candidate::new(b.join("secring.gpg"), Extract::Presence),
                ]
            })
            .unwrap_or_default()
        },
        remediation: &[
            "Keep ~/.gnupg out of agent scope; a reachable signing key enables commit/artifact forgery.",
        ],
    },
    // --- Commonly-missed surfaces ---
    StoreSpec {
        id: "aws.sso_cache",
        label: "AWS SSO/CLI token cache",
        item_noun: "cached token",
        resolve: |ctx| {
            // The AWS profile probe parses ~/.aws/{credentials,config} NAMES but
            // not the SSO/CLI token caches, which hold LIVE bearer tokens. These
            // caches always live under ~/.aws (not relocatable by a dir env var).
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".aws/sso/cache"), Extract::DirCount(is_json_suffix)),
                        Candidate::new(h.join(".aws/cli/cache"), Extract::DirCount(is_json_suffix)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &[
            "SSO/CLI token caches hold live bearer tokens; keep ~/.aws out of agent scope and use short SSO session TTLs.",
        ],
    },
    StoreSpec {
        id: "keyring.secret_store",
        label: "OS keyring / secret store",
        item_noun: "keyring",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(h) = &ctx.home {
                // GNOME Keyring / libsecret + KWallet (Linux Secret Service).
                out.push(Candidate::new(
                    h.join(".local/share/keyrings"),
                    Extract::DirCount(is_keyring_suffix),
                ));
                out.push(Candidate::new(
                    h.join(".local/share/kwalletd"),
                    Extract::DirCount(is_keyring_suffix),
                ));
                // macOS login keychain.
                out.push(Candidate::new(
                    h.join("Library/Keychains/login.keychain-db"),
                    Extract::Presence,
                ));
            }
            out
        },
        remediation: &[
            "The OS keyring backs the Secret Service / Keychain for many tools; an unlocked keyring readable by the agent exposes every stored secret.",
            "Run agents under a separate login session / keyring, or keep the keyring locked when an agent runs.",
        ],
    },
    StoreSpec {
        id: "ai_assistant.credentials",
        label: "AI coding-assistant credentials",
        item_noun: "credential file",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(h) = &ctx.home {
                // The agent's OWN credentials are themselves a reachable secret.
                for rel in [
                    ".claude/.credentials.json",
                    ".config/github-copilot/hosts.json",
                    ".config/github-copilot/apps.json",
                    ".codeium/config.json",
                    ".cursor/credentials.json",
                ] {
                    out.push(Candidate::new(h.join(rel), Extract::Presence));
                }
            }
            out
        },
        remediation: &[
            "An agent that can read its own (or a sibling assistant's) credential file can exfiltrate that token; keep these out of agent-readable scope.",
        ],
    },
    StoreSpec {
        id: "container.registry_auth",
        label: "Container registry credentials (podman/skopeo)",
        item_noun: "registry",
        resolve: |ctx| {
            let mut out = Vec::new();
            // Podman/skopeo/buildah default to $XDG_RUNTIME_DIR or ~/.config.
            if let Some(h) = &ctx.home {
                out.push(Candidate::new(
                    h.join(".config/containers/auth.json"),
                    Extract::Structured(docker_registries),
                ));
            }
            if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
                out.push(Candidate::new(
                    PathBuf::from(rt).join("containers/auth.json"),
                    Extract::Structured(docker_registries),
                ));
            }
            out
        },
        remediation: &[
            "Use a scoped, short-lived registry token per agent; don't expose containers/auth.json.",
        ],
    },
    StoreSpec {
        id: "kube.pod_token",
        label: "In-pod Kubernetes service-account token",
        item_noun: "token",
        resolve: |_ctx| {
            // Present when the agent runs INSIDE a pod — a live cluster credential.
            vec![Candidate::new(
                PathBuf::from("/var/run/secrets/kubernetes.io/serviceaccount/token"),
                Extract::Presence,
            )]
        },
        remediation: &[
            "An agent running in a pod holds the pod's service-account token; scope that ServiceAccount to least privilege and prefer projected, short-lived tokens.",
        ],
    },
    StoreSpec {
        id: "saas_cli.tokens",
        label: "SaaS CLI credentials",
        item_noun: "credential file",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    // Well-known per-CLI token files; presence == a stored token.
                    [
                        ".config/doctl/config.yaml",
                        ".fly/config.yml",
                        ".config/vercel/auth.json",
                        ".local/share/com.vercel.cli/auth.json",
                        ".config/netlify/config.json",
                        ".supabase/access-token",
                        ".sentryclirc",
                        ".config/helm/repositories.yaml",
                        ".circleci/cli.yml",
                        ".config/ngrok/ngrok.yml",
                        ".wrangler/config/default.toml",
                    ]
                    .iter()
                    .map(|rel| Candidate::new(h.join(rel), Extract::Presence))
                    .collect()
                })
                .unwrap_or_default()
        },
        remediation: &[
            "Each reachable SaaS CLI token file is a standing credential for that service; inject scoped, short-lived tokens per task instead.",
        ],
    },
    StoreSpec {
        id: "vpn.credentials",
        label: "VPN private keys / state",
        item_noun: "tunnel",
        resolve: |ctx| {
            let mut out = vec![
                // WireGuard configs embed the interface PrivateKey.
                Candidate::new(PathBuf::from("/etc/wireguard"), Extract::DirCount(is_conf_suffix)),
            ];
            if let Some(h) = &ctx.home {
                out.push(Candidate::new(
                    h.join(".config/wireguard"),
                    Extract::DirCount(is_conf_suffix),
                ));
                // Tailscale node key / auth state.
                out.push(Candidate::new(
                    h.join(".config/tailscale/tailscaled.state"),
                    Extract::Presence,
                ));
            }
            out
        },
        remediation: &[
            "A reachable WireGuard/Tailscale key lets an agent join your private network; keep VPN keys out of agent scope.",
        ],
    },
    StoreSpec {
        id: "jupyter.runtime",
        label: "Jupyter live-server tokens",
        item_noun: "runtime token",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![Candidate::new(
                        h.join(".local/share/jupyter/runtime"),
                        Extract::DirCount(is_json_suffix),
                    )]
                })
                .unwrap_or_default()
        },
        remediation: &[
            "A running Jupyter server's runtime token is remote code execution; don't leave notebook servers reachable to agents.",
        ],
    },
    StoreSpec {
        id: "onepassword.cli",
        label: "1Password CLI session",
        item_noun: "session",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".config/op/config"), Extract::Presence),
                        Candidate::new(h.join(".op/config"), Extract::Presence),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &[
            "A live `op` session can unlock your whole vault; require re-auth and keep ~/.config/op out of agent scope.",
        ],
    },
    StoreSpec {
        id: "cloud_init.user_data",
        label: "Cloud-init instance user-data",
        item_noun: "user-data file",
        resolve: |_ctx| {
            // On cloud VMs this often contains bootstrap secrets in plaintext.
            vec![
                Candidate::new(
                    PathBuf::from("/var/lib/cloud/instance/user-data.txt"),
                    Extract::Presence,
                ),
                Candidate::new(
                    PathBuf::from("/var/lib/cloud/instance/user-data.txt.i"),
                    Extract::Presence,
                ),
            ]
        },
        remediation: &[
            "Cloud-init user-data frequently embeds bootstrap secrets in plaintext; don't bake secrets into user-data, and keep /var/lib/cloud off agent-readable mounts.",
        ],
    },
    // --- Build-tool / package-manager credentials ---
    StoreSpec {
        id: "maven.credentials",
        label: "Maven server credentials",
        item_noun: "credential entry",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![Candidate::new(
                        h.join(".m2/settings.xml"),
                        Extract::Lines(xml_credential_markers),
                    )]
                })
                .unwrap_or_default()
        },
        remediation: &["Use a scoped, short-lived repository token per task; don't expose ~/.m2/settings.xml."],
    },
    StoreSpec {
        id: "gradle.credentials",
        label: "Gradle credentials",
        item_noun: "secret property",
        resolve: |ctx| {
            base_dir(ctx, "GRADLE_USER_HOME", ".gradle")
                .map(|g| {
                    vec![
                        Candidate::new(g.join("gradle.properties"), Extract::Lines(secret_property_keys)),
                        Candidate::new(g.join("gradle.encrypted.properties"), Extract::Presence),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Keep signing keys / publish tokens out of ~/.gradle/gradle.properties; inject per task."],
    },
    StoreSpec {
        id: "composer.auth",
        label: "Composer credentials",
        item_noun: "host",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".composer/auth.json"), Extract::Structured(composer_hosts)),
                        Candidate::new(h.join(".config/composer/auth.json"), Extract::Structured(composer_hosts)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Use scoped Composer tokens per task; don't expose ~/.composer/auth.json."],
    },
    StoreSpec {
        id: "rubygems.credentials",
        label: "RubyGems/Bundler credentials",
        item_noun: "credential",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".gem/credentials"), Extract::Lines(secret_property_keys)),
                        Candidate::new(h.join(".bundle/config"), Extract::Lines(secret_property_keys)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Use a scoped RubyGems API key per task; don't expose ~/.gem/credentials or ~/.bundle/config."],
    },
    StoreSpec {
        id: "pip.config",
        label: "pip index credentials",
        item_noun: "index host",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".config/pip/pip.conf"), Extract::Lines(url_userinfo_hosts)),
                        Candidate::new(h.join(".pip/pip.conf"), Extract::Lines(url_userinfo_hosts)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Don't embed credentials in index-url; use a keyring-backed or per-task token."],
    },
    StoreSpec {
        id: "nuget.config",
        label: "NuGet credentials",
        item_noun: "credential entry",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![Candidate::new(
                        h.join(".nuget/NuGet/NuGet.Config"),
                        Extract::Lines(xml_credential_markers),
                    )]
                })
                .unwrap_or_default()
        },
        remediation: &["Use scoped NuGet API keys per task; avoid ClearTextPassword in NuGet.Config."],
    },
    // --- Data / database tooling ---
    StoreSpec {
        id: "dbt.profiles",
        label: "dbt warehouse credentials",
        item_noun: "profile",
        resolve: |ctx| {
            base_dir(ctx, "DBT_PROFILES_DIR", ".dbt")
                .map(|d| vec![Candidate::new(d.join("profiles.yml"), Extract::Structured(dbt_profiles))])
                .unwrap_or_default()
        },
        remediation: &["~/.dbt/profiles.yml holds warehouse passwords; use scoped, short-lived DB roles per task."],
    },
    StoreSpec {
        id: "databricks.cfg",
        label: "Databricks credentials",
        item_noun: "profile",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| vec![Candidate::new(h.join(".databrickscfg"), Extract::Ini)])
                .unwrap_or_default()
        },
        remediation: &["~/.databrickscfg holds workspace tokens; scope and rotate, keep out of agent scope."],
    },
    StoreSpec {
        id: "snowflake.config",
        label: "Snowflake credentials",
        item_noun: "connection",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".snowsql/config"), Extract::Ini),
                        Candidate::new(h.join(".snowflake/connections.toml"), Extract::Ini),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Use key-pair auth with a scoped Snowflake role per task; don't store passwords in ~/.snowsql."],
    },
    StoreSpec {
        id: "mysql.client",
        label: "MySQL client credentials",
        item_noun: "credential",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".my.cnf"), Extract::Lines(secret_property_keys)),
                        Candidate::new(h.join(".mylogin.cnf"), Extract::Presence),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["~/.my.cnf / ~/.mylogin.cnf hold DB passwords (mylogin.cnf is trivially decryptable); scope DB roles per task."],
    },
    // --- Secrets / identity tooling ---
    StoreSpec {
        id: "sops.age_keys",
        label: "SOPS/age decryption keys",
        item_noun: "key file",
        resolve: |ctx| {
            let mut out = Vec::new();
            if let Some(k) = std::env::var_os("SOPS_AGE_KEY_FILE").filter(|v| !v.is_empty()) {
                out.push(Candidate::new(PathBuf::from(k), Extract::Presence));
            }
            if let Some(h) = &ctx.home {
                out.push(Candidate::new(h.join(".config/sops/age/keys.txt"), Extract::Presence));
                out.push(Candidate::new(h.join(".age"), Extract::DirCount(any_entry)));
            }
            out
        },
        remediation: &["An age/SOPS private key decrypts every secret encrypted to it — keep it out of agent scope, prefer a KMS."],
    },
    StoreSpec {
        id: "teleport.tsh",
        label: "Teleport (tsh) certificates",
        item_noun: "key store",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| vec![Candidate::new(h.join(".tsh/keys"), Extract::DirCount(any_entry))])
                .unwrap_or_default()
        },
        remediation: &["tsh certs grant SSH/k8s/DB/app access; rely on their short TTL and keep ~/.tsh off agent mounts."],
    },
    StoreSpec {
        id: "password_manager.cli",
        label: "Password-manager CLI session/store",
        item_noun: "store",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".config/Bitwarden CLI/data.json"), Extract::Presence),
                        Candidate::new(h.join(".local/share/lpass"), Extract::DirCount(any_entry)),
                        Candidate::new(h.join(".password-store"), Extract::DirCount(is_gpg_suffix)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["A live password-manager session/store can unlock everything; require re-auth and keep it out of agent scope."],
    },
    StoreSpec {
        id: "rclone.config",
        label: "rclone remote credentials",
        item_noun: "remote",
        resolve: |ctx| {
            base_dir(ctx, "RCLONE_CONFIG", ".config/rclone")
                .map(|d| {
                    // RCLONE_CONFIG may point at the file directly.
                    if d.extension().is_some() {
                        vec![Candidate::new(d, Extract::Ini)]
                    } else {
                        vec![Candidate::new(d.join("rclone.conf"), Extract::Ini)]
                    }
                })
                .unwrap_or_default()
        },
        remediation: &["rclone.conf stores cloud-storage creds (often only obscured, not encrypted); keep it out of agent scope."],
    },
    StoreSpec {
        id: "ansible.vault_password",
        label: "Ansible Vault password",
        item_noun: "password file",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".vault_pass"), Extract::Presence),
                        Candidate::new(h.join(".vault-password"), Extract::Presence),
                        Candidate::new(h.join(".vault_pass.txt"), Extract::Presence),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["An Ansible Vault password file decrypts all vaulted secrets; keep it out of agent scope."],
    },
    StoreSpec {
        id: "cloudflared.tunnel",
        label: "Cloudflare tunnel credentials",
        item_noun: "credential",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".cloudflared/cert.pem"), Extract::Presence),
                        Candidate::new(h.join(".cloudflared"), Extract::DirCount(is_json_suffix)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["A cloudflared cert/credential lets an agent stand up tunnels into your network; keep ~/.cloudflared scoped."],
    },
    StoreSpec {
        id: "container.runtime_secrets",
        label: "Container runtime secrets",
        item_noun: "secret",
        resolve: |_ctx| {
            // Docker/Swarm/compose mount secrets here inside containers.
            vec![Candidate::new(PathBuf::from("/run/secrets"), Extract::DirCount(any_entry))]
        },
        remediation: &["Secrets mounted at /run/secrets are readable by anything in the container; scope them to the workload."],
    },
    StoreSpec {
        id: "mail.credentials",
        label: "Mail client credentials",
        item_noun: "credential file",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    [
                        ".msmtprc",
                        ".authinfo",
                        ".authinfo.gpg",
                        ".fetchmailrc",
                        ".mbsyncrc",
                        ".offlineimaprc",
                        ".getmail/getmailrc",
                    ]
                    .iter()
                    .map(|rel| Candidate::new(h.join(rel), Extract::Presence))
                    .collect()
                })
                .unwrap_or_default()
        },
        remediation: &["Mail client rc files store SMTP/IMAP passwords; keep them out of agent scope."],
    },
    StoreSpec {
        id: "cloud_legacy.config",
        label: "Legacy cloud-tool credentials",
        item_noun: "credential file",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    // Home-level (the lateral probe only catches these in sibling repos).
                    [".s3cfg", ".boto", ".dockercfg"]
                        .iter()
                        .map(|rel| Candidate::new(h.join(rel), Extract::Presence))
                        .collect()
                })
                .unwrap_or_default()
        },
        remediation: &["~/.s3cfg, ~/.boto, ~/.dockercfg hold cloud/registry creds in plaintext; migrate to scoped, short-lived tokens."],
    },
    StoreSpec {
        id: "conda.tokens",
        label: "Conda/Anaconda tokens",
        item_noun: "token",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(
                            h.join(".continuum/anaconda-client/tokens"),
                            Extract::DirCount(any_entry),
                        ),
                        Candidate::new(h.join(".condarc"), Extract::Lines(url_userinfo_hosts)),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["Anaconda upload tokens and authed channel URLs grant package-publish/access; keep them out of agent scope."],
    },
    StoreSpec {
        id: "atuin.sync",
        label: "Atuin history-sync key",
        item_noun: "key",
        resolve: |ctx| {
            ctx.home
                .as_ref()
                .map(|h| {
                    vec![
                        Candidate::new(h.join(".local/share/atuin/key"), Extract::Presence),
                        Candidate::new(h.join(".local/share/atuin/session"), Extract::Presence),
                    ]
                })
                .unwrap_or_default()
        },
        remediation: &["The Atuin sync key decrypts your synced shell history (which often contains secrets); keep it out of agent scope."],
    },
];

/// Build one `StoreProbe` per spec. Called by the registry.
pub fn store_probes() -> Vec<Box<dyn Probe>> {
    STORES
        .iter()
        .map(|s| Box::new(StoreProbe(s)) as Box<dyn Probe>)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npmrc_extracts_registry_hosts_not_tokens() {
        let text = "//registry.npmjs.org/:_authToken=npm_SECRETVALUE\n\
                    registry=https://registry.npmjs.org/\n\
                    //npm.pkg.github.com/:_authToken=ghp_x\n\
                    _auth=base64stuff\n";
        let got = npmrc_registries(text);
        assert!(got.contains(&"registry.npmjs.org/".to_string()));
        assert!(got.contains(&"npm.pkg.github.com/".to_string()));
        assert!(got.contains(&"(default registry)".to_string()));
        // No token material.
        assert!(!got.iter().any(|s| s.contains("SECRET") || s.contains("ghp_")));
    }

    #[test]
    fn pgpass_extracts_host_not_password() {
        let text = "# comment\ndb.example.com:5432:app:appuser:s3cr3tpw\n*:*:*:*:wildcardpw\n";
        let got = pgpass_hosts(text);
        assert_eq!(got, vec!["db.example.com"]);
        assert!(!got.iter().any(|s| s.contains("s3cr3t") || s.contains("pw")));
    }

    #[test]
    fn terraformrc_extracts_hosts_not_tokens() {
        let text = "credentials \"app.terraform.io\" {\n  token = \"xxxSECRETxxx\"\n}\n";
        let got = terraformrc_hosts(text);
        assert_eq!(got, vec!["app.terraform.io"]);
    }

    #[test]
    fn kube_extracts_cluster_and_context_names() {
        let yaml = "apiVersion: v1\nclusters:\n- name: prod\n  cluster:\n    server: https://x\n\
                    contexts:\n- name: prod-ctx\n  context: {cluster: prod, user: admin}\n\
                    users:\n- name: admin\n  user:\n    token: SHOULD_NOT_APPEAR\n";
        let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let got = kube_names(&v);
        assert!(got.contains(&"prod".to_string()));
        assert!(got.contains(&"prod-ctx".to_string()));
        assert!(!got.iter().any(|s| s.contains("SHOULD_NOT_APPEAR")));
    }

    #[test]
    fn docker_extracts_registry_keys() {
        let json = r#"{"auths":{"https://index.docker.io/v1/":{"auth":"BASE64SECRET"}},"credsStore":"desktop"}"#;
        let v: serde_yaml::Value = serde_yaml::from_str(json).unwrap();
        let got = docker_registries(&v);
        assert_eq!(got, vec!["https://index.docker.io/v1/"]);
        assert!(!got.iter().any(|s| s.contains("BASE64SECRET")));
    }

    fn fixture_ctx(home: &std::path::Path) -> crate::context::Context {
        crate::context::Context {
            label: crate::context::ContextLabel::Cwd,
            cwd: home.to_path_buf(),
            repo_root: None,
            checkout_root: None,
            home: Some(home.to_path_buf()),
            platform: crate::context::Platform::detect(),
            env: crate::context::EnvSnapshot { vars: Vec::new() },
            git: crate::context::GitContext::default(),
            limits: crate::context::ScanLimits::default(),
            network: crate::context::NetworkPolicy::default(),
            discovery_roots: Vec::new(),
        }
    }

    fn docker_resolve(ctx: &Context) -> Vec<Candidate> {
        vec![Candidate::new(
            ctx.home.as_ref().unwrap().join("config.json"),
            Extract::Structured(docker_registries),
        )]
    }

    const DOCKER_TEST_SPEC: StoreSpec = StoreSpec {
        id: "docker.test",
        label: "Docker registry credentials",
        item_noun: "registry",
        resolve: docker_resolve,
        remediation: &["x"],
    };

    #[test]
    fn structured_store_lists_items_and_hides_values() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"auths":{"registry.internal:5000":{"auth":"TOPSECRETBASE64"}}}"#,
        )
        .unwrap();
        let f = run_store(&DOCKER_TEST_SPEC, &fixture_ctx(tmp.path()));
        assert_eq!(f.severity, Severity::Exposed);
        let rendered = format!("{} {}", f.summary, serde_json::to_string(&f.evidence).unwrap());
        assert!(rendered.contains("registry.internal:5000"));
        assert!(!rendered.contains("TOPSECRETBASE64"));
    }

    #[test]
    fn structured_store_present_without_creds_is_notable() {
        let tmp = tempfile::tempdir().unwrap();
        // A docker config with only a credsStore pointer — present, no auths.
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"credsStore":"desktop"}"#,
        )
        .unwrap();
        let f = run_store(&DOCKER_TEST_SPEC, &fixture_ctx(tmp.path()));
        assert_eq!(f.severity, Severity::Notable);
    }

    #[test]
    fn absent_store_is_info() {
        let tmp = tempfile::tempdir().unwrap();
        let f = run_store(&DOCKER_TEST_SPEC, &fixture_ctx(tmp.path()));
        assert_eq!(f.severity, Severity::Info);
    }

    #[test]
    fn presence_store_exposed_when_file_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        std::fs::write(home.join(".vault-token"), "s.SECRETTOKEN").unwrap();
        let spec = STORES.iter().find(|s| s.id == "vault.token").unwrap();
        let f = run_store(spec, &fixture_ctx(home));
        assert_eq!(f.severity, Severity::Exposed);
        let rendered = serde_json::to_string(&f.evidence).unwrap();
        assert!(!rendered.contains("SECRETTOKEN"));
    }
}
