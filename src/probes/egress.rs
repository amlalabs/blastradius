//! §12.11 — egress. Resolves a well-known host + opens ONE TLS connection to a
//! major anycast endpoint (default 1.1.1.1:443). Sends NO body, no identifiers.
//! Reports DNS success, resolved IP, TLS-handshake success, and latency only.

use serde_json::json;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::context::{Context, NetworkPolicy};
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::net::parse_host_port_target;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct EgressResult {
    pub dns_resolved: bool,
    pub resolved_ips: Vec<String>,
    pub tls_handshake: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Mockable egress connector (§18 — unit tests never touch the network).
pub trait EgressConnector {
    fn connect(&self, target: &str, timeout: Duration) -> EgressResult;
}

/// Real connector: DNS resolve → TCP connect → rustls handshake. No data sent.
pub struct RealConnector;

impl EgressConnector for RealConnector {
    fn connect(&self, target: &str, timeout: Duration) -> EgressResult {
        let mut result = EgressResult {
            dns_resolved: false,
            resolved_ips: Vec::new(),
            tls_handshake: false,
            latency_ms: None,
            error: None,
        };

        let target = match parse_host_port_target(target) {
            Ok(target) => target,
            Err(_) => {
                result.error = Some("invalid egress target".to_string());
                return result;
            }
        };
        let host = target.host;
        let port = target.port;

        // DNS resolution.
        let addrs: Vec<_> = match (host.as_str(), port).to_socket_addrs() {
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
        let addr = addrs[0];
        let mut sock = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => s,
            Err(e) => {
                result.error = Some(format!("tcp: {e}"));
                result.latency_ms = Some(start.elapsed().as_millis() as u64);
                return result;
            }
        };
        let _ = sock.set_read_timeout(Some(timeout));
        let _ = sock.set_write_timeout(Some(timeout));

        // TLS handshake via rustls (ring provider, webpki roots). No app data.
        match tls_handshake(&host, &mut sock) {
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

fn egress_target_kind(target: &str) -> &'static str {
    if target == NetworkPolicy::default().egress_target {
        "default"
    } else {
        "custom"
    }
}

fn egress_target_display(target: &str) -> String {
    if egress_target_kind(target) == "default" {
        target.to_string()
    } else {
        "[custom egress target]".to_string()
    }
}

fn resolved_ips_for_report(target: &str, ips: &[String]) -> Vec<String> {
    if egress_target_kind(target) == "default" {
        ips.to_vec()
    } else {
        Vec::new()
    }
}

fn finding_from_result(ctx: &Context, res: EgressResult) -> Finding {
    let target_display = egress_target_display(&ctx.network.egress_target);

    let severity = if res.tls_handshake {
        Severity::Exposed
    } else if res.dns_resolved {
        Severity::Notable
    } else {
        Severity::Info
    };

    let summary = if res.tls_handshake {
        format!(
            "outbound connectivity reachable — {}, TLS ok, {} ms",
            target_display,
            res.latency_ms.unwrap_or(0)
        )
    } else if res.dns_resolved {
        format!("DNS resolved but TLS handshake failed for {target_display}")
    } else {
        format!("outbound connectivity blocked for {target_display}")
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
        "target": target_display,
        "target_kind": egress_target_kind(&ctx.network.egress_target),
        "dns_resolved": res.dns_resolved,
        "resolved_ips": resolved_ips_for_report(&ctx.network.egress_target, &res.resolved_ips),
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

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        if ctx.network.offline || !ctx.network.egress_enabled {
            return Ok(vec![Finding::new(
                self.id(),
                self.class(),
                FindingScope::Network,
                "egress probe disabled",
                Severity::Info,
                Confidence::Confirmed,
            )
            .summary("network egress check disabled (--no-egress/--offline)")
            .evidence(json!({ "enabled": false }))]);
        }

        let connector = RealConnector;
        let res = connector.connect(&ctx.network.egress_target, CONNECT_TIMEOUT);
        Ok(vec![finding_from_result(ctx, res)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_egress_targets_are_redacted_for_reports() {
        let target = "secret-host.example:443";
        assert_eq!(egress_target_kind(target), "custom");
        assert_eq!(egress_target_display(target), "[custom egress target]");
        assert_eq!(
            resolved_ips_for_report(target, &["10.1.2.3".to_string()]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn default_egress_target_keeps_report_detail() {
        let target = NetworkPolicy::default().egress_target;
        assert_eq!(egress_target_kind(&target), "default");
        assert_eq!(egress_target_display(&target), "1.1.1.1:443");
        assert_eq!(
            resolved_ips_for_report(&target, &["1.1.1.1".to_string()]),
            vec!["1.1.1.1".to_string()]
        );
    }

    #[test]
    fn custom_egress_finding_omits_target_and_resolved_ips() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = Context::build(
            crate::context::ContextLabel::Cwd,
            tmp.path().to_path_buf(),
            crate::context::ScanLimits::default(),
            NetworkPolicy {
                egress_enabled: true,
                offline: false,
                egress_target: "secret-host.example:443".to_string(),
                ..NetworkPolicy::default()
            },
        );
        ctx.git = crate::context::GitContext::default();
        let finding = finding_from_result(
            &ctx,
            EgressResult {
                dns_resolved: true,
                resolved_ips: vec!["10.1.2.3".to_string()],
                tls_handshake: true,
                latency_ms: Some(7),
                error: None,
            },
        );

        let rendered = format!(
            "{} {}",
            finding.summary,
            serde_json::to_string(&finding.evidence).unwrap()
        );
        assert!(!rendered.contains("secret-host.example"));
        assert!(!rendered.contains("10.1.2.3"));
        assert!(rendered.contains("[custom egress target]"));
        assert_eq!(finding.evidence["target_kind"], "custom");
        assert_eq!(finding.evidence["resolved_ip_count"], 1);
    }
}
