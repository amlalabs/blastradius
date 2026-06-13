//! §extra — dangerous *existing* git-config directives (doc §4.1 "Git config abuse").
//!
//! `write_reach` asks whether `.git/config` is WRITABLE. This probe asks the
//! complementary question: what already-configured directives run a command or
//! redirect a transport on ordinary git operations? `alias.* = "!sh"`,
//! `core.sshCommand`, `core.fsmonitor`, content `filter.*` clean/smudge,
//! `diff.external`, `core.pager`, `core.hooksPath`, and `url.*.insteadOf`
//! rewrites are all exec/redirect sinks that fire under the developer's normal
//! environment, outside any Bash sandbox.
//!
//! READ-ONLY (`git config --list`, no writes) and value-free: only directive
//! CATEGORIES, fixed `core.*` key names, user-chosen alias/filter names, and
//! counts are emitted — never a directive VALUE (which can embed a command or a
//! credential URL) and never an `insteadOf` key (which embeds a URL).

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::command::run_stdout;

pub struct GitConfigProbe;

/// Classification of a single directive.
#[derive(Default)]
struct Directives {
    /// Names safe to emit (fixed core.* keys, alias/filter names).
    exec: Vec<String>,
    /// Redirect/auxiliary directive categories (count-bearing, name-safe).
    redirect: Vec<String>,
    /// insteadOf rewrites — counted only (the key embeds a URL).
    insteadof_count: usize,
}

impl Directives {
    fn is_empty(&self) -> bool {
        self.exec.is_empty() && self.redirect.is_empty() && self.insteadof_count == 0
    }
}

/// Parse `git config --list -z` output and classify exec/redirect directives.
/// `-z` separates entries with NUL and key/value with newline, so a value that
/// itself contains newlines can't desync parsing.
fn classify(list_z: &str) -> Directives {
    let mut d = Directives::default();
    for entry in list_z.split('\0') {
        if entry.is_empty() {
            continue;
        }
        let (key, value) = match entry.split_once('\n') {
            Some((k, v)) => (k, v),
            None => (entry, ""),
        };
        let key = key.trim();
        let value = value.trim();

        if let Some(alias) = key.strip_prefix("alias.") {
            // Shell-executing alias (`!cmd`); plain git aliases are not exec.
            if value.starts_with('!') {
                push(&mut d.exec, format!("alias.{alias}"));
            }
            continue;
        }
        if key.starts_with("filter.")
            && (key.ends_with(".clean") || key.ends_with(".smudge") || key.ends_with(".process"))
        {
            push(&mut d.exec, key.to_string());
            continue;
        }
        match key {
            "core.sshcommand" => push(&mut d.exec, "core.sshCommand".to_string()),
            "core.fsmonitor" => {
                // `true`/`false` select the builtin / disable; any other value is
                // an external hook command.
                if !matches!(value, "" | "true" | "false") {
                    push(&mut d.exec, "core.fsmonitor".to_string());
                }
            }
            "diff.external" => push(&mut d.exec, "diff.external".to_string()),
            "core.pager" => push(&mut d.redirect, "core.pager".to_string()),
            "core.hookspath" => push(&mut d.redirect, "core.hooksPath".to_string()),
            "credential.helper" => push(&mut d.redirect, "credential.helper".to_string()),
            _ => {
                if key.ends_with(".insteadof") {
                    d.insteadof_count += 1;
                }
            }
        }
    }
    d
}

