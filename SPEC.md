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
4. **Tooling comparison — the buyer's close (post-MVP).** `blastradius matrix --target {bare,worktree,conductor,cmux,ax}` runs the same battery across environments and shows which actually *contain* the reach. "A worktree shares `$HOME`" is obvious; "the orchestrator you're running inherits that and doesn't interpose on creds or egress either" is not — this is the load-bearing claim for fleet operators. Ships after the single-machine diagnostic on timeline, but it's higher-priority to the buyer than the worktree compare.

---

## 3. Hard non-goals

**Not an exploit tool.** No privilege escalation, config mutation, prompt injection, executing untrusted repo code, running package scripts, reading secret *values* into output, sending findings anywhere, brute-forcing, probing third-party infra, bypassing branch protection, real or dry-run `git push`, or retrieving cloud-metadata credentials. It only asks: what is reachable?

**Not a repo secret scanner.** Not TruffleHog/Gitleaks. It does not crawl git history. It focuses on ambient machine authority. It may *count* `.env`/key-like files in nearby repos; it never extracts their values.

**Not a LAN scanner.** No port scanning, no internal enumeration, no probing arbitrary hosts. Default network behavior is exactly one transparent egress reachability check (§13.11), disabled with `--no-egress` or `--offline`.

**Not telemetry.** No analytics, no report upload, no phone-home, no hidden beacon. The npm wrapper may contact the artifact host (GitHub Releases) on explicit invocation to download the binary; the binary itself sends nothing except the explicit, documented egress probe.

---

## 4. Safety & privacy (adoption requirements, not nice-to-haves)

A tool you ask people to run where their agent runs must be more careful than an ordinary utility.

### 4.1 Read-only by default
Default `scan` writes nothing. Allowed writes: terminal output; reports only when explicitly requested (`--report`/`--output`); temporary worktree create/remove in `compare`; OS-temp bookkeeping. No default writes to repo files, shell/git config, credential stores, registry configs, or `$HOME`.

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
- **Layer 1 — probes collect metadata only.** A probe stores `EnvVarMeta { key, value_len }`, never the value. This is the primary mechanism; safety lives here.
- **Layer 2 — final defensive sweep.** Before any render, run a conservative pattern sweep over the serialized output as defense-in-depth: `ghp_`, `github_pat_`, `sk-`, `AKIA`/`ASIA`, `xoxb-`/`xoxp-`, `npm_`, `glpat-`, JWT-shaped strings, PEM private-key blocks, `https://user:pass@host` URLs.
- **Self-test (§4.4)** asserts the layers hold.

> **Dropped from v1:** typed newtypes (`RedactedText`/`SafePath`/`SecretValue`). For a 5-day MVP they're build cost without proportional safety once Layer 1 + the canary self-test exist. Add post-event if desired. *(The redaction type-system is one of two tempting ratholes — see §18.)*

### 4.4 No `--no-redact`; ship a canary self-test instead
A raw-secret runtime flag is a foot-gun for demos, CI logs, and pasted reports. Replace it with:
```
BLASTRADIUS_TEST_SECRET=br_test_SHOULD_NOT_LEAK blastradius self-test-redaction
→ redaction self-test passed
  synthetic secret value was not present in terminal, markdown, or json renderers
```
If raw inspection is ever needed, gate it behind a **compile-time** feature, not a runtime flag.

### 4.5 Output is local only
Default stdout. `--report` writes `./blastradius-report.{md,json}`; `--json` / `--markdown` write one format. `--output ./audit` writes reports into `./audit/` and implies both formats when no explicit format flag is provided. Never write outside the requested directory. An existing output directory symlink is rejected. Report files are created private on Unix (`0600`), written through a same-directory temp file, and renamed into place so an existing report-file symlink is replaced rather than followed.

### 4.6 Egress probe transparency *(revised — see §13.11 for mechanism)*
Document the probe fully in `--help`, `scan --help`, and the README:
```
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
  credentials, paths, and port 0 rejected); custom targets are redacted in
  reports. Disable with --no-egress or --offline.
```
If an HTTP request is used instead of a bare TLS connect, headers must be boring (`User-Agent: blastradius/<version>`, `Accept: text/plain`) and carry no identifiers.

---

