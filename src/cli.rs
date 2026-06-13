//! clap CLI surface (§6, §7). Bare `blastradius` ≡ `blastradius scan`.

use clap::{Args, Parser, Subcommand};

use crate::util::net::validate_host_port_target;

const EGRESS_HELP: &str = "\
Network egress probe:
  By default, blastradius checks outbound reachability by resolving a
  well-known hostname and opening a single TLS connection to a major,
  always-available anycast endpoint (default: 1.1.1.1:443). No HTTP body
  and no findings, credentials, paths, env vars, repo names, hostnames,
  usernames, or machine identifiers are sent.

  It reports whether DNS resolution and the TLS handshake succeeded, the
  resolved IP, and latency.

  Any outbound connection necessarily exposes your source IP and a timestamp
  to the destination. Override with --egress-url HOST:PORT (schemes,
  credentials, paths, and port 0 are rejected); disable with --no-egress or
  --offline.";

#[derive(Parser, Debug)]
#[command(
    name = "blastradius",
    version,
    about = "Local reachability audit for coding-agent environments",
    long_about = EGRESS_HELP,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the reachability battery once against the current context.
    Scan(ScanArgs),
    /// Compare ambient reach from the repo root vs a temporary worktree.
    Compare(CompareArgs),
    /// Convenience for `scan --report`.
    Report(ScanArgs),
    /// Serve a local web dashboard of the reachable surface (+ optional AI
    /// attack-scenario analysis with `--ai`).
    Dashboard(DashboardArgs),
    /// Run synthetic fixtures through all renderers; assert no canary leaks.
    SelfTestRedaction,
    /// Print version.
    Version,
}

#[derive(Args, Debug, Clone)]
pub struct DashboardArgs {
    /// Port to serve on. 0 picks a free port.
    #[arg(long, default_value_t = 5321)]
    pub port: u16,
    /// Address to bind. 0.0.0.0 exposes the dashboard to your whole network with
    /// NO authentication — only use it on a trusted network. Use 127.0.0.1 for
    /// loopback-only.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,
    /// Don't auto-open the browser.
    #[arg(long)]
    pub no_open: bool,
    /// Generate AI attack-scenario narratives. This sends the VALUE-FREE finding
    /// inventory (severities, credential classes, names, counts, paths — never
    /// secret values) to the OpenAI API using OPENAI_API_KEY from env or ./.env.
    #[arg(long)]
    pub ai: bool,
    /// OpenAI model for `--ai` (or set OPENAI_MODEL).
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
    /// Run the scan fully offline (implies --no-egress; disables --ai).
    #[arg(long)]
    pub offline: bool,
    /// Disable the egress reachability probe during the scan.
    #[arg(long)]
    pub no_egress: bool,
    /// Additionally search all of $HOME for sibling repos.
    #[arg(long)]
    pub home_wide: bool,
    /// Also probe cloud-metadata reachability (a second outbound connection).
    #[arg(long)]
    pub check_metadata: bool,
}

#[derive(Args, Debug, Clone, Default)]
#[command(after_help = EGRESS_HELP, after_long_help = EGRESS_HELP)]
pub struct ScanArgs {
    /// Write both Markdown and JSON reports to ./ (or --output dir).
    #[arg(long)]
    pub report: bool,
    /// Write a JSON report.
    #[arg(long)]
    pub json: bool,
    /// Write a Markdown report.
    #[arg(long)]
    pub markdown: bool,
    /// Directory to write reports into (created if needed).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,
    /// Disable the egress reachability probe.
    #[arg(long)]
    pub no_egress: bool,
    /// Run fully offline (implies --no-egress).
    #[arg(long)]
    pub offline: bool,
    /// Max traversal depth for home/sibling roots.
    #[arg(long, value_name = "N")]
    pub max_depth: Option<usize>,
    /// Max number of sibling repos to enumerate.
    #[arg(long, value_name = "N")]
    pub max_repos: Option<usize>,
    /// Additionally search all of $HOME for sibling repos.
    #[arg(long)]
    pub home_wide: bool,
    /// List env/dotenv key NAMES (never values).
    #[arg(long)]
    pub verbose: bool,
    /// Enable broad heuristic env-name matching (reported at most Notable).
    #[arg(long)]
    pub env_broad: bool,
    /// Override the egress target (host:port).
    #[arg(long, value_name = "HOST:PORT", value_parser = validate_host_port_target)]
    pub egress_url: Option<String>,
    /// Also probe cloud-metadata reachability (a second outbound connection).
    #[arg(long)]
    pub check_metadata: bool,
    /// Exit nonzero if any finding meets this severity (info|notable|exposed).
    #[arg(long, value_name = "SEVERITY", value_parser = ["info", "notable", "exposed"])]
    pub fail_on: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn fail_on_rejects_unknown_thresholds() {
        assert!(Cli::try_parse_from(["blastradius", "scan", "--fail-on", "expose"]).is_err());
        assert!(Cli::try_parse_from(["blastradius", "scan", "--fail-on", "exposed"]).is_ok());
    }

    #[test]
    fn compare_accepts_individual_report_formats() {
        assert!(Cli::try_parse_from(["blastradius", "compare", "--json"]).is_ok());
        assert!(Cli::try_parse_from(["blastradius", "compare", "--markdown"]).is_ok());
    }

    #[test]
    fn egress_url_accepts_host_port_only() {
        assert!(
            Cli::try_parse_from(["blastradius", "scan", "--egress-url", "example.com:443"]).is_ok()
        );
        assert!(
            Cli::try_parse_from(["blastradius", "compare", "--egress-url", "[::1]:443"]).is_ok()
        );

        for target in [
            "https://example.com:443",
            "user:pass@example.com:443",
            "example.com:443/path",
            "example.com",
            "example.com:0",
            "::1:443",
        ] {
            assert!(
                Cli::try_parse_from(["blastradius", "scan", "--egress-url", target]).is_err(),
                "{target}"
            );
        }
    }
}

#[derive(Args, Debug, Clone, Default)]
#[command(after_help = EGRESS_HELP, after_long_help = EGRESS_HELP)]
pub struct CompareArgs {
    /// Write both Markdown and JSON reports.
    #[arg(long)]
    pub report: bool,
    /// Write a JSON report.
    #[arg(long)]
    pub json: bool,
    /// Write a Markdown report.
    #[arg(long)]
    pub markdown: bool,
    /// Disable the egress reachability probe.
    #[arg(long)]
    pub no_egress: bool,
    /// Run fully offline (implies --no-egress).
    #[arg(long)]
    pub offline: bool,
    /// Keep the temporary worktree if an error occurs (for debugging).
    #[arg(long)]
    pub keep_worktree_on_error: bool,
    /// Directory to write reports into.
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,
    /// Override the egress target (host:port).
    #[arg(long, value_name = "HOST:PORT", value_parser = validate_host_port_target)]
    pub egress_url: Option<String>,
    /// Also probe cloud-metadata reachability (a second outbound connection).
    #[arg(long)]
    pub check_metadata: bool,
}
