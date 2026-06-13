//! Local web dashboard (`blastradius dashboard`).
//!
//! Runs a scan, optionally generates an AI blast-radius analysis, and serves a
//! cinematic "blast radius" storytelling page. It binds `0.0.0.0:5321` by
//! default (network-reachable, no auth — a loud warning prints on any
//! non-loopback bind; pass `--bind 127.0.0.1` for loopback-only). The page loads
//! its UI runtime (React/Babel) and webfonts from a CDN for rendering — those
//! requests carry no scan data. The data the page shows is the value-free
//! finding inventory; it is embedded inline and swept through the Layer-2
//! redaction pass before it is ever written to a socket, so secret values never
//! leave the machine (§4.2).
//!
//! What is live vs. illustrative: the live scan drives the reachable-surface
//! tallies, the per-ring finding chips in the expanding-radius section, and the
//! full inventory. The §24 retro-hazard section is driven by the user's REAL
//! discovered agent transcripts — always (the value-free `HistoryAuditReport`,
//! over all agents and all time). The radius/constellation node
//! *geometry* uses a fixed illustrative layout, and the §23 benign-vs-risky
//! per-session score climax is an illustrative teaching fixture — both clearly
//! labeled as such on the page.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use crate::analyze::Analysis;
use crate::report::redaction::sweep;
use crate::report::RunReport;
use crate::session::history::HistoryAuditReport;

mod impact;
mod page;

/// Options controlling how the dashboard is served.
pub struct ServeOptions {
    pub port: u16,
    /// Address to bind (e.g. "127.0.0.1" or "0.0.0.0").
    pub bind: String,
    pub open_browser: bool,
    /// AI analysis result, or an error string if `--ai` was requested but failed.
    pub analysis: Result<Option<Analysis>, String>,
    /// Real retro-hazard report (always built from discovered transcripts). `None`
    /// only when no command supplies one; an empty report (no transcripts found)
    /// makes the page fall back to the illustrative fixture. Value-free by
    /// construction; flows through the Layer-2 sweep.
    pub history: Option<HistoryAuditReport>,
    /// Top-N ranked real session cards (value-free) for the session-score section;
    /// `None` falls back to the illustrative benign/risky fixture.
    pub sessions: Option<Value>,
}

/// The six radius rings, in FIXED outward order. The page lays them out by this
/// order; live data only fills in per-ring findings (the denominator).
const RING_ORDER: [&str; 6] = ["shell", "identity", "cloud", "neighbors", "network", "host"];

/// Sub-classify a reachable `Credentials` finding into a ring by its id (§ ring map).
fn cred_ring(id: &str) -> &'static str {
    const CLOUD_PREFIXES: [&str; 17] = [
        "aws.",
        "gcp.",
        "azure.",
        "kube.",
        "docker.",
        "container.",
        "databricks.",
        "dbt.",
        "snowflake.",
        "terraform.",
        "app.terraform.io",
        "conda.",
        "cloudflared.",
        "rclone.",
        "vault.",
        "cloud_init.",
        "cloud_legacy.",
    ];
    if CLOUD_PREFIXES.iter().any(|p| id.starts_with(p)) {
        return "cloud";
    }
    const SHELL_IDS: [&str; 4] = [
        "env.secret_names",
        "env.subprocess_scrub",
        "credentials.shell_history",
        "credentials.repl_history",
    ];
    if SHELL_IDS.contains(&id) || id.starts_with("cross_repo.dotenv") || id.starts_with("atuin.") {
        return "shell";
    }
    "identity"
}

/// Map a reachable finding to its ring id. EXHAUSTIVE over scope (first) then class.
fn ring_of(f: &crate::finding::Finding) -> &'static str {
    use crate::finding::{FindingClass, FindingScope};
    // Scope takes precedence for the cross-machine rings.
    match f.scope {
        FindingScope::SiblingRepos => return "neighbors",
        FindingScope::Network => return "network",
        FindingScope::Host => return "host",
        FindingScope::Ambient | FindingScope::CurrentRepo => {}
    }
    match f.class {
        FindingClass::CrossRepo => "neighbors",
        FindingClass::GitWrite => "network",
        FindingClass::Egress => {
            if f.id == "egress.mediation" {
                "cloud"
            } else {
                "network"
            }
        }
        FindingClass::Process => "host",
        FindingClass::HostPersistence => "host",
        FindingClass::SystemInfo => "host",
        FindingClass::Credentials => cred_ring(&f.id),
    }
}