## 5. Threat model

**Modeled actor:** a coding agent, local tool, script, compromised dependency, or subprocess running **as the current OS user**. It has the user's UID, cwd, inherited env, user-readable files and credential stores, host-permitted egress, and local git config/remotes.

**Not assumed:** root, kernel exploits, physical access, keychain-unlock bypass, network privileges beyond the host's, access to interaction-gated encrypted credentials, or server-side git permissions absent local credentials.

**"Reachable"** = a same-user local process can observe, read, enumerate, connect to, or infer it via ordinary OS APIs (readable file exists; env var present; remote configured; SSH key readable; `.env`/history/sibling-repo readable; DNS+TLS succeed; local git credentials for a host appear present).

**"Reachable" ≠** valid, sufficiently scoped, push-accepted, protection-bypassable, or malicious. Report carefully: prefer "GitHub credentials for github.com appear reachable locally; push may be possible depending on server-side authorization" over "Agent can push to GitHub."

---

## 6. Product shape

Commands: `scan` (default), `compare`, `report`, `self-test-redaction`, `version`. Bare `blastradius` ≡ `blastradius scan`.

**`scan`** — run the battery once against the current context.
Flags: `--report` `--json` `--markdown` `--output <dir>` `--no-egress` `--offline` `--egress-url <host:port>` `--check-metadata` `--max-depth <n>` `--max-repos <n>` `--home-wide` `--verbose` `--fail-on <severity>`.
Behavior: build context from process/cwd/home/platform/git → run enabled probes → render terminal summary → optional MD/JSON → never print values.

**`compare`** — run the battery from the repo root and from a temporary detached worktree off the same commit; render side-by-side. Flags: `--report` `--json` `--markdown` `--no-egress` `--offline` `--egress-url <host:port>` `--check-metadata` `--keep-worktree-on-error` `--output <dir>`. (Details §14.)

**`report`** — convenience for `scan --report`.

**`self-test-redaction`** — run synthetic fixtures through all renderers; assert no canary leaks (§4.4).

---

## 7. CLI UX

### 7.1 Trust banner (first lines)
```
blastradius — local reachability audit for coding-agent environments

Privacy:
  • no telemetry   • no findings leave this machine
  • secret values are never printed
  • one optional egress check; disable with --no-egress
```

### 7.2 Inventory, not a mystery score
Concrete counts beat `Risk: 87/100`:
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
    pub network: NetworkPolicy,
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
- *Credentials:* AWS profiles · SSH private keys · GitHub/token source (offline) · git credential store · secret-named env vars · `.env` discovery + key counts · shell-history token-pattern counts
- *Cross-repo:* sibling-repo enumeration · lateral `.env`/key-file counting
- *Git write:* remote inventory · local credential-source matching · push-likelihood inference · branch/default-branch warning
- *Egress:* DNS + single TLS connect to neutral host; `--no-egress`/`--offline`
- *Compare:* temp worktree harness · normalized side-by-side diff · cleanup

**Shipped since MVP (`✔`)** — the credential surface is now a **spec-driven store family** (`src/probes/store.rs`: one `StoreSpec` per store, run by one engine; add a store = add a data entry, see `src/probes/registry.rs`): npm/pypi/cargo tokens · Docker registry auth · kubeconfig cluster/context names · GCP/Azure config · HashiCorp Vault · Terraform Cloud · `.pgpass` hosts · GPG secret-key count. Plus bespoke probes: ssh-agent socket reachability (loaded-identity count) · dangerous git-config exec/redirect directives · writable Claude Code control & instruction surface · cloud-metadata reachability · Linux `/proc/*/environ` same-user exposure · writable shell rc · git-hooks writability.

**Also shipped** — browser session/cookie stores · cron/systemd-timer enumeration · ptrace/memory-introspection + `/proc/*/cmdline` secrets · reachable localhost datastores · local privilege escalation (groups + NOPASSWD sudo) · gpg-agent reach · network-config tampering · editor/login exec dotfiles · ~35-store credential family (build-tool/data/secrets-manager/VPN/mail/etc.) · **AI dashboard** (`dashboard [--ai]`). See `docs/claude-code-security-model.md §6a` for the full coverage map.

