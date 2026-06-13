# Running blastradius

`blastradius` is a local, read-only reachability audit: it shows what a coding
agent running as you can reach on this machine (credentials, identities, repos,
egress, escalation paths), and — optionally — narrates the attack scenarios that
reachable set enables. No findings or secret values leave your machine; the only
outbound connections are the documented egress probe, the opt-in `--ai` call, and
the dashboard page's CDN/webfont asset loads (which carry no scan data).

## 1. Prerequisites

- **Rust** (stable, edition 2021) — install via <https://rustup.rs>.
- Linux or macOS. (Most probes are unix; a few are Linux-only and degrade to
  `Info` elsewhere.)

## 2. Build

```sh
cargo build --release        # binary at ./target/release/blastradius
```

During development you can skip the build step and use `cargo run -- <args>`
(everything below works the same — just replace `blastradius` with
`cargo run --`).

```sh
# add it to your PATH for convenience (optional)
install -m755 target/release/blastradius ~/.local/bin/blastradius
```

> Prefer not to build? `npx @amlalabs/blastradius scan` fetches a release binary
> on first explicit invocation (never on install).

## 3. Run a scan

```sh
blastradius                  # default command == `scan`
blastradius scan             # the reachability battery, printed to the terminal
```

Every scan runs at full reach automatically — home-wide sibling search, broad
env-name heuristics, and key NAMES listed (value-free). There are no flags to
narrow or disable any of it. Findings are grouped by class (Credentials,
Cross-repo, Git write, Egress, Process, Host persistence, System info) and sorted
by severity (`exposed` > `notable` > `info`).

### Write report files

```sh
blastradius scan --report            # writes ./blastradius-report.{md,json}
blastradius scan --output audit      # writes audit/blastradius-report.{md,json}
blastradius scan --json              # JSON only
```

### Worktree comparison (the "a worktree is not a boundary" demo)

```sh
blastradius compare          # repo root vs a temporary worktree, side by side
```

## 4. The web dashboard

```sh
blastradius dashboard                 # serve the dashboard, auto-open browser
blastradius dashboard --ai            # + AI attack-scenario narratives
```

A local web page (value-free, swept; UI assets and webfonts load from a CDN) with
a scroll-driven blast-radius map, severity tiles, and the full inventory. **It
always also runs the §24 retro-hazard scan over every agent transcript on disk**
(Claude Code, Codex, Cursor, …, across all time) and renders what "already
happened and still matters" — value-free, no network beyond the standard probes.

| Flag | Default | Meaning |
|---|---|---|
| `--port <N>` | `5321` | Port to serve on (`0` = pick a free port). |
| `--bind <ADDR>` | `0.0.0.0` | Interface to bind. `0.0.0.0` = reachable on your network; `127.0.0.1` = loopback only. |
| `--no-open` | off | Don't auto-open the browser. |
| `--ai` | off | Generate attack scenarios via OpenAI (see §5). |
| `--model <M>` | `gpt-4o-mini` | OpenAI model for `--ai` (or set `OPENAI_MODEL`). |

> ⚠ **Security:** the dashboard has **no authentication** and renders your full
> reachable-credential inventory, escalation paths, post-root blast radius, and
> (by default) which still-reachable credentials your agents already read — a
> precise targeting map. The default `--bind 0.0.0.0` exposes that to anyone on
> your network — fine for demoing from a trusted machine, risky on
> shared/conference WiFi. Use `--bind 127.0.0.1` to restrict it to this machine.
> A loud warning prints
> whenever the bind is non-loopback.

Stop the server with Ctrl-C.

## 5. AI attack scenarios (`--ai`)

`--ai` is the **only** feature that sends scan data off-machine, and it is opt-in.
It transmits ONLY the value-free finding inventory (ids, classes, severities,
titles, summaries — the same metadata the local report prints) to the OpenAI
API, re-checks the payload for secret shapes before sending, and renders
conceptual attack-path narratives with containment. **No secret value, file
content, or env value is ever transmitted.**

Provide the key one of two ways:

```sh
# (a) environment variable
export OPENAI_API_KEY=sk-...
blastradius dashboard --ai

# (b) a .env file in the current directory (only OPENAI_API_KEY is read)
echo 'OPENAI_API_KEY=sk-...' >> .env
blastradius dashboard --ai
```

The key is used only as the bearer token — it is never logged, printed, or
written into any report or the dashboard.

## 6. Flags

The tool always runs at full power — home-wide sibling search, broad env-name
heuristics, key NAMES listed, and the egress + cloud-metadata network probes — so
there are no flags to narrow, scope, or disable any of that. What remains:

| Command | Flag | Meaning |
|---|---|---|
| `scan` | `--report` / `--json` / `--markdown` / `--output <dir>` | Write reports. |
| `scan` | `--fail-on <severity>` | Exit non-zero if any finding meets `info`/`notable`/`exposed` (CI gate). |
| `compare` | `--report` / `--json` / `--markdown` / `--output <dir>` | Write reports. |
| `dashboard` | `--port` / `--bind` / `--no-open` / `--ai` / `--model` | See §4–§5. |
| `audit-history` | `--baseline <file>` / `--fail-on-score <N>` / `--quiet` / `--report` / `--json` / `--markdown` / `--output` | Retro-hazard scan; CI gate on realized score. |

## 7. Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | runtime error |
| `2` | invalid usage |
| `3` | `compare` run outside a git repo |
| `4` | `--fail-on <severity>` threshold met |

Example CI gate:

```sh
blastradius scan --fail-on exposed
```

## 8. Verify / develop

```sh
cargo test                          # unit + fixture + worktree tests
blastradius self-test-redaction     # assert no synthetic secret leaks any renderer
cargo run -- compare                # iterate locally
```

## 9. What it never does

- No telemetry; no secret values in output; no exploit behavior; no repo secret
  scanning.
- No findings or secret values leave the machine; outbound connections are the
  documented egress probe, the opt-in `--ai` call, and the dashboard page's
  CDN/webfont asset loads (no scan data).
- Never writes to repo files, shell/git config, credential stores, or `$HOME` by
  default.
- Never attempts privilege escalation or reads root-owned file contents — the
  post-escalation blast radius is modeled from permission metadata only.

For the full coverage map (every surface it inventories, and what's deliberately
out of scope) see [`docs/claude-code-security-model.md`](docs/claude-code-security-model.md) §6a.