/// Design copy (label, blurb) for each ring id.
fn ring_meta(id: &str) -> (&'static str, &'static str) {
    match id {
        "shell" => ("This shell", "The environment the agent was handed."),
        "identity" => ("Your identity", "The keys and tokens that say you are you."),
        "cloud" => ("The cloud", "Provider identities mounted into your shell."),
        "neighbors" => (
            "Neighboring repos",
            "Everything else sitting next to the task on disk.",
        ),
        "network" => (
            "The network",
            "Where data could go, and where code could land.",
        ),
        "host" => ("The whole machine", "Beyond the task: the box itself."),
        _ => ("", ""),
    }
}

/// Build the value-free top-N ranked session cards for the session-score section.
/// Each card is the real `SessionReport` (serialized — value-free by contract,
/// §23.9) augmented with `rank`, a value-free `label`, and `touched` (the
/// finding ids the scored reasons point at, for constellation lighting). The
/// decomposed `reasons[]` (signal + weight + finding_ref + evidence) and
/// `toxic_combinations[]` ARE the illustrative "how this transcript is risky".
pub fn session_cards(reports: &[crate::session::report::SessionReport]) -> Value {
    let cards: Vec<Value> = reports
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut touched: Vec<String> =
                r.reasons.iter().filter_map(|x| x.finding_ref.clone()).collect();
            touched.sort();
            touched.dedup();
            let label = format!(
                "{} · {}",
                r.agent,
                r.session_id.chars().take(8).collect::<String>()
            );

            // Aggregate the per-event reasons into distinct "how it's risky" rows:
            // group by (signal, finding_ref), count occurrences, sum the weight, and
            // keep one value-free evidence sample. This is the illustrative breakdown
            // of exactly how the transcript earned its score — not 100 identical rows.
            use std::collections::BTreeMap;
            let mut order: Vec<(String, Option<String>)> = Vec::new();
            let mut agg: BTreeMap<(String, Option<String>), (i64, usize, String)> = BTreeMap::new();
            let mut weight_total: i64 = 0;
            for rs in &r.reasons {
                weight_total += rs.weight as i64;
                let key = (rs.signal.clone(), rs.finding_ref.clone());
                let sample = rs.evidence.first().cloned().unwrap_or_default();
                let e = agg.entry(key.clone()).or_insert_with(|| {
                    order.push(key.clone());
                    (0, 0, sample.clone())
                });
                e.0 += rs.weight as i64;
                e.1 += 1;
            }
            let mut how: Vec<Value> = order
                .iter()
                .map(|k| {
                    let (total, count, sample) = &agg[k];
                    json!({
                        "signal": k.0,
                        "finding_ref": k.1,
                        "count": count,
                        "weight_total": total,
                        "evidence": sample,
                        "why": impact::signal_impact(&k.0).unwrap_or(""),
                    })
                })
                .collect();
            // Heaviest contributions first — the sharpest "how" at the top.
            how.sort_by(|a, b| {
                b["weight_total"].as_i64().unwrap_or(0).cmp(&a["weight_total"].as_i64().unwrap_or(0))
            });

            let mut card = serde_json::to_value(r).unwrap_or(Value::Null);
            if let Some(obj) = card.as_object_mut() {
                obj.insert("rank".into(), json!(i + 1));
                obj.insert("label".into(), json!(label));
                obj.insert("touched".into(), json!(touched));
                obj.insert("how".into(), json!(how));
                obj.insert("weight_total".into(), json!(weight_total));
            }
            card
        })
        .collect();
    json!({ "ranked": cards })
}