**Post-MVP (`○`)** — sibling-repo remote inventory · benchmark matrix · browser-key decryption-key reachability · desktop-app (Slack/Discord/Thunderbird) local state.

---

## 12. Detailed probe specs

### 12.1 AWS credentials
Stat `~/.aws/credentials`, `~/.aws/config` (respect `AWS_SHARED_CREDENTIALS_FILE`/`AWS_CONFIG_FILE`, rendered as safe paths). INI-parse **profile names** only (`[prod]` → `prod`; `[profile staging]` → `staging`). No values, no STS, no network.
Evidence `{ "files":[...], "profile_count":2, "profiles":["default","prod"] }`. Severity: `Exposed` if creds file with ≥1 profile; `Notable` if only config; `Info` if absent. Remediation: per-agent AWS creds, narrow + short-lived; don't mount broad `~/.aws` into agent envs.

### 12.2 SSH private keys
Glob `~/.ssh/id_*`, `*_rsa|_ed25519|_ecdsa`; parse `~/.ssh/config`. Exclude `*.pub`, `known_hosts`, `authorized_keys`, `config`. Treat as private key if regular, readable, non-`.pub`, and first KB contains a `-----BEGIN ... PRIVATE KEY-----` header. No contents. Config: collect Host aliases only.
Evidence `{ "key_count":3, "paths":[...], "configured_hosts":["github.com","prod-bastion"] }`. Severity `Exposed`/`Notable`/`Info`. **Precision:** don't claim keys are unencrypted/usable — say "3 private key files readable; passphrase status not checked."

### 12.3 GitHub / token source — **offline by default**
Do **not** call GitHub or run `gh auth status` by default (network). Read local: `~/.config/gh/hosts.yml` (and `~/Library/Application Support/GitHub CLI/hosts.yml`, respecting `XDG_CONFIG_HOME`). Env vars `GITHUB_TOKEN`/`GH_TOKEN`/`GITHUB_PAT` come via §12.5. Report host, user (if present), `token_present`, `token_len`; never the token. **Scopes require network**, so default:
```
GitHub token source present for github.com; scopes not checked in offline mode
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
2. **Broad heuristic is recall you don't need live** — `(?i)(TOKEN|SECRET|PASSWORD|PASSWD|API[_-]?KEY|ACCESS[_-]?KEY|PRIVATE[_-]?KEY|CREDENTIAL|AUTH|BEARER)`. Off the demo path: heuristic-only matches are reported at most `Notable`, and only with `--env-broad`. Default `scan`/`compare` use the curated set so nothing live mislabels an ordinary var.

**Suppress known-non-secret keys always:** `SSH_AUTH_SOCK`, `GPG_TTY`, `LESSKEY`, `KEYMAP`, `XDG_*`, `LC_*`, `TERM*`. (Note: `HOMEBREW_GITHUB_API_TOKEN` is a real token — it is **not** suppressed.)

Output `secret-like env vars reachable: GITHUB_TOKEN(40), OPENAI_API_KEY(51)`.
Evidence `{ "matches":[{"key":"GITHUB_TOKEN","value_len":40}], "count":1, "via":"curated" }`.
Severity: `Exposed` for curated hits; `Notable` for `--env-broad` heuristic hits; `Info` if none. Conservative by design — false positives destroy trust in exactly the room you're demoing to.

### 12.6 `.env` discovery
Search current repo (`cwd`, repo root, parents → home) and sibling repos (bounded). Patterns `.env`, `.env.*`, `*.env`, `.envrc`; exclude `.env.example|.sample|.template|.defaults`. Read ≤ `max_dotenv_bytes`; parse **keys** via `^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=`; ignore comments/blank; never store values. Separate current-repo vs siblings:
```
.env files reachable
  current repo: 1 file, 12 keys
  sibling repos: 3 files, 27 keys across 3 repos
