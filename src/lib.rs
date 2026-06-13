//! blastradius — local reachability audit for coding-agent environments.
//!
//! Library entry points used by `main.rs` and the integration tests. The product
//! is the reachable-surface inventory (§17); `compare` is framing on top.

pub mod analyze;
pub mod cli;
pub mod compare;
pub mod context;
pub mod dashboard;
pub mod finding;
pub mod probes;
pub mod report;
pub mod runner;
pub mod severity;
pub mod util;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, ffi::OsString};

use anyhow::Result;

use crate::cli::{Cli, Command, CompareArgs, DashboardArgs, ScanArgs};
use crate::compare::{diff, worktree};
use crate::context::{Context, ContextLabel, ScanLimits, ScanOptions};
use crate::report::{ContextReport, RunReport};
use crate::runner::{default_probes, run_all};
use crate::severity::Severity;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Exit codes (§19).
pub mod exit {
    pub const SUCCESS: i32 = 0;
    pub const RUNTIME_ERROR: i32 = 1;
    pub const COMPARE_NOT_REPO: i32 = 3;
    pub const FAIL_ON_MET: i32 = 4;
}

/// Top-level dispatch. Returns the process exit code.
pub fn run(cli: Cli) -> i32 {
    let result = match cli.command {
        None => run_scan(ScanArgs::default(), "scan", false),
        Some(Command::Scan(args)) => run_scan(args, "scan", false),
        Some(Command::Report(args)) => run_scan(args, "scan", true),
        Some(Command::Compare(args)) => run_compare(args),
        Some(Command::Dashboard(args)) => run_dashboard(args),
        Some(Command::SelfTestRedaction) => self_test_redaction(),
        Some(Command::Version) => {
            println!("blastradius {VERSION}");
            Ok(exit::SUCCESS)
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            exit::RUNTIME_ERROR
        }
    }
}

fn build_limits(max_depth: Option<usize>, max_repos: Option<usize>, home_wide: bool) -> ScanLimits {
    let mut limits = ScanLimits::default();
    if let Some(d) = max_depth {
        limits.max_depth_home_roots = d;
    }
    if let Some(r) = max_repos {
        limits.max_sibling_repos = r;
    }
    limits.home_wide = home_wide;
    limits
}

fn build_options(env_broad: bool, verbose: bool) -> ScanOptions {
    ScanOptions { env_broad, verbose }
}

fn run_scan(args: ScanArgs, mode: &str, force_report: bool) -> Result<i32> {
    let limits = build_limits(args.max_depth, args.max_repos, args.home_wide);
    let options = build_options(args.env_broad, args.verbose);

    let cwd = std::env::current_dir()?;
    let label = if Context::build(
        ContextLabel::Cwd,
        cwd.clone(),
        limits.clone(),
        options.clone(),
    )
    .repo_root
    .is_some()
    {
        ContextLabel::RepoRoot
    } else {
        ContextLabel::Cwd
    };
    let ctx = Context::build(label, cwd, limits, options.clone());
    let findings = run_all(&ctx, &default_probes());

    let report = RunReport {
        mode: mode.to_string(),
        timestamp: util::time::now_iso8601(),
        version: VERSION.to_string(),
        platform: ctx.platform,
        command: command_line(),
        contexts: vec![ContextReport {
            context: ctx,
            findings,
        }],
        comparison: None,
    };

    println!("{}", report::terminal::render(&report));

    let (want_json, want_markdown) = report_formats(
        force_report,
        args.report,
        args.json,
        args.markdown,
        args.output.is_some(),
    );
    write_reports(&report, args.output.as_deref(), want_json, want_markdown)?;

    Ok(fail_on_exit(&report, args.fail_on.as_deref()))
}

