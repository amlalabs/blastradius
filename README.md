# blastradius

A local diagnostic that shows what a coding agent running as you can reach.

## Why this exists

Worktrees are not security boundaries. Coding agents inherit ambient authority —
your shell environment, SSH keys, git credentials, cloud profiles, registry
tokens, sibling repos, shell history, network egress, and filesystem visibility.
`blastradius` makes that reachable surface visible.

It proves **reachability, not intent.** It does not claim an agent is malicious,
that a token is valid, that a push would be accepted, or that every secret was
found. It claims: these files, stores, remotes, and egress routes are reachable
by code running as this user — and a worktree alone does not constrain that.

## Install

```sh
# npm wrapper (fetches the binary on first explicit invocation — never on install)
npx @amlalabs/blastradius compare
```

Release binaries and `SHA256SUMS` are also published on GitHub Releases for manual install.

## What it checks

Credentials — cloud & cluster stores (AWS profiles + **SSO/CLI token caches** ·
GCP · Azure · Kubernetes config + **in-pod service-account token** · Docker &
podman registry auth · HashiCorp Vault), package/registry tokens (npm · PyPI ·
Cargo · Terraform), database & signing creds (`.pgpass` · GPG keys), the **OS
keyring / Secret Service** (GNOME Keyring · KWallet · macOS Keychain),
**SaaS CLI tokens** (Vercel · Netlify · Fly · doctl · Sentry · …), the agent's
**own AI-assistant credentials**, SSH private keys and a reachable **ssh-agent**
(loaded keys usable without the key files), **browser cookie jars & saved
passwords** (session hijack past password+MFA), GitHub/git credential sources
(incl. XDG path), secret-named env vars, shell- and **DB/REPL-client** history
token patterns · cross-repo exposure (sibling repos, lateral `.env`/key files) ·
git remotes + push likelihood + dangerous git-config directives (`alias`/
`sshCommand`/`fsmonitor`/filters/`insteadOf`) · writable Claude Code control &
instruction surface (`settings.json`, `.mcp.json`, `CLAUDE.md`) · build/data/
secrets-manager creds (Maven · Gradle · Composer · RubyGems · pip · NuGet · dbt ·
Databricks · Snowflake · SOPS/age · Teleport · password managers · rclone · …) ·
**privilege escalation** (docker-group / NOPASSWD sudo → root) · **process memory
introspection** (`ptrace_scope` → dump ssh-agent/browser/password-manager RAM) ·
secrets in other processes' command lines · **reachable localhost datastores** ·
cron / systemd-timer & editor/login persistence · outbound egress.

The credential-store checks are **spec-driven** (~35 stores): each is a data
entry, so adding one is a few lines — see `src/probes/registry.rs`. The full
coverage map is in [docs/claude-code-security-model.md](docs/claude-code-security-model.md) §6a.

## What it never does

- no telemetry · no secret values · no exploit behavior · no repo secret scanning
- no network except the documented egress probe (one TLS connect; `--no-egress`
  to disable) **and** the opt-in `dashboard --ai` call (see below)
- never writes to repo files, shell/git config, credential stores, or `$HOME`
  by default

## Dashboard & AI blast-radius analysis

```sh
blastradius dashboard            # local web dashboard of the reachable surface
blastradius dashboard --ai       # + AI-generated attack-scenario narratives
```

`dashboard` runs a scan and serves a single self-contained page (no external
assets, works offline) visualizing the reachable surface, severities, and full
inventory. It binds `0.0.0.0:5321` by default; override with `--bind`/`--port`.

> ⚠ The dashboard has **no authentication** and renders your full reachable-credential
> inventory, escalation paths, and post-root blast radius. Binding to `0.0.0.0`
> exposes that to your whole network — only do so on a trusted network. Use
> `--bind 127.0.0.1` to restrict it to loopback.

`--ai` additionally asks the OpenAI API to describe, **for your own defensive
awareness**, how the *reachable* credentials/identities could be chained — attack
paths, impact, and containment — grounded only in what the scan found.

This is the **one** feature that sends anything off-machine, and it is opt-in.
It transmits ONLY the value-free inventory (finding ids, classes, severities,
titles, summaries — the same metadata the local report prints) and re-runs the
redaction sweep over the exact bytes before sending; **no secret value, file
content, or env value is ever transmitted**. The key is read from
`OPENAI_API_KEY` (environment or `./.env`) and used only as the bearer token.
Scenarios are conceptual blast-radius narratives with containment, not exploit
code. `--offline` disables it.

### Network egress probe

By default, blastradius resolves a well-known hostname and opens a single TLS
connection to a major always-available anycast endpoint (default `1.1.1.1:443`).
No HTTP body and no findings, credentials, paths, env vars, repo names,
hostnames, usernames, or machine identifiers are sent. It reports whether DNS
resolution and the TLS handshake succeeded, the resolved IP, and latency. Any
outbound connection necessarily exposes your source IP and a timestamp to the
destination. Override with `--egress-url HOST:PORT`; URL schemes, paths,
credentials, and port `0` are rejected. Custom targets are redacted in reports.
Disable with `--no-egress`/`--offline`.

## Usage

```sh
blastradius scan                 # run the battery once (default command)
blastradius scan --report        # also write ./blastradius-report.{md,json}
blastradius scan --output audit  # write audit/blastradius-report.{md,json}
blastradius scan --offline       # no network at all
blastradius scan --verbose       # list env/.env key NAMES (never values)
blastradius scan --env-broad     # opt-in heuristic env matching (Notable only)
blastradius compare              # repo-root vs temporary worktree, side by side
blastradius dashboard            # serve a local web dashboard of the reach
blastradius dashboard --ai       # + AI attack-scenario narratives (opt-in egress)
blastradius self-test-redaction  # assert no synthetic secret leaks any renderer
```

Exit codes: `0` success · `1` runtime error · `2` invalid usage · `3` compare
outside a git repo · `4` `--fail-on <severity>` threshold met (CI: `--fail-on exposed`).

## Demo

```
══ worktree comparison ════════════════════════════════════════

  AMBIENT BLAST RADIUS                  repo root      worktree
  ───────────────────────────────────────────────────────────
  AWS profiles                          2              2
  SSH private keys                      3              3
  secret-like env vars                  4              4
  sibling repos readable                23             23
  outbound connectivity                 open           open

  ►  working directory changed.  ambient blast radius UNCHANGED.
     A git worktree is a directory-level convenience, not a
     security boundary.
```

## Interpreting results

Reachability is not malice and not validity. A reachable credential may be
expired, scoped, or rejected server-side. Severity (`Info`/`Notable`/`Exposed`)
describes exposure; confidence (`Confirmed`/`Likely`/`Possible`/`Unknown`) is
reported separately for inferred capability such as push likelihood.

## What would contain this

- **Credential substitution** — scoped, short-lived creds per agent.
- **Filesystem isolation** — mount only the task repo + explicit deps.
- **Egress control** — default-deny outbound, then allowlist.
- **Process isolation** — prevent same-user process inspection.
- **Server-side enforcement** — branch protection, review, token scopes.

For a concrete worked example of one such containment layer — what the Claude Code
bubblewrap sandbox does and does not contain, audited against the open-source
`sandbox-runtime` — see [docs/claude-code-security-model.md](docs/claude-code-security-model.md).

## Development

```sh
cargo test                          # unit + fixture + worktree tests
cargo test --features network-tests # also exercise the real egress probe
cargo run -- compare
```

## License

MIT
