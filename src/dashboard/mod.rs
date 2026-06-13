//! Local web dashboard (`blastradius dashboard`).
//!
//! Runs a scan, optionally generates an AI blast-radius analysis, and serves a
//! single self-contained page on `127.0.0.1` — no external assets, no telemetry.
//! The page data is the value-free finding inventory; it is swept through the
//! Layer-2 redaction pass before it is ever written to a socket.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use crate::analyze::Analysis;
use crate::report::redaction::sweep;
use crate::report::RunReport;

mod page;

/// Options controlling how the dashboard is served.
pub struct ServeOptions {
    pub port: u16,
    /// Address to bind (e.g. "127.0.0.1" or "0.0.0.0").
    pub bind: String,
    pub open_browser: bool,
    /// AI analysis result, or an error string if `--ai` was requested but failed.
    pub analysis: Result<Option<Analysis>, String>,
}

/// Build the value-free dashboard JSON from a report (+ optional AI analysis).
pub fn build_data(report: &RunReport, analysis: &Result<Option<Analysis>, String>) -> Value {
    // The dashboard reflects the first (primary) context.
    let cr = report.contexts.first();
    let platform = format!("{:?}", report.platform);

    let mut verdict: Option<String> = None;
    let mut findings_json: Vec<Value> = Vec::new();
    let mut classes: Vec<(crate::finding::FindingClass, usize, usize)> = Vec::new();
    let (mut exposed, mut notable) = (0usize, 0usize);

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
            findings_json.push(json!({
                "id": f.id,
                "class": f.class.to_string(),
                "classLabel": f.class.section_title(),
                "scope": f.scope.to_string(),
                "title": f.title,
                "summary": f.summary,
                "severity": f.severity.label(),
                "confidence": f.confidence.label(),
                "reachable": reachable,
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
        "stats": { "exposed": exposed, "notable": notable, "classes": class_tiles },
        "findings": findings_json,
        "ai": ai,
    })
}

/// Render the full HTML page with the data embedded, swept for secret shapes.
fn render_html(data: &Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    // Guard against `</script>` breaking out of the embedded JSON block.
    let data_str = data_str.replace("</", "<\\/");
    let html = page::PAGE.replace("/*__BR_DATA__*/", &data_str);
    // Defense in depth: the data is value-free, but sweep the final bytes anyway.
    sweep(&html)
}

/// Serve the dashboard until interrupted (Ctrl-C).
pub fn serve(report: &RunReport, opts: ServeOptions) -> Result<()> {
    let data = build_data(report, &opts.analysis);
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
        println!("    (local only · value-free · Ctrl-C to stop)\n");
    } else {
        println!("\n  ▸ blastradius dashboard live at http://{}:{port} (and {url} locally)", opts.bind);
        eprintln!(
            "  ⚠ SECURITY: bound to {}:{port} — reachable by ANYONE on your network, with NO authentication.",
            opts.bind
        );
        eprintln!("    The page shows your full reachable-credential inventory, escalation paths, and");
        eprintln!("    post-root blast radius. Only do this on a trusted network; use --bind 127.0.0.1");
        eprintln!("    to restrict to loopback. Ctrl-C to stop.\n");
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
    use crate::context::{Context, ContextLabel, NetworkPolicy, ScanLimits};

    fn dummy_report() -> RunReport {
        let ctx = Context::build(
            ContextLabel::Cwd,
            std::env::temp_dir(),
            ScanLimits::default(),
            NetworkPolicy {
                egress_enabled: false,
                offline: true,
                ..NetworkPolicy::default()
            },
        );
        RunReport {
            mode: "dashboard".into(),
            offline: true,
            egress_enabled: false,
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
        let data = build_data(&report, &Ok(None));
        let html = render_html(&data);
        assert!(html.contains("AWS credentials reachable"));
        assert!(html.contains("<!doctype html>") || html.contains("<!DOCTYPE html>"));
        // No secret-shaped content.
        assert!(!crate::report::redaction::contains_secret_shaped(&html));
    }

    #[test]
    fn build_data_counts_reachable_classes() {
        let report = dummy_report();
        let data = build_data(&report, &Ok(None));
        assert_eq!(data["stats"]["exposed"], 1);
        assert_eq!(data["stats"]["classes"][0]["label"], "CREDENTIALS");
    }
}
