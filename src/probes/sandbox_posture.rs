//! §extra — Claude Code tool-surface / sandbox config audit (read-only).
//!
//! Parses the Claude Code settings JSON files (user, managed, project, MCP) and
//! reports the declared sandbox posture: whether sandboxing is on, which escape
//! hatches / weakenings are present, and the un-sandboxed tool surface (MCP
//! servers, hooks). Emits booleans / ints / enum-names / identifier-names ONLY —
//! never array element values (excludedCommands, hook commands, allow/deny rules,
//! domain strings, project filesystem paths).
//!
//! READ-ONLY (no file is created/modified) and value-free. Confidence is at most
//! Likely: declared config != runtime enforcement (CLI flags / env / managed
//! precedence can override what is on disk).

use serde_json::{json, Value};

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;
use crate::util::read::{read_to_string_capped, CappedReadError};

pub struct SandboxPostureProbe;

/// Cap for the (potentially large, OAuth/session-bearing) ~/.claude.json.
const MAX_CLAUDE_JSON: u64 = 8 * 1024 * 1024;
const MAX_SETTINGS_JSON: u64 = 4 * 1024 * 1024;

/// A parsed settings scope.
struct Scope {
    value: Value,
    is_managed: bool,
}

/// Read + parse a JSON file. Returns:
///   Ok(Some(v)) parsed, Ok(None) absent, Err(reason) degraded (generic reason).
fn read_json(path: &std::path::Path, cap: Option<u64>) -> Result<Option<Value>, &'static str> {
    let text = match read_to_string_capped(path, cap.unwrap_or(MAX_SETTINGS_JSON)) {
        Ok(t) => t,
        Err(CappedReadError::NotFound | CappedReadError::NotFile) => return Ok(None),
        Err(CappedReadError::TooLarge) => return Err("file exceeds size cap; not parsed"),
        Err(CappedReadError::Unreadable) => return Err("unreadable"),
    };
    // Never store the raw serde error string (it can echo input); generic only.
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Err("json parse error"),
    }
}

/// Load a candidate settings file into a `Scope`, recording it under
/// `scopes_found` (parsed) or `scopes_degraded` (parse/size error, generic
/// reason + shortened path only — never a content snippet).
#[allow(clippy::too_many_arguments)]
fn load_scope(
    path: std::path::PathBuf,
    label: &'static str,
    is_managed: bool,
    cap: Option<u64>,
    home: Option<&std::path::Path>,
    scopes_found: &mut Vec<&'static str>,
    scopes_degraded: &mut Vec<Value>,
) -> Option<Scope> {
    match read_json(&path, cap) {
        Ok(Some(value)) => {
            scopes_found.push(label);
            Some(Scope { value, is_managed })
        }
        Ok(None) => None,
        Err(reason) => {
            scopes_degraded.push(json!({
                "scope": label,
                "path": shorten(&path, home),
                "reason": reason,
            }));
            None
        }
    }
}

/// Traverse a fixed key path.
fn jget<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    Some(cur)
}

/// Boolean with managed-scope precedence (managed wins over non-managed).
fn bool_pref(scopes: &[Scope], path: &[&str]) -> Option<bool> {
    let mut managed = None;
    let mut other = None;
    for s in scopes {
        if let Some(b) = jget(&s.value, path).and_then(|x| x.as_bool()) {
            if s.is_managed {
                managed = Some(b);
            } else {
                other = Some(b);
            }
        }
    }
    managed.or(other)
}

/// String with managed-scope precedence.
fn str_pref(scopes: &[Scope], path: &[&str]) -> Option<String> {
    let mut managed = None;
    let mut other = None;
    for s in scopes {
        if let Some(t) = jget(&s.value, path).and_then(|x| x.as_str()) {
            if s.is_managed {
                managed = Some(t.to_string());
            } else {
                other = Some(t.to_string());
            }
        }
    }
    managed.or(other)
}

