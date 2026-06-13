//! §12.1 — AWS credentials. INI-parses profile NAMES only. No values, no STS.

use serde_json::json;
use std::path::PathBuf;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::parse::ini_section_names;
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct AwsProbe;

const MAX_AWS_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

impl Probe for AwsProbe {
    fn id(&self) -> &'static str {
        "aws.credentials.profiles"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = ctx.home.clone();

        // Respect AWS_SHARED_CREDENTIALS_FILE / AWS_CONFIG_FILE — but we only
        // have env *names* in the snapshot, so read the real path from the live
        // process env here (path, not secret value).
        let creds_path = std::env::var_os("AWS_SHARED_CREDENTIALS_FILE")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".aws/credentials")));
        let config_path = std::env::var_os("AWS_CONFIG_FILE")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".aws/config")));

        let mut files: Vec<String> = Vec::new();
        let mut skipped_files: Vec<serde_json::Value> = Vec::new();
        let mut profiles: Vec<String> = Vec::new();
        let mut have_creds = false;
        let mut have_config = false;

        if let Some(p) = &creds_path {
            match read_to_string_capped(p, MAX_AWS_CONFIG_BYTES) {
                Ok(text) => {
                    have_creds = true;
                    files.push(shorten(p, home.as_deref()));
                    for name in ini_section_names(&text) {
                        if !profiles.contains(&name) {
                            profiles.push(name);
                        }
                    }
                }
                Err(CappedReadError::NotFound | CappedReadError::NotFile) => {}
                Err(e) => {
                    have_creds = true;
                    files.push(shorten(p, home.as_deref()));
                    skipped_files.push(serde_json::json!({
                        "path": shorten(p, home.as_deref()),
                        "reason": e.reason(),
                    }));
                }
            }
        }
        if let Some(p) = &config_path {
            match read_to_string_capped(p, MAX_AWS_CONFIG_BYTES) {
                Ok(text) => {
                    have_config = true;
                    files.push(shorten(p, home.as_deref()));
                    for name in ini_section_names(&text) {
                        if !profiles.contains(&name) {
                            profiles.push(name);
                        }
                    }
                }
                Err(CappedReadError::NotFound | CappedReadError::NotFile) => {}
                Err(e) => {
                    have_config = true;
                    files.push(shorten(p, home.as_deref()));
                    skipped_files.push(serde_json::json!({
                        "path": shorten(p, home.as_deref()),
                        "reason": e.reason(),
                    }));
                }
            }
        }

        let (severity, title) = if have_creds && !profiles.is_empty() {
            (Severity::Exposed, "AWS credentials reachable")
        } else if have_creds {
            (
                Severity::Notable,
                "AWS credentials file present (not parsed)",
            )
        } else if have_config {
            (
                Severity::Notable,
                "AWS config present (no credentials file)",
            )
        } else {
            (Severity::Info, "no AWS credentials reachable")
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(if profiles.is_empty() && have_creds {
            "AWS credentials/config file present; no profile names parsed".to_string()
        } else if profiles.is_empty() {
            "no AWS profiles found".to_string()
        } else {
            format!("{} profile(s): {}", profiles.len(), profiles.join(", "))
        })
        .evidence(json!({
            "files": files,
            "profile_count": profiles.len(),
            "profiles": profiles,
            "skipped_files": skipped_files,
        }))
        .remediation(&[
            "Use per-agent AWS credentials, narrowly scoped and short-lived.",
            "Don't mount a broad ~/.aws into agent environments.",
        ]);

        Ok(vec![finding])
    }
}