fn run_compare(args: CompareArgs) -> Result<i32> {
    let cwd = std::env::current_dir()?;

    // Locate the MAIN repo root; exit 3 cleanly if not in a repo (§19).
    let main_root = match worktree::require_main_repo_root(&cwd) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("compare must be run inside a git repository");
            return Ok(exit::COMPARE_NOT_REPO);
        }
    };

    // Ensure temp worktrees are cleaned up on Ctrl-C (§13).
    let _ = ctrlc::set_handler(|| {
        worktree::cleanup_all();
        std::process::exit(130);
    });

    let limits = build_limits(None, None, false);
    let options = ScanOptions::default();

    // Repo-root context — discovery_roots are anchored here and SHARED (§12.8).
    let root_ctx = Context::build(
        ContextLabel::RepoRoot,
        main_root.clone(),
        limits.clone(),
        options.clone(),
    );
    let root_findings = run_all(&root_ctx, &default_probes());

    // Worktree context — same env snapshot, same discovery_roots, different cwd.
    let wt = worktree::Worktree::create(&main_root, args.keep_worktree_on_error)?;
    let mut wt_ctx = root_ctx.clone();
    wt_ctx.label = ContextLabel::Worktree;
    wt_ctx.cwd = wt.path().to_path_buf();
    // The worktree's CurrentRepo scans target its own checkout (at HEAD), while
    // discovery_roots and env are intentionally inherited unchanged (§12.8, §13).
    wt_ctx.checkout_root = Some(wt.path().to_path_buf());
    let wt_findings = run_all(&wt_ctx, &default_probes());

    let comparison = diff::compare(&root_findings, &wt_findings);

    let report = RunReport {
        mode: "compare".to_string(),
        timestamp: util::time::now_iso8601(),
        version: VERSION.to_string(),
        platform: root_ctx.platform,
        command: command_line(),
        contexts: vec![
            ContextReport {
                context: root_ctx,
                findings: root_findings,
            },
            ContextReport {
                context: wt_ctx,
                findings: wt_findings,
            },
        ],
        comparison: Some(comparison),
    };

    println!("{}", report::terminal::render(&report));

    let (want_json, want_markdown) = report_formats(
        false,
        args.report,
        args.json,
        args.markdown,
        args.output.is_some(),
    );
    write_reports(&report, args.output.as_deref(), want_json, want_markdown)?;

    // Worktree removed here via Drop.
    drop(wt);
    Ok(exit::SUCCESS)
}

fn run_dashboard(args: DashboardArgs) -> Result<i32> {
    use crate::analyze::{self, Analysis};

    let limits = build_limits(None, None, args.home_wide);
    let options = ScanOptions::default();

    let cwd = std::env::current_dir()?;
    let label = if Context::build(ContextLabel::Cwd, cwd.clone(), limits.clone(), options.clone())
        .repo_root
        .is_some()
    {
        ContextLabel::RepoRoot
    } else {
        ContextLabel::Cwd
    };
    let ctx = Context::build(label, cwd, limits, options.clone());
    let findings = run_all(&ctx, &default_probes());

    // Optional AI analysis — the ONE thing that sends data off-machine. Opt-in,
    // value-free, with an explicit disclosure of exactly what is transmitted.
    let analysis: std::result::Result<Option<Analysis>, String> = if args.ai {
        match load_openai_key() {
            Some(key) => {
                let model = args
                    .model
                    .clone()
                    .or_else(|| std::env::var("OPENAI_MODEL").ok())
                    .unwrap_or_else(|| "gpt-4o-mini".to_string());
                let profile = analyze::profile_from_findings(&format!("{:?}", ctx.platform), &findings);
                eprintln!("\n  ⚠ --ai: sending the VALUE-FREE inventory to OpenAI (model: {model})");
                eprintln!("    transmitted: {} reachable surface(s) as id/class/severity/title/summary", profile.reachable.len());
                eprintln!("    NOT transmitted: any secret value, file contents, or env values");
                eprintln!("    endpoint: api.openai.com · key: OPENAI_API_KEY (env or ./.env)\n");
                match analyze::analyze(&profile, &key, &model) {
                    Ok(a) => Ok(Some(a)),
                    Err(e) => Err(format!("{e:#}")),
                }
            }
            None => Err(
                "no OPENAI_API_KEY found (set it in the environment or ./.env)".to_string(),
            ),
        }
    } else {
        Ok(None)
    };

    let report = RunReport {
        mode: "dashboard".to_string(),
        timestamp: util::time::now_iso8601(),
        version: VERSION.to_string(),
        platform: ctx.platform,
        command: command_line(),
        contexts: vec![ContextReport {
            context: ctx,
            findings,
        }],
        comparison: None,
    };

    // Print the terminal report too, so the CLI stays useful headless.
    println!("{}", report::terminal::render(&report));

    dashboard::serve(
        &report,
        dashboard::ServeOptions {
            port: args.port,
            bind: args.bind.clone(),
            open_browser: !args.no_open,
            analysis,
        },
    )?;
    Ok(exit::SUCCESS)
}