/// Summed array length across scopes (union-by-count).
fn arr_count(scopes: &[Scope], path: &[&str]) -> usize {
    scopes
        .iter()
        .filter_map(|s| {
            jget(&s.value, path)
                .and_then(|x| x.as_array())
                .map(|a| a.len())
        })
        .sum()
}

/// Object key count summed across scopes.
fn obj_count(scopes: &[Scope], path: &[&str]) -> usize {
    scopes
        .iter()
        .filter_map(|s| {
            jget(&s.value, path)
                .and_then(|x| x.as_object())
                .map(|o| o.len())
        })
        .sum()
}

/// Does any denyRead array entry look like it covers home / credential dirs?
/// String compare on the PATH PATTERN (policy, not a secret).
fn deny_read_covers_creds(scopes: &[Scope]) -> bool {
    for s in scopes {
        if let Some(arr) =
            jget(&s.value, &["sandbox", "filesystem", "denyRead"]).and_then(|x| x.as_array())
        {
            for e in arr {
                if let Some(t) = e.as_str() {
                    let t = t.trim();
                    if t == "~" || t.starts_with("~/") || t.contains(".aws") || t.contains(".ssh") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Collect object KEY names across scopes (deduped), e.g. hook event types.
fn obj_keys(scopes: &[Scope], path: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in scopes {
        if let Some(obj) = jget(&s.value, path).and_then(|x| x.as_object()) {
            for k in obj.keys() {
                if !out.contains(k) {
                    out.push(k.clone());
                }
            }
        }
    }
    out.sort();
    out
}

/// Count total nested hook entries (sum of per-event array lengths).
fn hook_entry_count(scopes: &[Scope]) -> usize {
    let mut total = 0usize;
    for s in scopes {
        if let Some(obj) = jget(&s.value, &["hooks"]).and_then(|x| x.as_object()) {
            for v in obj.values() {
                if let Some(arr) = v.as_array() {
                    total += arr.len();
                }
            }
        }
    }
    total
}

impl Probe for SandboxPostureProbe {
    fn id(&self) -> &'static str {
        "claude_code.sandbox_posture"
    }
    fn class(&self) -> FindingClass {
        FindingClass::SystemInfo
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = ctx.home.as_deref();
        let mut scopes_degraded: Vec<serde_json::Value> = Vec::new();
        let mut scopes_found: Vec<&'static str> = Vec::new();

        // --- AMBIENT scopes: user + managed + global ~/.claude.json. ---
        let mut ambient_scopes: Vec<Scope> = Vec::new();
        if let Some(h) = &ctx.home {
            if let Some(s) = load_scope(
                h.join(".claude/settings.json"),
                "user",
                false,
                None,
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            ) {
                ambient_scopes.push(s);
            }
            if let Some(s) = load_scope(
                h.join(".claude/settings.local.json"),
                "user_local",
                false,
                None,
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            ) {
                ambient_scopes.push(s);
            }
        }
        if let Some(s) = load_scope(
            std::path::PathBuf::from("/etc/claude-code/managed-settings.json"),
            "managed_linux",
            true,
            None,
            home,
            &mut scopes_found,
            &mut scopes_degraded,
        ) {
            ambient_scopes.push(s);
        }
        if let Some(s) = load_scope(
            std::path::PathBuf::from(
                "/Library/Application Support/ClaudeCode/managed-settings.json",
            ),
            "managed_macos",
            true,
            None,
            home,
            &mut scopes_found,
            &mut scopes_degraded,
        ) {
            ambient_scopes.push(s);
        }
        // Global ~/.claude.json — only its mcpServers keys are read (large/noisy).
        let claude_json_global: Option<Value> = ctx.home.as_ref().and_then(|h| {
            load_scope(
                h.join(".claude.json"),
                "claude_json_global",
                false,
                Some(MAX_CLAUDE_JSON),
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            )
            .map(|s| s.value)
        });

        let managed_present = ambient_scopes.iter().any(|s| s.is_managed);

        // --- Extract the AMBIENT posture (user + managed). ---
        let sandbox_enabled = bool_pref(&ambient_scopes, &["sandbox", "enabled"]);
        let fail_if_unavailable = bool_pref(&ambient_scopes, &["sandbox", "failIfUnavailable"]);
        let allow_unsandboxed =
            bool_pref(&ambient_scopes, &["sandbox", "allowUnsandboxedCommands"]);
        let strict_mode = allow_unsandboxed == Some(false);
        let weaker_nested = bool_pref(&ambient_scopes, &["sandbox", "enableWeakerNestedSandbox"]);
        let excluded_commands_count = arr_count(&ambient_scopes, &["sandbox", "excludedCommands"]);

        let allow_write_count =
            arr_count(&ambient_scopes, &["sandbox", "filesystem", "allowWrite"]);
        let deny_write_count = arr_count(&ambient_scopes, &["sandbox", "filesystem", "denyWrite"]);
        let allow_read_count = arr_count(&ambient_scopes, &["sandbox", "filesystem", "allowRead"]);
        let deny_read_count = arr_count(&ambient_scopes, &["sandbox", "filesystem", "denyRead"]);
        let deny_read_covers = deny_read_covers_creds(&ambient_scopes);

        let allowed_domains_count =
            arr_count(&ambient_scopes, &["sandbox", "network", "allowedDomains"]);
        let denied_domains_count =
            arr_count(&ambient_scopes, &["sandbox", "network", "deniedDomains"]);
        let allow_unix_sockets_count =
            arr_count(&ambient_scopes, &["sandbox", "network", "allowUnixSockets"]);
        let weaker_network = bool_pref(
            &ambient_scopes,
            &["sandbox", "network", "enableWeakerNetworkIsolation"],
        );
        let custom_proxy = jget_any(&ambient_scopes, &["sandbox", "network", "httpProxyPort"]);

        let default_mode = str_pref(&ambient_scopes, &["permissions", "defaultMode"]);
        let perm_allow = arr_count(&ambient_scopes, &["permissions", "allow"]);
        let perm_deny = arr_count(&ambient_scopes, &["permissions", "deny"]);
        let perm_ask = arr_count(&ambient_scopes, &["permissions", "ask"]);

        let skip_dangerous =
            bool_pref(&ambient_scopes, &["skipDangerousModePermissionPrompt"]).unwrap_or(false);
        let default_mode_bypass = default_mode.as_deref() == Some("bypassPermissions");

        let enabled_plugin_count = obj_count(&ambient_scopes, &["enabledPlugins"]);
        let hook_event_types = obj_keys(&ambient_scopes, &["hooks"]);
        let hook_entry_total = hook_entry_count(&ambient_scopes);

        // MCP servers (ambient): global ~/.claude.json mcpServers keys only.
        let mut mcp_server_names: Vec<String> = Vec::new();
        if let Some(g) = &claude_json_global {
            if let Some(obj) = jget(g, &["mcpServers"]).and_then(|x| x.as_object()) {
                for k in obj.keys() {
                    if !mcp_server_names.contains(k) {
                        mcp_server_names.push(k.clone());
                    }
                }
            }
        }
        // Also any user-settings mcpServers.
        for s in &ambient_scopes {
            if let Some(obj) = jget(&s.value, &["mcpServers"]).and_then(|x| x.as_object()) {
                for k in obj.keys() {
                    if !mcp_server_names.contains(k) {
                        mcp_server_names.push(k.clone());
                    }
                }
            }
        }
        mcp_server_names.sort();
        let enable_all_project_mcp = bool_pref(&ambient_scopes, &["enableAllProjectMcpServers"]);

        // --- Severity: content-derived (correction #1). Cap at Notable
        // (correction: this probe reads config, does not prove reachability). ---
        let no_config = scopes_found.is_empty();
        let sandbox_on = sandbox_enabled == Some(true);
        let escape_hatch_active = skip_dangerous || default_mode_bypass;

        let weakened = excluded_commands_count > 0
            || allow_unix_sockets_count > 0
            || weaker_nested == Some(true)
            || weaker_network == Some(true)
            || allow_unsandboxed == Some(true)
            || !deny_read_covers
            || !hook_event_types.is_empty()
            || !mcp_server_names.is_empty()
            // escape hatches only meaningful when sandbox is ON (correction #2).
            || escape_hatch_active;

        let (severity, title, summary) = if no_config {
            (
                Severity::Info,
                "no Claude Code config found",
                "no Claude Code settings present in user/managed scopes".to_string(),
            )
        } else if sandbox_on {
            if weakened {
                (
                    Severity::Notable,
                    "sandbox enabled but weakened",
                    "sandbox.enabled=true but escape hatches / weakening / un-sandboxed surface present".to_string(),
                )
            } else {
                (
                    Severity::Info,
                    "sandbox enabled and contained",
                    "sandbox.enabled=true, no escape hatches, creds denied, no MCP/hooks"
                        .to_string(),
                )
            }
        } else {
            // Sandbox off / absent is the dominant fact (escape hatches are then
            // contextual, not double-counted).
            (
                Severity::Notable,
                "no enforceable Bash sandbox declared",
                "sandbox disabled/absent — Bash runs unsandboxed unless an outer boundary (devcontainer/VM/sandbox-runtime) wraps the process".to_string(),
            )
        };

        let ambient = Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            // Declared config != runtime enforcement (spec). Undocumented keys
            // (skipDangerousModePermissionPrompt) further reduce certainty.
            Confidence::Likely,
        )
        .summary(summary)
        .evidence(json!({
            "sandbox": {
                "enabled": sandbox_enabled,
                "fail_if_unavailable": fail_if_unavailable,
                "allow_unsandboxed_commands": allow_unsandboxed,
                "strict_mode": strict_mode,
                "weaker_nested_sandbox": weaker_nested,
                "excluded_commands_count": excluded_commands_count,
                "fs": {
                    "allow_write_count": allow_write_count,
                    "deny_write_count": deny_write_count,
                    "allow_read_count": allow_read_count,
                    "deny_read_count": deny_read_count,
                    "deny_read_covers_home_or_creds": deny_read_covers,
                },
                "network": {
                    "allowed_domains_count": allowed_domains_count,
                    "denied_domains_count": denied_domains_count,
                    "allow_unix_sockets_count": allow_unix_sockets_count,
                    "weaker_network_isolation": weaker_network,
                    "custom_proxy": custom_proxy,
                },
            },
            "permissions": {
                "default_mode": default_mode,
                "allow_count": perm_allow,
                "deny_count": perm_deny,
                "ask_count": perm_ask,
            },
            "escape_hatches": {
                "skip_dangerous_mode_prompt": skip_dangerous,
                "default_mode_bypass": default_mode_bypass,
                "note": "skipDangerousModePermissionPrompt is undocumented (suppresses dangerous-mode confirmation); escape hatches only matter when sandbox.enabled=true.",
            },
            "unsandboxed_surface": {
                "hook_event_types": hook_event_types,
                "hook_entry_count": hook_entry_total,
                "mcp_server_names": mcp_server_names,
                "mcp_server_count": mcp_server_names_len(&mcp_server_names),
                "enable_all_project_mcp": enable_all_project_mcp,
                "enabled_plugin_count": enabled_plugin_count,
                "note": "Hook/MCP counts are a floor (declared min): plugins/marketplace and interactive approval can add more.",
            },
            "scopes_found": scopes_found,
            "scopes_degraded": scopes_degraded,
            "managed_present": managed_present,
            "note": "Declared config != runtime enforcement: CLI flags (--dangerously-skip-permissions/--sandbox), env, and managed precedence can override files on disk.",
        }))
        .remediation(&[
            "Enable sandbox.enabled with allowUnsandboxedCommands:false and denyRead over ~/.aws, ~/.ssh, and credential dirs.",
            "Remove escape hatches (skipDangerousModePermissionPrompt, defaultMode:bypassPermissions) and minimise excludedCommands / allowUnixSockets.",
            "Treat MCP servers and hooks as un-sandboxed code surface; review each.",
        ]);

        let mut findings = vec![ambient];

        // --- CurrentRepo finding: project-scoped counts (allowed to differ
        // across worktrees, so kept OUT of the Ambient diff — correction #3). ---
        let mut project_scopes: Vec<Scope> = Vec::new();
        if let Some(root) = &ctx.checkout_root {
            if let Some(s) = load_scope(
                root.join(".claude/settings.json"),
                "project",
                false,
                None,
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            ) {
                project_scopes.push(s);
            }
            if let Some(s) = load_scope(
                root.join(".claude/settings.local.json"),
                "project_local",
                false,
                None,
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            ) {
                project_scopes.push(s);
            }
            if let Some(s) = load_scope(
                root.join(".mcp.json"),
                "mcp_json",
                false,
                None,
                home,
                &mut scopes_found,
                &mut scopes_degraded,
            ) {
                project_scopes.push(s);
            }
        }
        // Project-specific MCP from ~/.claude.json projects[<checkout_root>].
        let mut project_mcp_names: Vec<String> = Vec::new();
        if let (Some(g), Some(root)) = (&claude_json_global, &ctx.checkout_root) {
            let key = root.to_string_lossy().to_string();
            if let Some(obj) =
                jget(g, &["projects", &key, "mcpServers"]).and_then(|x| x.as_object())
            {
                for k in obj.keys() {
                    if !project_mcp_names.contains(k) {
                        project_mcp_names.push(k.clone());
                    }
                }
            }
        }
        // .mcp.json mcpServers + project settings mcpServers.
        for s in &project_scopes {
            for p in [&["mcpServers"][..]] {
                if let Some(obj) = jget(&s.value, p).and_then(|x| x.as_object()) {
                    for k in obj.keys() {
                        if !project_mcp_names.contains(k) {
                            project_mcp_names.push(k.clone());
                        }
                    }
                }
            }
        }
        project_mcp_names.sort();

        if !project_scopes.is_empty() || !project_mcp_names.is_empty() {
            let proj_hooks = obj_keys(&project_scopes, &["hooks"]);
            let proj_hook_entries = hook_entry_count(&project_scopes);
            let proj_sandbox = bool_pref(&project_scopes, &["sandbox", "enabled"]);
            let proj_default_mode = str_pref(&project_scopes, &["permissions", "defaultMode"]);

            let proj_surface = !proj_hooks.is_empty() || !project_mcp_names.is_empty();
            let proj_sev = if proj_surface {
                Severity::Notable
            } else {
                Severity::Info
            };

            findings.push(
                Finding::new(
                    "claude_code.project_tool_surface",
                    self.class(),
                    FindingScope::CurrentRepo,
                    "project-scoped Claude Code tool surface",
                    proj_sev,
                    Confidence::Likely,
                )
                .summary(format!(
                    "{} project MCP server(s), {} hook event type(s) declared for this repo",
                    project_mcp_names.len(),
                    proj_hooks.len()
                ))
                .evidence(json!({
                    "project_sandbox_enabled": proj_sandbox,
                    "project_default_mode": proj_default_mode,
                    "mcp_server_names": project_mcp_names,
                    "mcp_server_count": project_mcp_names.len(),
                    "hook_event_types": proj_hooks,
                    "hook_entry_count": proj_hook_entries,
                    "note": "Project scope is allowed to differ across worktrees; kept out of the ambient comparison.",
                }))
                .remediation(&[
                    "Review per-project .claude/settings.json, .mcp.json, and hooks before running an agent in the repo.",
                ]),
            );
        }

        Ok(findings)
    }
}

fn mcp_server_names_len(v: &[String]) -> usize {
    v.len()
}

/// Whether a key exists (any non-null value) across scopes — used for presence
/// flags like a custom proxy port.
fn jget_any(scopes: &[Scope], path: &[&str]) -> bool {
    scopes
        .iter()
        .any(|s| jget(&s.value, path).map(|v| !v.is_null()).unwrap_or(false))
}
