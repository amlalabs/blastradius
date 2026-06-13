//! §extra — reachable localhost services.
//!
//! An overlooked lateral surface: development machines run datastores and admin
//! UIs bound to `127.0.0.1` with weak or NO authentication, on the assumption
//! that "only I can reach localhost." A coding agent running as you reaches them
//! too — it can read/dump your local Postgres, Redis, Mongo, Elasticsearch, or
//! hit an unauthenticated admin panel.
//!
//! Loopback-only and value-free: this performs a bounded TCP connect to a curated
//! set of well-known service ports on `127.0.0.1` and reports which accepted a
//! connection. NO bytes are sent and no protocol is spoken — it never leaves the
//! machine, so it does not touch the egress promise. Connect attempts are bounded
//! by a short per-port timeout.

use serde_json::json;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};

pub struct LocalServicesProbe;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

/// (port, service label, is_datastore). Datastores often hold data directly and
/// frequently ship with trust-localhost / no-auth defaults.
const SERVICES: &[(u16, &str, bool)] = &[
    (5432, "PostgreSQL", true),
    (3306, "MySQL/MariaDB", true),
    (6379, "Redis", true),
    (27017, "MongoDB", true),
    (9200, "Elasticsearch", true),
    (5984, "CouchDB", true),
    (11211, "Memcached", true),
    (8086, "InfluxDB", true),
    (2379, "etcd", true),
    (7474, "Neo4j", true),
    (9092, "Kafka", true),
    (8200, "Vault", true),
    (5601, "Kibana", false),
    (15672, "RabbitMQ admin", false),
    (9090, "Prometheus", false),
    (3000, "Grafana/dev server", false),
    (8080, "HTTP admin/dev", false),
    (1080, "MinIO/console", false),
];

impl Probe for LocalServicesProbe {
    fn id(&self) -> &'static str {
        "host.local_services"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Process
    }

    fn run(&self, _ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let mut reachable: Vec<serde_json::Value> = Vec::new();
        let mut datastore_hits = 0usize;

        for (port, label, is_ds) in SERVICES {
            let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, *port));
            if TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).is_ok() {
                if *is_ds {
                    datastore_hits += 1;
                }
                reachable.push(json!({ "port": port, "service": label, "datastore": is_ds }));
            }
        }

        let (severity, title, summary) = if datastore_hits > 0 {
            (
                Severity::Exposed,
                "unauthenticated-prone local datastores reachable",
                format!(
                    "{} local service(s) reachable on 127.0.0.1, {} datastore(s) — an agent can read/dump these directly; local datastores often trust localhost with no auth",
                    reachable.len(),
                    datastore_hits
                ),
            )
        } else if !reachable.is_empty() {
            (
                Severity::Notable,
                "local services reachable on loopback",
                format!("{} local service(s) reachable on 127.0.0.1 (admin/dev surfaces)", reachable.len()),
            )
        } else {
            (
                Severity::Info,
                "no well-known local services reachable",
                "none of the probed datastore/admin ports were open on 127.0.0.1".to_string(),
            )
        };

        Ok(vec![Finding::new(
            self.id(),
            self.class(),
            FindingScope::Host,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(summary)
        .evidence(json!({
            "reachable": reachable,
            "datastore_count": datastore_hits,
            "ports_probed": SERVICES.len(),
            "note": "Loopback-only TCP connect; no bytes sent, no protocol spoken — nothing leaves the machine. Reachability is not a claim that a given service lacks auth.",
        }))
        .remediation(&[
            "Don't rely on 'localhost-only' as authentication; require credentials on local datastores or keep them off the agent's network namespace.",
            "Run agents in a network namespace without access to host loopback services.",
        ])])
    }
}