/// Load the OpenAI key from the environment, falling back to `OPENAI_API_KEY` in
/// a `./.env` file. Returns the value for use as a bearer token only — it is
/// never printed, logged, or placed in any report.
fn load_openai_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENAI_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    // Minimal, single-key .env reader (we never load other values).
    let text = std::fs::read_to_string(".env").ok()?;
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(rest) = line.strip_prefix("OPENAI_API_KEY=") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn write_reports(
    report: &RunReport,
    output: Option<&str>,
    want_json: bool,
    want_markdown: bool,
) -> Result<()> {
    if !want_json && !want_markdown {
        return Ok(());
    }
    let dir: PathBuf = output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    prepare_output_dir(&dir)?;

    if want_markdown {
        let path = dir.join("blastradius-report.md");
        write_report_file(&path, &report::markdown::render(report))?;
        eprintln!("wrote {}", path.display());
    }
    if want_json {
        let path = dir.join("blastradius-report.json");
        write_report_file(&path, &report::json::render(report))?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

fn prepare_output_dir(dir: &Path) -> Result<()> {
    if dir.as_os_str().is_empty() {
        anyhow::bail!("report output directory is empty");
    }

    match std::fs::symlink_metadata(dir) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                anyhow::bail!(
                    "report output directory must not be a symlink: {}",
                    dir.display()
                );
            }
            if !meta.is_dir() {
                anyhow::bail!("report output path is not a directory: {}", dir.display());
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(dir)?;
            let meta = std::fs::symlink_metadata(dir)?;
            if meta.file_type().is_symlink() {
                anyhow::bail!(
                    "report output directory must not be a symlink: {}",
                    dir.display()
                );
            }
            if !meta.is_dir() {
                anyhow::bail!("report output path is not a directory: {}", dir.display());
            }
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn report_formats(
    force_report: bool,
    report: bool,
    json: bool,
    markdown: bool,
    output_provided: bool,
) -> (bool, bool) {
    let both = force_report || report || (output_provided && !json && !markdown);
    (both || json, both || markdown)
}

fn write_report_file(path: &Path, contents: &str) -> Result<()> {
    if path.is_dir() {
        anyhow::bail!("report path is a directory: {}", path.display());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid report path: {}", path.display()))?;
    let tmp = parent.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(contents.as_bytes())?;
    f.sync_all()?;
    drop(f);

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(windows)]
            {
                // Windows cannot rename over an existing destination. Remove
                // only the destination directory entry, then retry; symlink
                // entries are removed themselves, not followed.
                if path.exists() {
                    std::fs::remove_file(path)?;
                    if let Err(retry) = std::fs::rename(&tmp, path) {
                        let _ = std::fs::remove_file(&tmp);
                        return Err(retry.into());
                    }
                    return Ok(());
                }
            }
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_directory_implies_both_report_formats() {
        assert_eq!(
            report_formats(false, false, false, false, true),
            (true, true)
        );
    }

    #[test]
    fn explicit_format_flags_are_respected() {
        assert_eq!(
            report_formats(false, false, true, false, false),
            (true, false)
        );
        assert_eq!(
            report_formats(false, false, false, true, true),
            (false, true)
        );
        assert_eq!(
            report_formats(false, true, false, false, false),
            (true, true)
        );
    }

    #[test]
    fn command_line_redacts_user_supplied_values() {
        let cmd = command_line_from([
            "/tmp/bin/blastradius",
            "scan",
            "--output",
            "/tmp/customer-secret",
            "--max-depth=3",
            "unexpected-secret",
        ]);
        assert_eq!(
            cmd,
            "blastradius scan --output [value] --max-depth=[value] [arg]"
        );
        assert!(!cmd.contains("customer-secret"));
        assert!(!cmd.contains("unexpected-secret"));
    }
}

fn fail_on_exit(report: &RunReport, fail_on: Option<&str>) -> i32 {
    if let Some(threshold) = fail_on {
        if let Some(sev) = Severity::parse_threshold(threshold) {
            if report.max_severity_rank() >= sev.rank() {
                return exit::FAIL_ON_MET;
            }
        }
    }
    exit::SUCCESS
}

fn command_line() -> String {
    command_line_from(std::env::args())
}

fn command_line_from<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    const VALUE_FLAGS: &[&str] = &[
        "--output",
        "--max-depth",
        "--max-repos",
        "--fail-on",
        "--bind",
        "--port",
        "--model",
    ];
    const COMMANDS: &[&str] = &[
        "scan",
        "compare",
        "report",
        "dashboard",
        "self-test-redaction",
        "version",
    ];

    let mut out = Vec::new();
    let mut expect_value = false;
    for (idx, raw) in args.into_iter().map(Into::into).enumerate() {
        if idx == 0 {
            let name = Path::new(&raw)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("blastradius");
            out.push(name.to_string());
            continue;
        }
        if expect_value {
            out.push("[value]".to_string());
            expect_value = false;
            continue;
        }
        if let Some((flag, _value)) = raw.split_once('=') {
            if raw.starts_with("--") {
                out.push(format!("{flag}=[value]"));
                continue;
            }
        }
        if VALUE_FLAGS.contains(&raw.as_str()) {
            out.push(raw);
            expect_value = true;
        } else if raw.starts_with('-') || COMMANDS.contains(&raw.as_str()) {
            out.push(raw);
        } else {
            out.push("[arg]".to_string());
        }
    }
    if expect_value {
        out.push("[missing]".to_string());
    }
    out.join(" ")
}

struct EnvRestore {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvRestore {
    fn capture(key: &'static str) -> EnvRestore {
        EnvRestore {
            key,
            original: env::var_os(key),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

/// §4.4 — canary self-test. Asserts no synthetic secret value leaks through any
/// renderer, and that the Layer-2 sweep strips known secret shapes.
pub fn self_test_redaction() -> Result<i32> {
    use crate::report::redaction::contains_secret_shaped;

    let _restore_canary = EnvRestore::capture("BLASTRADIUS_TEST_SECRET");
    let _restore_openai = EnvRestore::capture("OPENAI_API_KEY");

    // 1. Ensure a canary value is present in the process environment.
    let canary = env::var("BLASTRADIUS_TEST_SECRET")
        .unwrap_or_else(|_| "br_test_SHOULD_NOT_LEAK".to_string());
    env::set_var("BLASTRADIUS_TEST_SECRET", &canary);
    // A curated-shaped env var carrying the canary as its value — Layer 1 must
    // strip the value (only the name + length are ever captured).
    env::set_var("OPENAI_API_KEY", &canary);

    // 2. Run a real scan against the live (now-seeded) environment.
    let options = ScanOptions::default();
    let limits = ScanLimits::default();
    let cwd = std::env::current_dir()?;
    let ctx = Context::build(ContextLabel::Cwd, cwd, limits, options);
    let findings = run_all(&ctx, &default_probes());

    // 3. Add a synthetic finding whose evidence carries known secret SHAPES to
    //    prove the Layer-2 sweep removes them even if Layer 1 ever failed.
    let mut findings = findings;
    findings.push(
        finding::Finding::new(
            "selftest.synthetic",
            finding::FindingClass::Credentials,
            finding::FindingScope::Ambient,
            "synthetic secret-shaped fixture",
            Severity::Info,
            severity::Confidence::Unknown,
        )
        .summary("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345 AKIAIOSFODNN7EXAMPLE")
        .evidence(serde_json::json!({
            "pem": "-----BEGIN RSA PRIVATE KEY-----",
            "url": "https://user:hunter2@example.com/x",
        })),
    );

    let report = RunReport {
        mode: "self-test".to_string(),
        timestamp: util::time::now_iso8601(),
        version: VERSION.to_string(),
        platform: ctx.platform,
        command: command_line(),
        contexts: vec![ContextReport {
            context: ctx,
            findings,
        }],
        comparison: None,
    };

    let term = report::terminal::render(&report);
    let md = report::markdown::render(&report);
    let json = report::json::render(&report);
    // The dashboard is an additional rendered surface: build its value-free JSON
    // payload and the full HTML page, then subject it to the same canary-leak +
    // secret-shape checks as the other renderers.
    let ai_none: std::result::Result<Option<crate::analyze::Analysis>, String> = Ok(None);
    let dash = crate::dashboard::render_html(&crate::dashboard::build_data(&report, &ai_none));

    let mut failures = Vec::new();
    for (name, rendered) in [
        ("terminal", &term),
        ("markdown", &md),
        ("json", &json),
        ("dashboard", &dash),
    ] {
        if rendered.contains(&canary) {
            failures.push(format!("canary leaked in {name} renderer"));
        }
        if contains_secret_shaped(rendered) {
            failures.push(format!("secret-shaped string survived {name} sweep"));
        }
    }

    if failures.is_empty() {
        println!("→ redaction self-test passed");
        println!(
            "  synthetic secret value was not present in terminal, markdown, json, or dashboard renderers"
        );
        Ok(exit::SUCCESS)
    } else {
        for f in &failures {
            eprintln!("✗ {f}");
        }
        Err(anyhow::anyhow!("redaction self-test FAILED"))
    }
}

/// Re-exported for integration tests.
pub fn scan_context(cwd: &Path, options: ScanOptions, limits: ScanLimits) -> ContextReport {
    let ctx = Context::build(ContextLabel::Cwd, cwd.to_path_buf(), limits, options);
    let findings = run_all(&ctx, &default_probes());
    ContextReport {
        context: ctx,
        findings,
    }
}
