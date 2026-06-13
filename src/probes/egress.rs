//! §12.11 — egress reachability. ALWAYS runs (no flags, no configuration): a scan
//! resolves a well-known anycast endpoint (1.1.1.1:443) and opens ONE TLS
//! connection to it. Sends NO body and no findings, credentials, paths, env vars,
//! repo names, hostnames, usernames, or machine identifiers. Reports DNS success,
//! the resolved IP, TLS-handshake success, and latency only.

use serde_json::json;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Fixed, non-configurable egress target: a major always-available anycast host.
const EGRESS_HOST: &str = "1.1.1.1";
const EGRESS_PORT: u16 = 443;
const EGRESS_TARGET: &str = "1.1.1.1:443";

#[derive(Debug, Clone)]
struct EgressResult {
    dns_resolved: bool,
    resolved_ips: Vec<String>,
    tls_handshake: bool,
    latency_ms: Option<u64>,
    error: Option<String>,
}

/// DNS resolve → TCP connect → rustls handshake to the fixed target. No data sent.
fn probe_egress() -> EgressResult {
    let mut result = EgressResult {
        dns_resolved: false,
        resolved_ips: Vec::new(),
        tls_handshake: false,
        latency_ms: None,
        error: None,
    };

    let addrs: Vec<_> = match (EGRESS_HOST, EGRESS_PORT).to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(e) => {
            result.error = Some(format!("dns: {e}"));
            return result;
        }
    };
    if addrs.is_empty() {
        result.error = Some("dns resolved to no addresses".to_string());
        return result;
    }
    result.dns_resolved = true;
    result.resolved_ips = addrs.iter().map(|a| a.ip().to_string()).collect();
    result.resolved_ips.dedup();

    let start = Instant::now();
    let mut sock = match TcpStream::connect_timeout(&addrs[0], CONNECT_TIMEOUT) {
        Ok(s) => s,
        Err(e) => {
            result.error = Some(format!("tcp: {e}"));
            result.latency_ms = Some(start.elapsed().as_millis() as u64);
            return result;
        }
    };
    let _ = sock.set_read_timeout(Some(CONNECT_TIMEOUT));
    let _ = sock.set_write_timeout(Some(CONNECT_TIMEOUT));

    // TLS handshake via rustls (ring provider, webpki roots). No app data.
    match tls_handshake(EGRESS_HOST, &mut sock) {
        Ok(()) => {
            result.tls_handshake = true;
            result.latency_ms = Some(start.elapsed().as_millis() as u64);
        }
        Err(e) => {
            result.error = Some(format!("tls: {e}"));
            result.latency_ms = Some(start.elapsed().as_millis() as u64);
        }
    }
    result
}

fn tls_handshake(host: &str, sock: &mut TcpStream) -> Result<(), String> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let server_name =
        rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| e.to_string())?;
    let mut conn =
        rustls::ClientConnection::new(Arc::new(config), server_name).map_err(|e| e.to_string())?;

    // Drive the handshake to completion; send no application data.
    while conn.is_handshaking() {
        match conn.complete_io(sock) {
            Ok((_rd, _wr)) => {
                if conn.is_handshaking() && !conn.wants_read() && !conn.wants_write() {
                    return Err("handshake stalled".to_string());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn finding_from_result(res: EgressResult) -> Finding {
    let severity = if res.tls_handshake {
        Severity::Exposed
    } else if res.dns_resolved {
        Severity::Notable
    } else {
        Severity::Info
    };

    let summary = if res.tls_handshake {
        format!(
            "outbound connectivity reachable — {EGRESS_TARGET}, TLS ok, {} ms",
            res.latency_ms.unwrap_or(0)
        )
    } else if res.dns_resolved {
        format!("DNS resolved but TLS handshake failed for {EGRESS_TARGET}")
    } else {
        format!("outbound connectivity blocked for {EGRESS_TARGET}")
    };

    Finding::new(
        "egress.connectivity",
        FindingClass::Egress,
        FindingScope::Network,
        if res.tls_handshake {
            "outbound network egress reachable"
        } else {
            "outbound network egress not confirmed"
        },
        severity,
        Confidence::Confirmed,
    )
    .summary(summary)
    .evidence(json!({
        "target": EGRESS_TARGET,
        "dns_resolved": res.dns_resolved,
        "resolved_ips": res.resolved_ips,
        "resolved_ip_count": res.resolved_ips.len(),
        "tls_handshake": res.tls_handshake,
        "latency_ms": res.latency_ms,
        "note": "No findings were sent. The remote necessarily observed source IP and timestamp.",
    }))
    .remediation(&[
        "Egress control: default-deny outbound, then allowlist only what the task needs.",
    ])
}

pub struct EgressProbe;

impl Probe for EgressProbe {
    fn id(&self) -> &'static str {
        "egress.connectivity"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Egress
    }

    fn run(&self, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        Ok(vec![finding_from_result(probe_egress())])
    }
}
