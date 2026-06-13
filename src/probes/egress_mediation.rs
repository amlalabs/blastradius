//! §extra (findings #11/#12) — egress mediation & cloud-metadata reachability.
//!
//! Two things the bare egress probe doesn't capture:
//!   #11 — whether egress is forced through a filtering proxy (HTTP(S)_PROXY),
//!         and the caveat that the sandbox proxy filters on hostname only (no
//!         TLS inspection unless tlsTerminate), leaving domain-fronting / exfil
//!         to an allowed domain residual.
//!   #12 — in network-allowed mode the sandbox can reach cloud metadata
//!         (169.254.169.254) and the LAN directly. Metadata reachability is an
//!         IMDS-credential exposure surface.
//!
//! Proxy detection is pure env inspection (NAMES + redacted endpoint, never
//! credentials). The metadata reachability check is a SECOND, fixed outbound TCP
//! connect to the link-local IMDS address; like the rest of the scan it ALWAYS
//! runs — there is no flag to disable it. No data is sent and no response read.

use serde_json::json;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::parse::redact_url_userinfo;

pub struct EgressMediationProbe;

/// The well-known link-local cloud metadata endpoint (AWS/Azure/GCP IMDS).
const METADATA_TARGET: &str = "169.254.169.254:80";
const METADATA_TIMEOUT: Duration = Duration::from_millis(1200);

/// Inspect proxy env vars: report presence + a redacted endpoint, never creds.
fn proxy_info() -> (bool, Vec<serde_json::Value>) {
    let mut set = false;
    let mut entries = Vec::new();
    for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
        if let Some(v) = std::env::var_os(key) {
            set = true;
            let raw = v.to_string_lossy().to_string();
            let redacted = redact_url_userinfo(&raw);
            let is_localhost = redacted.contains("localhost")
                || redacted.contains("127.0.0.1")
                || redacted.contains("[::1]");
            let scheme = redacted.split("://").next().unwrap_or("").to_string();
            entries.push(json!({
                "var": key,
                "endpoint_redacted": redacted,
                "scheme": scheme,
                "is_localhost": is_localhost,
            }));
        }
    }
    (set, entries)
}

impl Probe for EgressMediationProbe {
    fn id(&self) -> &'static str {
        "egress.mediation"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Egress
    }

    fn run(&self, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let (proxy_set, proxy_entries) = proxy_info();

        // Cloud-metadata reachability — always probed (a fixed TCP connect).
        let (metadata_checked, metadata_reachable, metadata_latency) = match metadata_connect() {
            Some(ms) => (true, true, Some(ms)),
            None => (true, false, None),
        };

        let (severity, title, confidence) = if metadata_reachable {
            (
                Severity::Exposed,
                "cloud metadata endpoint reachable (IMDS credential surface)",
                Confidence::Confirmed,
            )
        } else if proxy_set {
            (
                Severity::Notable,
                "egress is proxy-mediated (hostname-only filtering)",
                Confidence::Confirmed,
            )
        } else {
            (
                Severity::Info,
                "no egress proxy mediation detected",
                Confidence::Confirmed,
            )
        };

        let summary = if metadata_reachable {
            format!(
                "169.254.169.254 reachable in {} ms — a request-forgery/IMDS path can mint cloud credentials; egress is not namespace-isolated here",
                metadata_latency.unwrap_or(0)
            )
        } else if proxy_set {
            "outbound traffic is routed through a proxy; the sandbox proxy enforces an allowlist on hostname/SNI only (no TLS inspection unless tlsTerminate) — exfil to an allowed domain / domain-fronting is residual".to_string()
        } else {
            "no HTTP(S)_PROXY mediation; if sandboxed via --unshare-net egress is blocked outright, otherwise traffic goes direct".to_string()
        };

        let finding = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Network,
            title,
            severity,
            confidence,
        )
        .summary(summary)
        .evidence(json!({
            "proxy_mediated": proxy_set,
            "proxies": proxy_entries,
            "tls_inspection": "not performed by the default sandbox proxy (hostname/SNI allowlist only); requires tlsTerminate + injected CA",
            "metadata": {
                "checked": metadata_checked,
                "reachable": metadata_reachable,
                "latency_ms": metadata_latency,
                "target": METADATA_TARGET,
                "note": "TCP connect only; no request sent, no credentials retrieved.",
            },
        }))
        .remediation(&[
            "For real containment use a TLS-terminating egress proxy with an allowlist, not hostname-only filtering.",
            "Block link-local 169.254.0.0/16 (IMDS) from agent egress; prefer IMDSv2 with hop-limit 1.",
        ]);

        Ok(vec![finding])
    }
}

/// TCP-connect to the metadata endpoint; returns latency ms on success. No data
/// is sent and no response is read.
fn metadata_connect() -> Option<u64> {
    let addr = METADATA_TARGET.to_socket_addrs().ok()?.next()?;
    let start = Instant::now();
    TcpStream::connect_timeout(&addr, METADATA_TIMEOUT).ok()?;
    Some(start.elapsed().as_millis() as u64)
}
