//! The probe registry — the single source of truth for what runs in a scan.
//!
//! ## Adding a new detection
//!
//! There are two ways, chosen by shape:
//!
//! 1. **A credential/config file store** (the common case — "a file on disk holds
//!    creds; report the names/counts, never the values"): add a [`StoreSpec`]
//!    entry to `store::STORES`. No new file, no `Probe` impl, no edit here.
//!    [`store::store_probes`] turns each spec into a probe automatically.
//!    See [`crate::probes::store`].
//!
//! 2. **Bespoke logic** (sockets, permission inference, protocol probes): create a
//!    `Probe` impl in its own module and add ONE line to [`bespoke_probes`] below,
//!    in the group matching its `FindingClass`.
//!
//! Execution order here is for readability only — `runner::run_all` re-sorts all
//! findings deterministically by class/severity/id, so order never affects output.
//! Keeping this list explicit (rather than link-time auto-registration) is
//! deliberate: for a security tool, a visible inventory of everything that
//! executes is worth more than save-a-line magic.

use crate::probes;
use crate::runner::Probe;

/// Every probe that a scan runs: the spec-driven credential stores followed by
/// the bespoke probes.
pub fn all() -> Vec<Box<dyn Probe>> {
    let mut probes = bespoke_probes();
    // Credential-store probes are data-driven; append the whole family.
    probes.extend(probes::store::store_probes());
    probes
}

/// Probes with custom logic, grouped by finding class for readability.
fn bespoke_probes() -> Vec<Box<dyn Probe>> {
    vec![
        // ---- Credentials ----
        Box::new(probes::aws::AwsProbe),
        Box::new(probes::ssh::SshProbe),
        Box::new(probes::ssh_agent::SshAgentProbe),
        Box::new(probes::github::GithubProbe),
        Box::new(probes::git_credentials::GitCredentialsProbe),
        Box::new(probes::env::EnvProbe),
        Box::new(probes::env_scrub::EnvScrubProbe),
        Box::new(probes::shell_history::ShellHistoryProbe),
        Box::new(probes::repl_history::ReplHistoryProbe),
        Box::new(probes::browser_stores::BrowserStoresProbe),
        // ---- Cross-repo ----
        Box::new(probes::dotenv::DotenvProbe),
        Box::new(probes::sibling_repos::SiblingReposProbe),
        Box::new(probes::lateral_secrets::LateralSecretsProbe),
        // ---- Git write ----
        Box::new(probes::git_write::GitWriteProbe),
        Box::new(probes::git_config::GitConfigProbe),
        // ---- Egress ----
        Box::new(probes::egress::EgressProbe),
        Box::new(probes::egress_mediation::EgressMediationProbe),
        // ---- Process ----
        Box::new(probes::sandbox_reach::SandboxReachProbe),
        Box::new(probes::sandbox_detect::SandboxDetectProbe),
        Box::new(probes::privilege::PrivilegeProbe),
        Box::new(probes::privileged_reach::PrivilegedReachProbe),
        Box::new(probes::process_introspect::ProcessIntrospectProbe),
        Box::new(probes::local_services::LocalServicesProbe),
        // ---- Host persistence ----
        Box::new(probes::write_reach::WriteReachProbe),
        Box::new(probes::deferred_exec_sinks::DeferredExecSinksProbe),
        Box::new(probes::sandbox_integrity::SandboxIntegrityProbe),
        Box::new(probes::claude_surface::ClaudeSurfaceProbe),
        Box::new(probes::network_config::NetworkConfigProbe),
        // ---- System info ----
        Box::new(probes::sandbox_posture::SandboxPostureProbe),
    ]
}