/// Build the value-free dashboard JSON from a report (+ optional AI analysis,
/// the retro `HistoryAuditReport`, and the ranked top-N session cards).
pub fn build_data(
    report: &RunReport,
    analysis: &Result<Option<Analysis>, String>,
    history: Option<&HistoryAuditReport>,
    sessions: Option<&Value>,
) -> Value {
    // The dashboard reflects the first (primary) context.
    let cr = report.contexts.first();
    let platform = format!("{:?}", report.platform);

    let mut verdict: Option<String> = None;
    let mut findings_json: Vec<Value> = Vec::new();
    let mut classes: Vec<(crate::finding::FindingClass, usize, usize)> = Vec::new();
    let (mut exposed, mut notable) = (0usize, 0usize);

    // Live-radius accumulator (the reachable-surface denominator nodes).
    let mut ring_findings: std::collections::HashMap<&'static str, Vec<Value>> =
        std::collections::HashMap::new();

    if let Some(cr) = cr {
        for f in &cr.findings {
            if f.id == "process.sandbox_detect" {
                verdict = f
                    .evidence
                    .get("verdict")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
            let reachable = f.severity.rank() >= crate::severity::Severity::Notable.rank();
            match f.severity {
                crate::severity::Severity::Exposed => exposed += 1,
                crate::severity::Severity::Notable => notable += 1,
                _ => {}
            }
            if reachable {
                if let Some(e) = classes.iter_mut().find(|(c, _, _)| *c == f.class) {
                    e.1 += 1;
                    if matches!(f.severity, crate::severity::Severity::Exposed) {
                        e.2 += 1;
                    }
                } else {
                    classes.push((
                        f.class,
                        1,
                        usize::from(matches!(f.severity, crate::severity::Severity::Exposed)),
                    ));
                }
            }

            // Finding-impact teaching copy: prefer the curated per-id (why, how);
            // fall back to the per-FindingClass copy keyed by class.to_string().
            let class_str = f.class.to_string();
            let (why, how) = impact::finding_impact(&f.id)
                .unwrap_or_else(|| impact::finding_impact_class(&class_str));

            if reachable {
                // Live radius: value-free per-ring node (paths/scope/labels only).
                let detail = format!("{} · {}", f.confidence.label(), f.scope);
                ring_findings.entry(ring_of(f)).or_default().push(json!({
                    "id": f.id,
                    "title": f.title,
                    "severity": f.severity.label(),
                    "metric": f.summary,
                    "detail": [detail],
                    "class": class_str,
                    "remediation": f.remediation,
                    "why": why,
                    "how": how,
                }));
            }
            findings_json.push(json!({
                "id": f.id,
                "class": class_str,
                "classLabel": f.class.section_title(),
                "scope": f.scope.to_string(),
                "title": f.title,
                "summary": f.summary,
                "severity": f.severity.label(),
                "confidence": f.confidence.label(),
                "reachable": reachable,
                "remediation": f.remediation,
                "why": why,
                "how": how,
            }));
        }
    }
    classes.sort_by_key(|(c, _, _)| c.order());

    let class_tiles: Vec<Value> = classes
        .iter()
        .map(|(c, n, ex)| {
            json!({ "class": c.to_string(), "label": c.section_title(), "count": n, "exposed": ex })
        })
        .collect();

    // Emit the six rings in FIXED outward order; empty rings carry n:0.
    let rings: Vec<Value> = RING_ORDER
        .iter()
        .map(|&id| {
            let (label, blurb) = ring_meta(id);
            let findings = ring_findings.remove(id).unwrap_or_default();
            json!({
                "id": id,
                "label": label,
                "blurb": blurb,
                "n": findings.len(),
                "findings": findings,
            })
        })
        .collect();

    let ai = match analysis {
        Ok(Some(a)) => json!({
            "enabled": true,
            "model": a.model,
            "overall": a.overall,
            "scenarios": a.scenarios,
            "error": Value::Null,
        }),
        Ok(None) => json!({ "enabled": false, "error": Value::Null, "scenarios": [] }),
        Err(e) => json!({ "enabled": true, "error": e, "scenarios": [] }),
    };

    json!({
        "tool": { "name": "blastradius", "version": report.version },
        "generated": report.timestamp,
        "platform": platform,
        "verdict": verdict,
        "stats": {
            "exposed": exposed,
            "notable": notable,
            "total": findings_json.len(),
            "classes": class_tiles,
            // Documented capability breadth (§23.1/§23.15): the tool runs
            // ~35 probes across ~30 credential stores. This is a tool-breadth
            // figure for the constellation caption, not a per-scan finding count.
            // Orientation must match the page fallback (probes >= stores) so the
            // "~N probes · ~M credential stores" caption stays coherent.
            "breadth": { "probes": 35, "stores": 30 },
        },
        "rings": rings,
        "findings": findings_json,
        "ai": ai,
        // §24.6 retro section: the real, value-free retro report (always served
        // when present); null/empty falls back to the labeled illustrative fixture.
        "history": match history {
            Some(h) => serde_json::to_value(h).unwrap_or(Value::Null),
            None => Value::Null,
        },
        // §23 session-score section: the top-N ranked real sessions (value-free
        // cards). Null falls back to the labeled illustrative benign/risky fixture.
        "sessions": match sessions {
            Some(s) => s.clone(),
            None => Value::Null,
        },
    })
}

/// Render the full HTML page with the data embedded, swept for secret shapes.
pub(crate) fn render_html(data: &Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    // Guard against `</script>` breaking out of the embedded JSON block.
    let data_str = data_str.replace("</", "<\\/");
    let html = page::PAGE.replace("/*__BR_DATA__*/", &data_str);
    // Defense in depth: the data is value-free, but sweep the final bytes anyway.
    sweep(&html)
}

/// Serve the dashboard until interrupted (Ctrl-C).
pub fn serve(report: &RunReport, opts: ServeOptions) -> Result<()> {
    let data = build_data(report, &opts.analysis, opts.history.as_ref(), opts.sessions.as_ref());
    let html = render_html(&data);

    let listener = TcpListener::bind((opts.bind.as_str(), opts.port))
        .with_context(|| format!("binding {}:{}", opts.bind, opts.port))?;
    let port = listener.local_addr()?.port();

    let loopback = matches!(opts.bind.as_str(), "127.0.0.1" | "::1" | "localhost");
    // 0.0.0.0 / :: aren't browser-connectable; open loopback locally.
    let browse_host = if loopback { opts.bind.clone() } else { "127.0.0.1".to_string() };
    let url = format!("http://{browse_host}:{port}");

    if loopback {
        println!("\n  ▸ blastradius dashboard live at {url}");
        println!("    (findings stay local · value-free · UI assets via CDN · Ctrl-C to stop)\n");
    } else {
        println!("\n  ▸ blastradius dashboard live at http://{}:{port} (and {url} locally)", opts.bind);
        eprintln!(
            "  ⚠ SECURITY: bound to {}:{port} — reachable by ANYONE on your network, with NO authentication.",
            opts.bind
        );
        eprintln!("    The page shows your full reachable-credential inventory, escalation paths, and");
        eprintln!("    post-root blast radius. Only do this on a trusted network; use --bind 127.0.0.1");
        eprintln!("    to restrict to loopback. Ctrl-C to stop.");
        // §24.6: Tab 4 publishes which still-reachable credentials an agent
        // actually read — a precise LAN targeting map — so name that exposure
        // explicitly when real history is served on a non-loopback bind.
        if let Some(h) = opts.history.as_ref() {
            let live = h
                .hazards
                .iter()
                .filter(|hz| {
                    matches!(hz.status, crate::session::retro::HazardStatus::StillReachable)
                })
                .count();
            eprintln!(
                "    ⚠ retro history: the page publishes {live} STILL-REACHABLE realized hazard(s) — a precise",
            );
            eprintln!("      map of which reachable credentials your agents already read.");
            eprintln!("      Loopback (--bind 127.0.0.1) strongly advised on an untrusted network.");
        }
        eprintln!();
    }

    if opts.open_browser {
        open_browser(&url);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                // Each connection is cheap; handle inline. Errors are per-client.
                let _ = handle(s, &html);
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

/// Handle one HTTP/1.1 request: serve the page on `/`, `204` on favicon, `404`
/// otherwise. We read (and discard) the request head, then write one response.
fn handle(mut stream: TcpStream, html: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    // Read the request line + headers (up to the blank line), bounded.
    let mut buf = [0u8; 4096];
    let mut head = Vec::new();
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        head.extend_from_slice(&buf[..n]);
        if head.windows(4).any(|w| w == b"\r\n\r\n") || head.len() > 16 * 1024 {
            break;
        }
    }
    let request_line = String::from_utf8_lossy(&head);
    let path = request_line
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, content_type, body): (&str, &str, &[u8]) = if path == "/" || path.starts_with("/?")
    {
        ("200 OK", "text/html; charset=utf-8", html.as_bytes())
    } else if path == "/favicon.ico" {
        ("204 No Content", "image/x-icon", b"")
    } else {
        ("404 Not Found", "text/plain; charset=utf-8", b"not found")
    };

    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// Best-effort: open the default browser. Never fails the run.
fn open_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{Context, ContextLabel, ScanLimits, ScanOptions};

    fn dummy_report() -> RunReport {
        let ctx = Context::build(
            ContextLabel::Cwd,
            std::env::temp_dir(),
            ScanLimits::default(),
            ScanOptions::default(),
        );
        RunReport {
            mode: "dashboard".into(),
            timestamp: "2026-06-12T00:00:00Z".into(),
            version: "test".into(),
            platform: ctx.platform,
            command: "blastradius dashboard".into(),
            contexts: vec![crate::report::ContextReport {
                context: ctx,
                findings: vec![crate::finding::Finding::new(
                    "aws.credentials.profiles",
                    crate::finding::FindingClass::Credentials,
                    crate::finding::FindingScope::Ambient,
                    "AWS credentials reachable",
                    crate::severity::Severity::Exposed,
                    crate::severity::Confidence::Confirmed,
                )
                .summary("2 profiles")],
            }],
            comparison: None,
        }
    }

    #[test]
    fn html_embeds_data_and_is_swept() {
        let report = dummy_report();
        let data = build_data(&report, &Ok(None), None, None);
        let html = render_html(&data);
        assert!(html.contains("AWS credentials reachable"));
        assert!(html.contains("<!doctype html>") || html.contains("<!DOCTYPE html>"));
        // No secret-shaped content.
        assert!(!crate::report::redaction::contains_secret_shaped(&html));
    }

    #[test]
    fn build_data_counts_reachable_classes() {
        let report = dummy_report();
        let data = build_data(&report, &Ok(None), None, None);
        assert_eq!(data["stats"]["exposed"], 1);
        assert_eq!(data["stats"]["classes"][0]["label"], "CREDENTIALS");
    }

    #[test]
    fn build_data_emits_six_rings() {
        let report = dummy_report();
        let data = build_data(&report, &Ok(None), None, None);
        let rings = data["rings"].as_array().expect("rings is an array");
        assert_eq!(rings.len(), 6, "exactly six rings");
        let ids: Vec<&str> = rings.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            ["shell", "identity", "cloud", "neighbors", "network", "host"],
            "rings in fixed outward order"
        );
        // The dummy aws.credentials.profiles (Exposed/Credentials) lands in cloud.
        let cloud = rings.iter().find(|r| r["id"] == "cloud").unwrap();
        let titles: Vec<&str> = cloud["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap())
            .collect();
        assert!(
            titles.contains(&"aws.credentials.profiles"),
            "aws creds land in cloud ring, got {titles:?}"
        );
    }

    #[test]
    fn ring_of_unit() {
        use crate::finding::{Finding, FindingClass, FindingScope};
        use crate::severity::{Confidence, Severity};
        let mk = |id: &str, class, scope| {
            Finding::new(id, class, scope, "t", Severity::Exposed, Confidence::Confirmed)
        };
        assert_eq!(
            ring_of(&mk(
                "ssh.private_keys",
                FindingClass::Credentials,
                FindingScope::Ambient
            )),
            "identity"
        );
        assert_eq!(
            ring_of(&mk(
                "env.secret_names",
                FindingClass::Credentials,
                FindingScope::Ambient
            )),
            "shell"
        );
        assert_eq!(
            ring_of(&mk(
                "aws.credentials.profiles",
                FindingClass::Credentials,
                FindingScope::Ambient
            )),
            "cloud"
        );
        assert_eq!(
            ring_of(&mk(
                "git.push_likelihood",
                FindingClass::GitWrite,
                FindingScope::CurrentRepo
            )),
            "network"
        );
        assert_eq!(
            ring_of(&mk(
                "cross_repo.sibling_repos",
                FindingClass::CrossRepo,
                FindingScope::SiblingRepos
            )),
            "neighbors"
        );
        assert_eq!(
            ring_of(&mk(
                "host.privilege_escalation",
                FindingClass::HostPersistence,
                FindingScope::Host
            )),
            "host"
        );
        assert_eq!(
            ring_of(&mk(
                "egress.mediation",
                FindingClass::Egress,
                FindingScope::Ambient
            )),
            "cloud"
        );
        assert_eq!(
            ring_of(&mk(
                "egress.connectivity",
                FindingClass::Egress,
                FindingScope::Network
            )),
            "network"
        );
    }

    #[test]
    fn stats_breadth_present() {
        let report = dummy_report();
        let data = build_data(&report, &Ok(None), None, None);
        assert!(
            data["stats"]["breadth"]["probes"].is_u64(),
            "breadth.probes is an integer"
        );
        assert!(
            data["stats"]["breadth"]["stores"].is_u64(),
            "breadth.stores is an integer"
        );
    }

    #[test]
    fn page_has_sections_and_is_swept() {
        let data = build_data(&dummy_report(), &Ok(None), None, None);
        let html = render_html(&data);
        assert!(!crate::report::redaction::contains_secret_shaped(&html));
        assert!(html.contains("illustrative — post-MVP, not from your scan"));
        assert!(html.contains("REACHABILITY, NOT EXPLOITATION"));
        assert!(html.contains("fonts.googleapis.com"));
        assert!(html.contains("unpkg.com/react"));
        assert!(html.contains("id=\"br-data\""));
    }

    /// Build a small real `HistoryAuditReport` from a value-free trace so the
    /// `D.history` injection path can be exercised end-to-end.
    fn sample_history() -> HistoryAuditReport {
        use crate::finding::{Finding, FindingClass, FindingScope};
        use crate::session::trace::{AgentEvent, SessionTrace};
        use crate::severity::{Confidence, Severity};
        let baseline = vec![
            Finding::new(
                "aws.credentials.profiles",
                FindingClass::Credentials,
                FindingScope::Ambient,
                "AWS creds",
                Severity::Exposed,
                Confidence::Confirmed,
            ),
            Finding::new(
                "egress.connectivity",
                FindingClass::Egress,
                FindingScope::Network,
                "egress",
                Severity::Exposed,
                Confidence::Confirmed,
            ),
        ];
        let trace = SessionTrace {
            session_id: "X".into(),
            agent: "claude-code".into(),
            repo: Some("app".into()),
            started_at: Some("2026-06-10T00:00:00Z".into()),
            events: vec![
                AgentEvent::FileRead {
                    path: "~/.aws/credentials".into(),
                },
                AgentEvent::NetworkAccess {
                    host: "evil.example".into(),
                    port: 443,
                },
            ],
            privileged_user: false,
            after_hours: false,
        };
        let now = crate::util::time::unix_from_iso8601("2026-06-13T00:00:00Z").unwrap();
        crate::session::history::build_history_report(&baseline, &[trace], now, Vec::new())
    }

    #[test]
    fn history_absent_emits_null_present_emits_report() {
        // Without history, D.history is null.
        let none = build_data(&dummy_report(), &Ok(None), None, None);
        assert!(none["history"].is_null(), "history is null when absent");
        // With history, D.history is the real value-free report.
        let h = sample_history();
        let data = build_data(&dummy_report(), &Ok(None), Some(&h), None);
        assert!(data["history"].is_object(), "history present when injected");
        assert!(
            !data["history"]["hazards"].as_array().unwrap().is_empty(),
            "real hazard flows into D.history"
        );
        assert_eq!(data["history"]["hazards"][0]["session"]["agent"], "claude-code");
        // The rendered page that consumes D.history stays value-free.
        let html = render_html(&data);
        assert!(!crate::report::redaction::contains_secret_shaped(&html));
    }

    #[test]
    fn planted_canary_in_page_is_swept() {
        const CANARY: &str = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
        let mut data = build_data(&dummy_report(), &Ok(None), None, None);
        // Inject a shaped canary into a Value field, then run the SAME path
        // render_html uses: to_string → </ guard → marker replace → sweep.
        data["canary"] = json!(CANARY);
        let data_str = serde_json::to_string(&data).unwrap();
        let data_str = data_str.replace("</", "<\\/");
        let html = page::PAGE.replace("/*__BR_DATA__*/", &data_str);
        let swept = crate::report::redaction::sweep(&html);
        assert!(!swept.contains(CANARY), "raw canary must be swept out");
        assert!(swept.contains("[REDACTED]"), "redaction marker must appear");
    }
}