fn push(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

fn finding_for(
    id: &'static str,
    scope: FindingScope,
    label: &str,
    d: &Directives,
) -> Finding {
    let exec = !d.exec.is_empty();
    let redirect = !d.redirect.is_empty() || d.insteadof_count > 0;

    let (severity, title, summary) = if exec {
        (
            Severity::Exposed,
            format!("{label} git config runs commands on ordinary git operations"),
            format!(
                "exec directives ({}) fire outside any sandbox on normal git ops{}",
                d.exec.join(", "),
                if redirect { "; plus transport/redirect directives" } else { "" }
            ),
        )
    } else if redirect {
        (
            Severity::Notable,
            format!("{label} git config has transport/redirect directives"),
            format!(
                "{}{}{} present — can redirect transports or helpers on git ops",
                d.redirect.join(", "),
                if !d.redirect.is_empty() && d.insteadof_count > 0 { ", " } else { "" },
                if d.insteadof_count > 0 {
                    format!("{} insteadOf rewrite(s)", d.insteadof_count)
                } else {
                    String::new()
                }
            ),
        )
    } else {
        (
            Severity::Info,
            format!("no exec/redirect directives in {label} git config"),
            format!("no command-executing or transport-redirecting directives in {label} git config"),
        )
    };

    Finding::new(id, FindingClass::GitWrite, scope, title, severity, Confidence::Confirmed)
        .summary(summary)
        .evidence(json!({
            "exec_directives": d.exec,
            "redirect_directives": d.redirect,
            "insteadof_rewrite_count": d.insteadof_count,
            "note": "Directive categories / fixed key names only; directive values and insteadOf keys (which can embed commands or credential URLs) are never emitted.",
        }))
        .remediation(&[
            "Review git aliases, core.sshCommand/fsmonitor, content filters, and insteadOf rewrites before running git in an agent-touched repo.",
            "Treat git config as code: pin or audit these directives for untrusted/agent-modified repositories.",
        ])
}

impl Probe for GitConfigProbe {
    fn id(&self) -> &'static str {
        "git.config_exec_directives"
    }
    fn class(&self) -> FindingClass {
        FindingClass::GitWrite
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let mut findings = Vec::new();

        // Global (~/.gitconfig) — ambient: applies to every repo the user touches.
        if let Some(out) = run_stdout("git", &["config", "--global", "--list", "-z"], None) {
            let d = classify(&out);
            if !d.is_empty() {
                findings.push(finding_for(
                    self.id(),
                    FindingScope::Ambient,
                    "global",
                    &d,
                ));
            }
        }

        // Local (repo .git/config) — CurrentRepo: may differ across worktrees.
        if ctx.git.is_repo {
            if let Some(out) =
                run_stdout("git", &["config", "--local", "--list", "-z"], Some(&ctx.cwd))
            {
                let d = classify(&out);
                findings.push(finding_for(
                    "git.config_exec_directives.local",
                    FindingScope::CurrentRepo,
                    "local repo",
                    &d,
                ));
            }
        }

        if findings.is_empty() {
            findings.push(
                Finding::new(
                    self.id(),
                    FindingClass::GitWrite,
                    FindingScope::Ambient,
                    "no exec/redirect git-config directives",
                    Severity::Info,
                    Confidence::Confirmed,
                )
                .summary("no command-executing or transport-redirecting git config detected")
                .evidence(json!({ "exec_directives": [], "redirect_directives": [] })),
            );
        }

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_alias_is_exec() {
        let z = "alias.st\nstatus\0alias.danger\n!rm -rf /\0core.pager\nless\0";
        let d = classify(z);
        assert!(d.exec.contains(&"alias.danger".to_string()));
        assert!(!d.exec.contains(&"alias.st".to_string()));
        assert!(d.redirect.contains(&"core.pager".to_string()));
    }

    #[test]
    fn fsmonitor_boolean_is_not_exec_but_command_is() {
        assert!(classify("core.fsmonitor\ntrue\0").exec.is_empty());
        assert!(classify("core.fsmonitor\n/opt/watchman-hook\0")
            .exec
            .contains(&"core.fsmonitor".to_string()));
    }

    #[test]
    fn insteadof_is_counted_not_named() {
        // The key embeds a URL; we must not surface it.
        let z = "url.git@github.com:.insteadof\nhttps://github.com/\0";
        let d = classify(z);
        assert_eq!(d.insteadof_count, 1);
        assert!(d.exec.is_empty() && d.redirect.is_empty());
    }

    #[test]
    fn content_filters_are_exec() {
        let d = classify("filter.lfs.clean\ngit-lfs clean\0filter.lfs.smudge\ngit-lfs smudge\0");
        assert!(d.exec.contains(&"filter.lfs.clean".to_string()));
        assert!(d.exec.contains(&"filter.lfs.smudge".to_string()));
    }
}
