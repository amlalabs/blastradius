# Claude Code security model — architecture & audit

> **Status:** audit notes, 2026-06-08. Scope: the Linux (bubblewrap) sandbox.
> **Sources:** the open-source [`anthropic-experimental/sandbox-runtime`](https://github.com/anthropic-experimental/sandbox-runtime)
> (file:line references below are against `main` at audit time), the official docs at
> <https://code.claude.com/docs/en/sandboxing> and `.../security`, local empirical
> measurement, and static inspection of the locally installed Linux x64 Claude Code client
> artifact described in §0.

This document explains what the Claude Code sandbox does and does not contain, why, and how
each `blastradius` probe maps onto it. It exists so that a reader can interpret a `blastradius`
report against the *real* enforcement boundary rather than an assumed one.

---

## 0. Client artifact inspected

Local static inspection used:

- npm package: `@anthropic-ai/claude-code@2.1.169`
- platform package: `@anthropic-ai/claude-code-linux-x64@2.1.169`
- binary: `/home/souvik/.npm-global/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe`
- binary SHA-256: `6c02082fe7fc4327b6cc2536a17cf9ea89603af879e654013764a223e3c6b1fb`
- extracted `.bun` section SHA-256:
  `0c857d3252bf7f0e543d2188a925a9ba63f17a80819dc6d422ae871a67d239fd`

The ELF contains a Bun `.bun` payload with `file:///$bunfs/root/src/entrypoints/cli.js` and
bundled sandbox-runtime code. Direct execution of this installed artifact printed Bun 1.3.14
help/version on this host, so the client findings below are static-analysis findings, not a
successful live session trace.

Minified symbol names observed in the bundle are included where useful:

- `PZ$` = `convertToSandboxRuntimeConfig`
- `xq` = `SandboxManager`
- `DD` = bundled platform runtime facade
- `lj7` = Linux `bwrap` wrapper construction
- `cj7` = Linux `socat` bridge startup
- `sD7` / `tD7` = embedded seccomp helper fd setup

## 1. The one-paragraph model

On Linux, the Claude Code sandbox is **off by default** (`sandbox.enabled: false`; enable with
`/sandbox` or settings). When on, it wraps **only Bash-tool subprocesses** using `bubblewrap`
(`bwrap`) + `socat` + a host-side filtering proxy, via `@anthropic-ai/sandbox-runtime`. It
enforces three **independent** controls — writes (allow-only), network (deny-all behind a
hostname-allowlist proxy), and process/IPC isolation (PID namespace + a narrow seccomp filter).
It deliberately does **not**, by default, restrict **reads** or **scrub the environment**. Every
other Claude Code tool — Read, Edit, Write, WebFetch, MCP servers, hooks — runs **outside** this
sandbox under permission *rules*, which are policy, not kernel enforcement.

The load-bearing consequence: **the sandbox is a capability boundary on *write* and *egress*,
not a visibility boundary on *reads*.** Your credentials remain readable; what changes is the
ability to exfiltrate or tamper.

---

## 2. Enforcement architecture

### 2.1 The actual `bwrap` invocation

Reconstructed from `src/sandbox/linux-sandbox-utils.ts` (`wrapCommandWithSandboxLinux`) for the
common case of network + write + read restrictions:

```
bwrap --new-session --die-with-parent \
      --unshare-net \                                       # only when network is restricted
      --bind <http.sock> <http.sock> --bind <socks.sock> <socks.sock> \
      --setenv HTTP_PROXY http://localhost:3128 ... \       # + per-tool CA-trust vars
      --ro-bind / / \                                       # write-restricted: host root is read-only
      --bind <cwd> <cwd> \                                  # allowWrite paths (e.g. the workspace)
      --ro-bind /dev/null <.git/hooks | .bashrc | .mcp.json | ...> \   # mandatory write-denies
      [--tmpfs <denyRead dirs>] [--ro-bind /dev/null <denyRead files>] \
      --dev /dev \
      --unshare-pid --proc /proc \
      -- <shell> -c "socat …:3128/:1080 ; trap ; apply-seccomp <shell> -c <usercmd>"
```

Two stages run inside (`linux-sandbox-utils.ts:1024-1071`):

- **Stage 1 — outer bwrap:** network namespace, filesystem binds, PID namespace, fresh `/proc`.
  `socat` listeners start here (they still need `socket(AF_UNIX)` to reach the bridge sockets).
- **Stage 2 — `apply-seccomp`:** creates a *nested* user+PID+mount namespace, becomes PID 1,
  sets `PR_SET_NO_NEW_PRIVS`, installs the seccomp BPF filter, then `exec`s the user command —
  so the user command runs with unix-socket creation already blocked and cannot see/ptrace
  `bwrap`/`bash`/`socat`.

### 2.2 Filesystem control (`generateFilesystemArgs`)

- **Writes are allow-only.** With write restrictions, root mounts `--ro-bind / /` and only
  explicit `allowWrite` paths get `--bind` (`linux-sandbox-utils.ts:717-768`). **With no write
  config at all, the root is `--bind / /` — the entire host is read-write** (`:896`).
- **Reads are deny-then-allow.** Default is *all reads allowed*; `denyRead` masks paths
  (`--tmpfs` over directories, `--ro-bind /dev/null` over files) and `allowRead` re-binds
  exceptions (`:900-1008`).
- **Mandatory write-denies** always apply within the writable tree: `.git/hooks`, `.git/config`
  (unless `allowGitConfig`), and the `DANGEROUS_FILES`/`DANGEROUS_DIRECTORIES` set —
  `.gitconfig`, `.bashrc`, `.zshrc`, `.profile`, `.mcp.json`, `.vscode`, `.idea`,
  `.claude/commands`, `.claude/agents` (`sandbox-utils.ts:11-40`). Discovery is a ripgrep scan
  bounded to `--max-depth 3` from cwd (`linux-sandbox-utils.ts:65, 230-243`).
- **Defenses worth noting:** symlink-replacement attempts on deny paths are masked with
  `/dev/null` (`:802-809`); non-existent deny paths are blocked by mounting over the first
  missing component (`:818-876`), which creates transient "ghost" mount-point files on the host
  that are tracked and cleaned up (`:286-378`).

### 2.3 Network control (`initializeLinuxNetworkBridge`, proxy)

- `--unshare-net` removes all interfaces; the **only** egress is `socat` TCP listeners on
  `localhost:3128` (HTTP) and `:1080` (SOCKS) inside the sandbox, bridged over Unix sockets to a
  host proxy (`linux-sandbox-utils.ts:470-630`). A tool that ignores `HTTP_PROXY` therefore
  *cannot* egress — there is no route except the proxy.
- Filtering happens at the **host proxy**, not the kernel boundary; the source explicitly notes
  Linux gives "all-or-nothing" isolation and that "network restrictions … depend on the proxy's
  filtering capabilities" (`:489-492`).
- HTTPS is filtered on **hostname only unless `tlsTerminate` is configured**. TLS termination
  works by injecting a CA into per-tool trust-store env vars (`CA_TRUST_VARS`,
  `sandbox-utils.ts:303-313`), so a cert-pinning or env-ignoring client evades inspection.
- `NO_PROXY` covers `localhost`, `*.local`, link-local `169.254.0.0/16`, and RFC-1918
  (`sandbox-utils.ts:341-352`).

### 2.4 Process / IPC control (`seccomp-unix-block.c`)

- `--unshare-pid` + fresh `--proc /proc` hide other same-user host processes.
- The seccomp filter is **default-`ALLOW`** and denies only `socket(AF_UNIX)` and the three
  `io_uring_*` syscalls (`seccomp-unix-block.c:64, 99, 113`). It is **not** a syscall allowlist.
- `--new-session` blocks `TIOCSTI` terminal-injection. `--die-with-parent` ties lifetime to the
  parent.

### 2.5 What the Claude Code client passes to the runtime

The local Claude Code client bundles the sandbox runtime and calls it in two layers:

1. Session initialization / settings refresh:

   ```
   DD.initialize(PZ$(settings), sandboxAskCallback)
   DD.updateConfig(PZ$(settings))
   ```

2. Bash command execution:

   ```
   DD.wrapWithSandbox(command, binShell, perCommandSandboxOverride, abortSignal)
   ```

On Linux, `wrapWithSandbox` returns a shell string. The Bash tool then spawns the selected shell
with that wrapped command; the sandbox runtime itself expands that into a `bwrap ... -- <shell> -c`
command. The important runtime config fields emitted by `PZ$` are:

```
{
  network: {
    allowedDomains,
    deniedDomains,
    allowUnixSockets,
    allowAllUnixSockets,
    allowLocalBinding,
    allowMachLookup,
    httpProxyPort,
    socksProxyPort
  },
  filesystem: {
    denyRead,
    allowRead,
    allowWrite,
    denyWrite
  },
  ignoreViolations,
  enableWeakerNestedSandbox,
  enableWeakerNetworkIsolation,
  ripgrep,
  seccomp,
  bwrapPath,
  socatPath
}
```

Observed client-side derivation:

- `network.allowedDomains` comes from `sandbox.network.allowedDomains` plus permission allow
  rules whose content is `domain:<host>`. If managed settings set
  `sandbox.network.allowManagedDomainsOnly`, only managed policy sources contribute allowed
  domains.
- `network.deniedDomains` comes from `sandbox.network.deniedDomains` plus permission deny rules
  whose content is `domain:<host>`.
- `filesystem.allowWrite` starts with the working/project paths and configured additional
  directories, then folds in permission allow rules and `sandbox.filesystem.allowWrite`.
- `filesystem.denyWrite` includes settings, MCP, hook, git, and project-management paths plus
  permission deny rules and `sandbox.filesystem.denyWrite`.
- `filesystem.denyRead` and `allowRead` come from permission rules and `sandbox.filesystem`.
  If managed settings set `allowManagedReadPathsOnly`, only managed policy `allowRead` entries
  are used.
- `bwrapPath` and `socatPath` are passed through from settings. If unset, the Linux wrapper falls
  back to the bare executable names `bwrap` and `socat`; the client does **not** appear to pin
  absolute trusted paths by default.
- In the compiled Bun client, `seccomp` is set to `{ applyPath: "/proc/self/fd/3",
  argv0: "apply-seccomp" }` when embedded files are present. The Bash subprocess opens
  `/proc/self/exe` and passes it as stdio fd 3, so the same executable is invoked as a multicall
  `apply-seccomp` helper inside `bwrap`.

---

## 3. What is contained vs. not (the asymmetry)

This table describes the effect on **Bash-tool subprocesses** when the sandbox is enabled. It
does **not** describe Read/Edit/Write file tools; those are outside `bwrap` and are handled in §4.

| Resource / action | Default sandbox effect | Mechanism |
|---|---|---|
| Read `~/.aws`, `~/.ssh`, `~/.config/gh`, `~/.git-credentials`, `~/.netrc`, shell history | **Not contained** (readable) | Reads allowed globally; requires opt-in `denyRead` |
| Secret env vars (`GITHUB_TOKEN`, `AWS_*`, API keys) | **Not contained** (inherited) | No `--clearenv`; only proxy/CA vars are `--setenv` on top |
| Write `~/.bashrc`, `$PATH` dirs, `.git/hooks`, `.gitconfig`, `.mcp.json` (within cwd) | **Contained** | `--ro-bind / /` + mandatory deny list |
| Write anywhere outside `allowWrite` | **Contained** | Read-only root |
| Outbound egress / `git push` | **Contained** | `--unshare-net` + deny-all proxy |
| Egress to an allowlisted domain | Allowed; hostname-only unless `tlsTerminate` | Proxy filtering; domain-fronting/path-exfil residual |
| `/proc/*/environ`, other same-user processes | **Contained** | PID namespace + fresh `/proc` (+ `/proc` deny, client-side) |
| `AF_UNIX` / `docker.sock` | **Contained** (x86_64/arm64) | seccomp `EPERM` on `socket(AF_UNIX)` |
| Read/Edit/Write/WebFetch/**MCP**/hooks | **Not sandboxed at all** | Only the *Bash* tool is wrapped; others use permission rules |

---

## 4. Execution paths that bypass the sandbox

The sandbox is a perimeter around **one tool**. Code that Claude generates is dangerous through
paths the sandbox never sees:

| Path | Inside bwrap? | Note |
|---|---|---|
| Bash tool subprocess | ✅ (if enabled) | The only OS-sandboxed path |
| Write/Edit tool | ❌ | Permission rules only; can plant deferred-execution payloads |
| WebFetch | ❌ | Not subject to the Bash net-namespace/proxy |
| MCP servers | ❌ | Unsandboxed host processes |
| Hooks (PreToolUse/Stop/…) | ❌ | Host shell commands, full user authority |
| `excludedCommands` | ❌ | Configured to skip the sandbox |
| `dangerouslyDisableSandbox` / `allowUnsandboxedCommands` | ❌ | Escape hatch (default-on) |
| Deferred execution (postinstall, `.git/hooks`, `Makefile`, `.envrc`, CI, `.vscode/tasks.json`) | ❌ | Detonates later — `npm install`, CI, direnv, next commit |

A **compromised skill** is fundamentally a prompt-injection / supply-chain trust problem, which
the sandbox does not address. Its natural payload is *persistence that runs outside bwrap later*
— exactly the supply-chain pattern `blastradius` exists to surface.

### 4.1 File-tool deferred-execution surface

The file tools are an unusually important attack surface because they give the model persistent
effect on the developer's machine/repository without entering the OS sandbox. The sandbox can
prevent a Bash command from writing `.git/hooks`, but it does not automatically prevent the
Write/Edit tools from modifying files that will later execute under the developer, editor, build
system, or CI runner.

High-leverage vectors:

| Vector | Trigger | Why it matters |
|---|---|---|
| Git hook poisoning | Developer later runs normal `git commit`, `git push`, checkout, merge, or rebase operations | Runs outside Claude Code with the developer's normal environment. Repo hooks and `core.hooksPath` can both matter. |
| Shell profile / rc poisoning | New terminal, login shell, SSH session, or shell startup | Durable user-level persistence; can intercept common commands through aliases/functions or environment changes. |
| Build / task-runner tampering | Developer or CI runs install/build/test/lint tasks | Very practical in normal workflows: package scripts, Makefiles, justfiles, Gradle, Cargo build scripts, Python packaging hooks, and similar files are routinely executed. |
| CI/CD config poisoning | Push/PR/merge triggers CI | Often higher impact than local execution because CI may hold publishing credentials, cloud credentials, deploy tokens, or production-adjacent access. |
| Editor / IDE config attacks | Opening the workspace, running a task, accepting recommendations, file-type hooks | Workspace trust varies by tool, but editor tasks/settings/extensions can become execution or credential-access paths outside the sandbox. |
| `$PATH` poisoning | User later runs a common command whose name resolves to a writable earlier `$PATH` entry | Quiet command interception from `~/bin`, `~/.local/bin`, or project-local tool directories when those precede system paths. |
| Git config abuse | Later Git operations or credential flows | `alias.*`, `credential.helper`, `insteadOf`, `core.hooksPath`, and repo/global config can redirect behavior without obvious source-code changes. |
| Environment/config file tampering | App, shell, direnv, framework, or test harness loads `.env*` / config files | Lets a file write change runtime behavior without creating an executable-looking file. |
| User autostart | Login/reboot/session start | Linux user systemd units, desktop autostart entries, and macOS LaunchAgents are true persistence surfaces if writable. |
| Dependency / lockfile tampering | Next install/build or downstream consumer resolves dependencies | Can create delayed supply-chain effects; impact depends on ecosystem lockfile semantics and review discipline. |
| Shebang / interpreter abuse | A modified helper script is later invoked indirectly | A file can look like data or a normal helper while selecting an unexpected interpreter or behavior. |

Read + Write together makes this more targeted: the model can first map the repo and home
configuration (hooks, `.env*`, CI files, build scripts, editor settings, existing trust paths),
then modify the one file most likely to run later. It can also create "slow exfiltration" by
writing sensitive material into files that are later committed, uploaded, logged, or collected by
ordinary tooling. `blastradius` must not perform that attack; it should inventory whether the
corresponding sinks are reachable and writable.

Defenses that actually address this class:

- Treat file read and file write as separate capabilities, with a strict workspace boundary by
  default and explicit approval for expansion.
- Make high-risk paths read-only or strongly prompted: git hooks/config, shell rc/profile files,
  `$PATH` directories, CI configs, editor task/config files, autostart paths, dependency manifests
  and lockfiles, and credential-adjacent config.
- Prefer allowlisted write directories over broad repo/home writes, and apply deny rules to
  deferred-execution sinks even when they live inside the workspace.
- Review MCP servers, hooks, skills, and file-edit permissions as code-execution authority, not
  as harmless configuration.

---

## 5. Audit findings

Severity is this author's judgment. "In threat model" = whether it falls within what the
user-level sandbox claims to defend. Items marked *accepted* are explicitly acknowledged
tradeoffs in the source, not hidden bugs.

| # | Finding | Severity | In TM? | Source |
|---|---|---|---|---|
| 1 | **`bwrap`/`socat` resolved via `$PATH` by default**; exec uses the bare name unless settings provide absolute `bwrapPath`/`socatPath`; no integrity/signature check. The inspected Claude Code client passes those paths through from settings and otherwise falls back to the runtime default bare names. (`whichSync` is used only for the dependency *check*, not the exec.) | High | Yes | `linux-sandbox-utils.ts:1290, 501, 673`; local client `PZ$`/`lj7`/`cj7` |
| 2 | **No `--clearenv`** — the child inherits the full parent environment; credentials in env pass through. | High | Yes | `wrapCommandWithSandboxLinux` (no clearenv) |
| 3 | **No write restriction ⇒ `--bind / /` (full host RW).** Controls are independent; enabling only network leaves the filesystem writable. | High | Config foot-gun | `:896` |
| 4 | **Seccomp is default-ALLOW**, denying only `socket(AF_UNIX)` + `io_uring_*` — not a syscall allowlist; full kernel-LPE surface remains. | Med | Accepted | `seccomp-unix-block.c:64,99,113` |
| 5 | **ia32 `socketcall()` bypass** — a 32-bit x86 process can create `AF_UNIX` via `socketcall`, uncovered by the x86_64 filter. | Med | Yes (niche) | `seccomp-unix-block.c:11-22` |
| 6 | **Mandatory-deny scan bounded to `--max-depth 3`** — dangerous files nested deeper in a monorepo are not write-protected within the writable tree. | Med | Yes | `:65, 230-243` |
| 7 | **Fail-open seccomp in the standalone runtime** — if `apply-seccomp` is unavailable (unsupported arch / missing global install), it warns and continues without unix-socket blocking. The inspected compiled client improves this by passing its own executable as fd 3 and using `ARGV0=apply-seccomp`; the fail-open condition still matters when that embedded helper path is unavailable or when using the runtime outside the compiled client. | Med | Yes | `:1133-1143`; local client `sD7`/`tD7` |
| 8 | **Inherited-FD / `SCM_RIGHTS` gap** — seccomp blocks socket *creation*, not use of an inherited/received unix-socket FD. | Low-Med | Accepted | `:1051-1054` |
| 9 | **Ghost mount-point files** written to host for non-existent deny paths; cleanup is deferred and skipped on `SIGKILL` (only `process.on('exit')`). | Low | Robustness | `:286-378, 818-876` |
| 10 | **Missing namespaces** — no `--unshare-ipc`/`-uts`/`-cgroup`; host IPC and cgroup info exposed. | Low | Partial | argv has only `--unshare-net/-pid` |
| 11 | **HTTPS filtered hostname-only unless `tlsTerminate`**; TLS termination relies on env-injected CA, evadable by pinning/env-ignoring clients; domain-fronting/path-exfil to allowed hosts residual. | Med | Documented | `request-filter.ts:8`, `sandbox-utils.ts:303-313` |
| 12 | **`NO_PROXY` includes `169.254.0.0/16` + RFC-1918** — harmless under `--unshare-net`, but in network-allowed mode the sandbox reaches cloud metadata and the LAN directly, unproxied. | Low-Med | Config-dependent | `sandbox-utils.ts:341-352` |
| 13 | **File tools can plant deferred-execution sinks outside the OS sandbox** — Write/Edit can modify git hooks, shell profiles, build scripts, CI configs, editor tasks, git config, autostart files, and lock/manifests that execute later under non-sandboxed developer or CI authority. | High | Outside Bash sandbox | §4 / §4.1; local client only wraps Bash |

### Headline

Finding **#1** is the one to push on: the runtime's default is `$PATH` resolution of its own
enforcement binaries with no integrity check, so a PATH-shadow (a malicious earlier-`$PATH`
`bwrap`, which the unsandboxed Write tool can drop) makes the "sandbox" attacker-controlled while
reporting success — a **bootstrapping/TOCTOU** weakness in the class of runc CVE-2019-5736. The
only mitigation is the caller passing absolute, trusted `bwrapPath`/`socatPath` values. In the
inspected Claude Code client, those values are not pinned by default; they are passed through from
settings when present, otherwise the runtime uses `bwrap`/`socat` by name.

---

## 6. How `blastradius` measures this

These probes turn the model above into a live, value-free inventory:

| Probe id | Class | Measures |
|---|---|---|
| `claude_code.sandbox_posture` | SystemInfo | Reads Claude Code settings (names/counts only): `sandbox.enabled`, MCP-server count, hook count, escape hatches (`excludedCommands`, `allowUnsandboxedCommands`), `denyRead` presence — the unsandboxed surface (§4) |
| `process.sandbox_reach` | Process | `AF_UNIX`/`docker.sock` reachability and same-user `/proc/*/environ` exposure, with a PID-namespace-isolation hint (§2.4, findings #5/#8). Emits `process.afunix_docker_sock` and `process.proc_environ`. |
| `host.writable_persistence_paths` (+ `host.writable_git_hooks`) | HostPersistence | Read-only mode/ownership check of persistence sinks an agent could write to that execute **outside** bwrap — shell rc/profiles & home dotfiles, `$PATH` dirs (escalation), `.gitconfig`, and git hooks / `core.hooksPath` (findings #6/#13) |
| `env.subprocess_scrub` | Credentials | Whether env-borne credentials are scrubbed (`CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`) or inherited (finding #2) |
| `host.sandbox_binary_integrity` | HostPersistence | Resolves `bwrap`/`socat` exactly as a bare-name exec would, then flags whether that binary is **replaceable in place** (writable file/dir) or **PATH-shadowable** (a user-writable dir precedes it on `$PATH`) — finding #1, the runc CVE-2019-5736 class. Escalates if a setuid-root binary sits on a writable path; **downgrades to Info** when `sandbox.bwrapPath`/`socatPath` pin an absolute, non-writable path (managed scope wins). |
| `process.sandbox_detect` | Process | Full self-detection: per-namespace isolation (net/pid/ipc/uts/cgroup/user — exposes finding #10 when run inside a sandbox), active seccomp filter, `AF_UNIX` block, `HTTP(S)_PROXY`, `/proc/self/environ` reach, and ia32-execution support (the `socketcall` bypass surface, finding #5). Emits a `sandboxed: yes/likely/no` verdict — the framing signal for every other finding. |
| `host.deferred_exec_sinks` (+ `host.autostart_sinks`) | HostPersistence | Repo sinks that run **outside** bwrap later — Makefile/justfile, `.envrc`, `package.json` lifecycle scripts, CI workflows, `.vscode` tasks, lockfiles — plus user-autostart dirs; presence + writability (findings #6/#13) |
| `egress.mediation` | Egress | Proxy mediation (`HTTP(S)_PROXY`) + the hostname-only / no-TLS-inspection caveat (finding #11), and opt-in cloud-metadata reachability (`--check-metadata`, finding #12) |
| `egress.connectivity` | Egress | Outbound reachability (DNS + TLS) to a neutral host |
| `claude_code.writable_control_surface` (+ `.repo`) | HostPersistence | Read-only writability check of the files that *constrain or instruct the agent*: `settings.json`/`settings.local.json`/`.mcp.json` (writing them disables the sandbox / adds hooks) and `CLAUDE.md`/`AGENTS.md`/skills (durable prompt injection). Own-writable = Notable, non-owner-writable = Exposed (§4 / §4.1, finding #13). |
| `ssh.agent_socket` | Credentials | Whether `SSH_AUTH_SOCK` is reachable and how many identities are loaded (count only, via `REQUEST_IDENTITIES`) — loaded keys are usable for auth without reading any key file, including passphrase-protected keys |
| `git.config_exec_directives` (+ `.local`) | GitWrite | Existing git-config directives that exec/redirect on ordinary git ops — shell `alias.*`, `core.sshCommand`/`fsmonitor`, content `filter.*`, `diff.external` (exec), and `core.pager`/`core.hooksPath`/`*.insteadOf` (redirect). Categories/keys only, never values (§4.1 "Git config abuse"). |

### Credential & cluster reach (spec-driven store family)

The original AWS probe is now one of a **data-driven family**: each store is a
`StoreSpec` entry (`src/probes/store.rs`) run by one engine, so the inventory
below is extended by adding a few lines of data, not a new probe. All are
`Credentials`-class, `Ambient`-scope, read-only, and emit identifier
names/counts only — never values.

| Probe id | Reaches | Extracted (value-free) |
|---|---|---|
| `aws.credentials` | `~/.aws` (+ `AWS_*_FILE`) | profile names |
| `gcp.credentials` | `~/.config/gcloud` (+ `CLOUDSDK_CONFIG`) | ADC / token-db presence, account count |
| `azure.credentials` | `~/.azure` (+ `AZURE_CONFIG_DIR`) | token-cache presence |
| `kube.config` | `~/.kube/config` + `$KUBECONFIG` | cluster + context names |
| `docker.registry_auth` | `~/.docker/config.json` (+ `DOCKER_CONFIG`) | registry hostnames |
| `vault.token` | `~/.vault-token` | presence |
| `npm.token` | `~/.npmrc` (+ `NPM_CONFIG_USERCONFIG`) | registry hosts with auth |
| `pypi.token` | `~/.pypirc` | index names |
| `cargo.token` | `~/.cargo/credentials*` (+ `CARGO_HOME`) | registry names |
| `terraform.token` | `~/.terraformrc`, `~/.terraform.d/credentials.tfrc.json` | host names |
| `postgres.pgpass` | `~/.pgpass` (+ `PGPASSFILE`) | host names (never the password column) |
| `gpg.private_keys` | `~/.gnupg` (+ `GNUPGHOME`) | secret-key count |
| `aws.sso_cache` | `~/.aws/sso/cache`, `~/.aws/cli/cache` | live SSO/CLI token count (the profile probe misses these) |
| `kube.pod_token` | `/var/run/secrets/kubernetes.io/serviceaccount/token` | in-pod service-account token presence |
| `container.registry_auth` | `~/.config/containers/auth.json`, `$XDG_RUNTIME_DIR` | podman/skopeo registry hostnames |
| `keyring.secret_store` | `~/.local/share/keyrings`, `kwalletd`, macOS `login.keychain-db` | keyring/wallet count (the Secret Service master store) |
| `ai_assistant.credentials` | `~/.claude/.credentials.json`, Copilot, Codeium, Cursor | the agent's own/sibling assistant token presence |
| `saas_cli.tokens` | Vercel, Netlify, Fly, doctl, Supabase, Sentry, Helm, CircleCI, ngrok, Wrangler | per-CLI token-file presence |
| `vpn.credentials` | `/etc/wireguard`, `~/.config/wireguard`, Tailscale state | tunnel key/state presence |
| `jupyter.runtime` | `~/.local/share/jupyter/runtime` | live notebook-server token count (RCE) |
| `onepassword.cli` | `~/.config/op`, `~/.op` | live `op` session presence |
| `cloud_init.user_data` | `/var/lib/cloud/instance/user-data.txt` | bootstrap-secret-bearing user-data presence |

Commonly-missed surfaces probed outside the store family:

| Probe id | Class | Measures |
|---|---|---|
| `browser.session_stores` | Credentials | Presence/profile-count of Chromium (Chrome/Chromium/Brave/Edge) cookie jars + `Login Data`, and Firefox `cookies.sqlite`/`logins.json` — a cookie jar is a bearer credential for every logged-in site (session hijack past password+MFA). No DB is opened. |
| `credentials.repl_history` | Credentials | Secret-looking line counts in DB-client and REPL histories (`~/.psql_history`, `~/.mysql_history`, `~/.python_history`, `~/.node_repl_history`, pgcli/mycli, …) — connection strings and inline passwords typed interactively, as readable as shell history. Counts only. |
| `host.privilege_escalation` | Process | Local root-escalation reach: root-equivalent group membership (`docker`/`lxd`/`libvirt`/`kvm`) via `id -nG`, and passwordless sudo via `sudo -n -l` (a non-interactive **list** — no command is run as root). An agent in the `docker` group or with `NOPASSWD` sudo can take over the host without any exploit. |
| `process.memory_introspection` (+ `process.cmdline_secrets`) | Process | `kernel.yama.ptrace_scope` — when 0/absent, the agent can dump the live memory of any same-uid process (ssh-agent/gpg-agent keys, browser sessions, password managers) with no file on disk; plus secret-shaped args in same-uid `/proc/*/cmdline` (counts only). |
| `host.local_services` | Process | Loopback TCP-connect to well-known datastore/admin ports (Postgres/MySQL/Redis/Mongo/ES/etcd/Vault/…) — local services often trust localhost with no auth, and the agent reaches them. No bytes sent; nothing leaves the machine. |
| `gpg.agent_socket` | Credentials | A reachable `gpg-agent` socket (the GPG analogue of ssh-agent): a cached passphrase lets it decrypt/sign as you without the passphrase or key file. Presence only. |
| `host.network_config` | HostPersistence | Writability of `/etc/hosts` / `/etc/resolv.conf` (silent domain/DNS redirect → MITM) and readability of NetworkManager stored WiFi/VPN secrets. Permission check only. |
| `host.privileged_reachability` | Process | **Conditional, post-escalation blast radius:** inventories the root-only assets (shadow, SSH host keys, root creds, k8s admin.conf, Docker/DB data, kernel memory) that become reachable IF the agent reaches root — via a detected escalation path (`docker`/`sudo` group) and/or a local kernel LPE (sandbox seccomp is default-allow, findings #4/#5). It **never escalates and never reads a root file** — only `stat`/permission metadata. A root file that is *already* readable (e.g. group-readable `/etc/shadow`) is surfaced as a live misconfiguration. |

The credential-store family now spans ~35 stores (AWS/SSO · GCP · Azure · Kube ·
Docker/podman · Vault · npm/PyPI/Cargo/Maven/Gradle/Composer/RubyGems/pip/NuGet ·
dbt/Databricks/Snowflake/MySQL/pgpass · GPG · OS-keyring · SOPS-age/Teleport/
password-managers/Ansible-vault/rclone · VPN/cloudflared · Jupyter/1Password ·
SaaS-CLI · AI-assistant · mail · legacy-cloud · conda · atuin · cloud-init ·
container `/run/secrets`), each a value-free `StoreSpec` data entry.

### AI blast-radius narratives (`dashboard --ai`)

`blastradius dashboard` serves a local, self-contained web dashboard of the
reachable surface. With `--ai`, it sends the **value-free** inventory (finding
ids/classes/severities/titles/summaries — never secret values, re-swept before
send) to the OpenAI API and renders defender-oriented attack-path scenarios
(chain · impact · containment) grounded only in what the scan found. This is the
one feature that egresses anything, and it is opt-in; `--offline` disables it.

All probes are read-only and never emit secret values (see [the redaction layers](../SPEC.md)).

### Findings not host-detectable (documented only)

A handful of findings are properties of the runtime's *design*, not observable from a host
self-audit, so they remain documented rather than probed:

- **#3 (no write restriction ⇒ full host RW)** — the Claude Code client always injects the
  workspace into `allowWrite` programmatically (§2.5), so an empty `allowWrite` in *settings* is
  not evidence of full-RW; the gap is a property of using the bare runtime without write config.
- **#4 (seccomp default-ALLOW), #8 (inherited-FD / `SCM_RIGHTS`), #9 (ghost mount-point files)**
  — internal runtime behavior, not reflected in host state.
- **#7 (fail-open seccomp)** — depends on the client's embedded `apply-seccomp` availability at
  exec time, which isn't observable from a separate process. (`process.sandbox_detect` does
  report whether a seccomp filter is active in *its own* process.)

---

## 6a. Coverage map (what the reachable-surface inventory spans)

The inventory aims to be exhaustive over the ambient authority a same-user agent
inherits. By taxonomy:

| Surface class | Covered |
|---|---|
| **Cloud/cluster identity** | AWS(+SSO cache), GCP, Azure, Kubernetes (config + in-pod token), Docker/podman registry, cloud-init user-data, container `/run/secrets`, IMDS (opt-in) |
| **Secrets managers / decryptors** | HashiCorp Vault, SOPS/age keys, 1Password/Bitwarden/LastPass/`pass`, GPG keys **and gpg-agent**, OS keyring (GNOME/KWallet/Keychain), Ansible-vault, Teleport |
| **Package/registry/build** | npm, PyPI, Cargo, Terraform, Maven, Gradle, Composer, RubyGems/Bundler, pip, NuGet, Conda |
| **Data/DB** | dbt, Databricks, Snowflake, MySQL client, `.pgpass`, reachable localhost datastores |
| **VCS/identity** | SSH keys + **ssh-agent**, GitHub/git creds (+XDG), git exec-config directives, push-likelihood |
| **Network identity** | VPN (WireGuard/Tailscale), cloudflared, rclone, SaaS CLIs, mail clients, legacy cloud (`.s3cfg`/`.boto`/`.dockercfg`) |
| **Env / history** | secret-named env vars, subprocess scrub, shell + DB/REPL histories, Atuin sync key |
| **Cross-repo** | sibling repos, lateral secret/key files (incl. tfstate, Rails `master.key`) |
| **Process / kernel** | sandbox detection/reach, `/proc/*/environ` + **`/proc/*/cmdline`**, **ptrace/memory introspection**, AF_UNIX/docker.sock, **privilege escalation** (groups + NOPASSWD sudo) |
| **Host persistence / deferred exec** | shell rc + editor/login dotfiles, `$PATH` shadowing, git hooks, build/CI/editor/dev-env sinks, autostart + **cron/systemd timers**, sandbox-binary integrity, Claude control/instruction surface, network-config tampering |
| **Egress** | DNS+TLS reachability, proxy mediation, cloud-metadata (opt-in) |

**Deliberately out of scope** (documented, not silently missing):

- **Root-only system files** (`/etc/shadow`, host keys, root creds, kernel memory) — not
  reachable as the agent user, so they are not read. They ARE inventoried as *conditional,
  post-escalation* reachability by `host.privileged_reachability` (stat/permission metadata
  only — no escalation, no content read), and any that are mis-permissioned to be readable
  now are surfaced. We never assume root and never attempt to gain it.
- **Heavy/cross-platform desktop app stores** (Slack/Discord/Teams LevelDB, Thunderbird
  profiles, full browser history DBs) — browser *cookie + login* stores are covered (the
  high-signal part); enumerating every Electron app's local state is open-ended and low-yield.
- **Deep niche dev tools** (Hex/Elixir, CocoaPods, Chef/Puppet/Salt, Consul/Nomad, SBT/Ivy) —
  the spec-driven store engine makes each a few lines if a specific environment needs it.
- **Secret *values*** — by inviolable design (§4.2): only reachability, names, and counts.

This map is the basis for the claim that the inventory is comprehensive: every class of
ambient authority a coding agent inherits is represented; remaining items are either
root-gated, open-ended desktop-app state, or niche tools addable as one `StoreSpec`.

---

## 7. Caveats / what was not verified

- Runtime source findings reflect the open-source `sandbox-runtime` at a point in time. Client
  findings reflect only the locally installed `@anthropic-ai/claude-code@2.1.169` Linux x64
  binary listed in §0.
- The inspected client bundles sandbox-runtime code and passes runtime config as described in
  §2.5. Future client builds may change the defaults, pass pinned binary paths, enable
  `tlsTerminate`, or change `denyRead` / env-scrub behavior.
- Version-specific facts not visible in the binary still come from public docs/changelogs.
- macOS (Seatbelt / `sandbox-exec`) and Windows are out of scope here.
- Findings #4, #5, #8, #11 are explicitly acknowledged tradeoffs in the source. #1, #2, #3, #6,
  and #13 are the ones a reviewer should weigh most.

## 8. References

- Runtime source: `src/sandbox/linux-sandbox-utils.ts`, `src/sandbox/sandbox-utils.ts`,
  `src/sandbox/request-filter.ts`, `vendor/seccomp-src/seccomp-unix-block.c`
  (<https://github.com/anthropic-experimental/sandbox-runtime>)
- Docs: <https://code.claude.com/docs/en/sandboxing>, <https://code.claude.com/docs/en/security>
- Engineering writeup: <https://www.anthropic.com/engineering/claude-code-sandboxing>
- Prior art on the substitution class: runc CVE-2019-5736
