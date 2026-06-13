//! clap CLI surface (§6, §7). Bare `blastradius` ≡ `blastradius scan`.

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
    /// Additionally search all of $HOME for sibling repos.
    #[arg(long)]
    pub home_wide: bool,
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
    /// Keep the temporary worktree if an error occurs (for debugging).
    #[arg(long)]
    pub keep_worktree_on_error: bool,
    /// Directory to write reports into.
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,
}