```
Evidence `{ "current_repo":{"file_count":1,"key_count":12}, "sibling_repos":{"repo_count":3,"file_count":3,"key_count":27} }`. Default output doesn't list keys; `--verbose` lists key **names** only. Severity `Exposed` if non-example `.env` found; else `Info`.

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

### 12.11 Egress — **REVISED MECHANISM (Fix 2): no first-party SPOF**
Goal: "can a process make outbound connections from here?" Don't make the answer depend on infra you stood up the night before, and don't make yourself the host logging every attendee's IP.

**Default:** resolve a well-known hostname + open a single **TLS connection** to a major always-up anycast endpoint (default `1.1.1.1:443`), no HTTP body. Measure handshake latency; send nothing. Override with `--egress-url HOST:PORT`; URL schemes, paths, credentials, and port `0` are rejected before the scan starts. Custom targets are redacted in reports as `[custom egress target]`; reports keep `target_kind` and `resolved_ip_count` but omit the raw custom hostname/IPs. Disable with `--no-egress`/`--offline`.
*Optional:* if you want a controlled HTTP response instead of a bare connect, use a HEAD to a major CDN — **or** stand up your own endpoint **now (not event-eve)** and own the "yes, our host sees your IP" line in `--help`. Either way the probe must never be a single fresh endpoint whose downtime breaks the demo.
```
outbound connectivity reachable — 1.1.1.1, TLS ok, 19 ms
```
Evidence for the default target includes `{ "target":"1.1.1.1:443", "target_kind":"default", "dns_resolved":true, "resolved_ips":["1.1.1.1"], "resolved_ip_count":1, "tls_handshake":true, "latency_ms":19 }`. Evidence for a custom target replaces `target` with `[custom egress target]` and omits `resolved_ips` while preserving `resolved_ip_count`. Severity: `Exposed` if open; `Notable` if DNS ok but handshake fails; `Info` if disabled/blocked. Privacy note in report: "No findings were sent. The remote necessarily observed source IP and timestamp."

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

**Terminal (default):** readable in 80–100 cols, deterministic ordering. Sections: `CREDENTIALS` → `CROSS-REPO` → `GIT WRITE` → `EGRESS` → `WORKTREE COMPARISON` → `WHAT WOULD CONTAIN THIS`.

**Markdown (`--report` → `blastradius-report.md`):** timestamp, version, platform, value-redacted command shape, privacy note, findings by class, comparison table (if any), limitations, containment guidance. No values.

**JSON (`--report` → `blastradius-report.json`):** stable from day one so the future matrix mode needs no core rewrite.
```json
{
  "schema_version": "1.0",
  "tool": { "name": "blastradius", "version": "0.1.0" },
  "run": { "id": null, "timestamp": "2026-06-08T12:00:00Z", "mode": "compare", "offline": false, "egress_enabled": true },
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

**Day 3 — worktree compare + egress (framing on top of the engine).** git context; temp worktree harness; cleanup guard; **shared `discovery_roots`**; normalization; punch-protected comparison renderer; egress (neutral host); `--no-egress`/`--offline`. *DoD:* `cargo run -- compare` shows side-by-side ambient repo-root vs worktree. The compare is near-free on top of the engine — so **the engine (Days 1–2) is the asset and the real line in the sand: if the back half slips, a clean reachability inventory alone is still the product.**

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

**Exit codes:** `0` success · `1` runtime error · `2` invalid usage · `3` compare outside a git repo · `4` `--fail-on` threshold met. Findings don't cause nonzero exit by default; CI uses `--fail-on exposed`.

---

## 20. README structure

Lead with the worktree claim.
```
# blastradius
A local diagnostic that shows what a coding agent running as you can reach.

## Why this exists
Worktrees are not security boundaries. Coding agents inherit ambient authority.

## Install        npx @amlalabs/blastradius compare   ·   curl -fsSL … | sh
## What it checks  credentials, env vars, sibling repos, git auth surface, egress
## What it never does
  no telemetry · no secret values · no exploit behavior · no repo secret scanning
  no network except the documented egress probe (one TLS connect; --no-egress to disable)
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
3. **GitHub scope detection offline by default** — `gh auth status`/API calls contradict the one-egress-probe promise.
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

Does **not** need: `--compare-ax`; benchmark matrix; cloud API validation; GitHub scope verification; Docker/Kube/GCP/Azure probes; Windows; GUI; hosted dashboard.

A clean, trustworthy local binary whose product is the reachable-surface inventory. The worktree reveal is the hook that gets the broad room to look at it; the orchestrator matrix (post-MVP) is what closes the fleet operators who are the buyer.
