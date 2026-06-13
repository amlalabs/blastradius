//! §12.9 — lateral secret reach. Counts secret-like files in sibling repos.
//! Counts only; never reads values.

use serde_json::json;
use walkdir::WalkDir;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::probes::dotenv::{is_dotenv_file, scan_dir_for_dotenvs};
use crate::probes::sibling_repos;
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::is_ignored_dir;

pub struct LateralSecretsProbe;

/// Whether a filename is a key-like / credential file (§12.9). Excludes examples.
fn is_key_like(name: &str) -> bool {
    for ex in [".example", ".sample", ".template", ".defaults"] {
        if name.ends_with(ex) {
            return false;
        }
    }
    name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".jks")
        || name.ends_with(".keystore")
        || name.ends_with(".ppk")
        || name.ends_with(".ovpn")
        // Plaintext-secret files that routinely sit checked into repos.
        || name.ends_with(".tfstate")
        || name.ends_with(".tfstate.backup")
        || name == "id_rsa"
        || name == "id_ed25519"
        || name == "id_ecdsa"
        || name == "id_dsa"
        || name == "kubeconfig"
        || name == ".kubeconfig"
        || name == "credentials.json"
        || name == "master.key"            // Rails (decrypts credentials.yml.enc)
        || name == "credentials.yml.enc"
        || name == ".s3cfg"                 // s3cmd
        || name == ".boto"                  // gsutil/boto
        || name == ".dockercfg"             // legacy docker auth
        || name == ".pgpass"
        || (name.starts_with("service-account") && name.ends_with(".json"))
}

impl Probe for LateralSecretsProbe {
    fn id(&self) -> &'static str {
        "cross_repo.lateral_secrets"
    }
    fn class(&self) -> FindingClass {
        FindingClass::CrossRepo
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let siblings = sibling_repos::enumerate(ctx);

        let mut repos_with_secrets = 0usize;
        let mut dotenv_files = 0usize;
        let mut dotenv_keys = 0usize;
        let mut key_like_files = 0usize;

        for repo in &siblings {
            let mut repo_has_secret = false;

            // .env files + keys (reuse the dotenv scanner).
            let scan = scan_dir_for_dotenvs(repo, &ctx.limits);
            if scan.file_count > 0 {
                repo_has_secret = true;
                dotenv_files += scan.file_count;
                dotenv_keys += scan.key_count;
            }

            // key-like files.
            let mut examined = 0usize;
            let walker = WalkDir::new(repo)
                .max_depth(ctx.limits.max_depth_home_roots)
                .follow_links(ctx.limits.follow_symlinks)
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
                examined += 1;
                if examined > ctx.limits.max_files_examined_per_repo {
                    break;
                }
                let name = entry.file_name().to_string_lossy();
                if is_key_like(&name) {
                    key_like_files += 1;
                    repo_has_secret = true;
                } else if is_dotenv_file(&name) {
                    // already counted above; ensures repo flagged even if read failed
                    repo_has_secret = true;
                }
            }

            if repo_has_secret {
                repos_with_secrets += 1;
            }
        }

        let severity = if repos_with_secrets > 0 {
            Severity::Exposed
        } else if !siblings.is_empty() {
            Severity::Notable
        } else {
            Severity::Info
        };

        let summary = if repos_with_secrets > 0 {
            format!("secret-like files present in {repos_with_secrets} sibling repo(s)")
        } else if !siblings.is_empty() {
            "sibling repos reachable, no secret-like files found".to_string()
        } else {
            "no sibling repos to inspect".to_string()
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::SiblingRepos,
            if repos_with_secrets > 0 {
                "lateral secrets reachable in sibling repos"
            } else {
                "no lateral secrets in sibling repos"
            },
            severity,
            Confidence::Confirmed,
        )
        .summary(summary)
        .evidence(json!({
            "repos_with_secret_like_files": repos_with_secrets,
            "dotenv_files": dotenv_files,
            "dotenv_keys": dotenv_keys,
            "key_like_files": key_like_files,
        }))
        .remediation(&[
            "Filesystem isolation: an agent should not see credentials belonging to other projects.",
        ]);

        Ok(vec![finding])
    }
}
