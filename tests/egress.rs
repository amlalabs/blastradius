//! Egress tests. Unit-level test uses a mock connector (no network). The real
//! network test is opt-in behind `--features network-tests` (§18).

use std::time::Duration;

use blastradius::probes::egress::{EgressConnector, EgressResult};

/// A mock connector that never touches the network.
struct MockConnector {
    result: EgressResult,
}

impl EgressConnector for MockConnector {
    fn connect(&self, _target: &str, _timeout: Duration) -> EgressResult {
        self.result.clone()
    }
}

#[test]
fn mock_connector_reports_handshake() {
    let mock = MockConnector {
        result: EgressResult {
            dns_resolved: true,
            resolved_ips: vec!["1.1.1.1".into()],
            tls_handshake: true,
            latency_ms: Some(19),
            error: None,
        },
    };
    let r = mock.connect("1.1.1.1:443", Duration::from_secs(1));
    assert!(r.dns_resolved && r.tls_handshake);
    assert_eq!(r.latency_ms, Some(19));
}

#[cfg(feature = "network-tests")]
#[test]
fn real_egress_connects() {
    use blastradius::probes::egress::RealConnector;
    let r = RealConnector.connect("1.1.1.1:443", Duration::from_secs(5));
    assert!(r.dns_resolved, "DNS should resolve");
    assert!(r.tls_handshake, "TLS handshake to 1.1.1.1 should succeed");
}
