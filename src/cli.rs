//! clap CLI surface (§6, §7). Bare `blastradius` ≡ `blastradius scan`.
//!
//! Minimal by design: the tool always runs at full power — every scan is
//! home-wide, the network egress + cloud-metadata probes always run, and the
//! dashboard always discovers ALL agent transcripts across ALL time. There are
//! no flags to narrow, scope, or disable any of that; only genuinely operational
//! knobs remain (report formats, dashboard port/bind, opt-in `--ai`).

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "blastradius",
    version,
    about = "Local reachability audit for coding-agent environments",
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
    /// Serve a local web dashboard of the reachable surface (+ optional AI
    /// attack-scenario analysis with `--ai`). Always discovers and renders the
    /// real retro-hazard history from your local agent transcripts.
    Dashboard(DashboardArgs),
    /// Read-only, value-free discovery preview of agent session transcripts
    /// (§24.5): every discovered session, counts per event kind. Never scores,
    /// joins, or touches the network.
    Sessions,
    /// Retro-hazard scan: join historical agent sessions against the current
    /// reachable surface and rank what "already happened and still matters"
    /// (§24.5).
    AuditHistory(AuditHistoryArgs),
    /// Run synthetic fixtures through all renderers; assert no canary leaks.
    SelfTestRedaction,
}

#[derive(Args, Debug, Clone, Default)]
pub struct AuditHistoryArgs {
    /// Consume a prior `scan`/`compare` JSON as the baseline (the denominator).
    /// With none, runs the scan battery once.
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<String>,
    /// Exit with code 4 when any hazard's realized_score >= N (0..=100).
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(0..=100))]
    pub fail_on_score: Option<u8>,
    /// One value-free line per hazard, for cron/CI.
    #[arg(long)]
    pub quiet: bool,
    /// Write both Markdown and JSON reports.
    #[arg(long)]
    pub report: bool,
    /// Write a JSON report.
    #[arg(long)]
    pub json: bool,
    /// Write a Markdown report.
    #[arg(long)]
    pub markdown: bool,
    /// Directory to write reports into.
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,
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
}

#[derive(Args, Debug, Clone, Default)]
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
    /// Exit nonzero if any finding meets this severity (info|notable|exposed).
    #[arg(long, value_name = "SEVERITY", value_parser = ["info", "notable", "exposed"])]
    pub fail_on: Option<String>,
}

#[derive(Args, Debug, Clone, Default)]
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
    /// Directory to write reports into.
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,
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
    fn removed_flags_are_rejected() {
        // No backwards-compat: the scoping/disable flags are gone for good.
        for f in [
            "--home-wide",
            "--max-depth",
            "--since",
            "--agent",
            "--no-history",
        ] {
            assert!(
                Cli::try_parse_from(["blastradius", "dashboard", f]).is_err(),
                "{f} should be rejected"
            );
        }
    }
}
