# Integrating agent-beacon Threat Rules (CEL/JSON) into blastradius

**Status:** design — proposed
**Date:** 2026-06-13
**Scope:** how the agent-beacon "Threat Rules v1" standard (YAML + CEL over Beacon
Event JSON, session correlation, conformance fixtures) and the `beacon scan --json`
CLI relate to blastradius's session layer (`src/session/`), and the concrete,
staged plan to adopt the valuable parts without breaking blastradius's hard
constraints.

> Grounded against `agent-beacon` submodule `v0.0.60-3-ged012e0` and the live
> `src/session/` tree. This doc supersedes the earlier ad-hoc analysis and folds
> in an adversarial constraint review (value-free / determinism / dependency /
> supply-chain). Where the review found the first-pass plan wrong, the corrected
> position is stated inline and marked **[CORRECTION]**.

---

## 0. Executive summary

1. **blastradius already has a session engine.** `src/session/` is built and wired
   into `sessions` / `audit-history` / `dashboard`. SPEC §11/§23/§24 still describe
   it as "design only" — **that is stale** and should be reconciled (Stage 0).
   blastradius already *has* a rule engine; it is hardcoded Rust predicates that
   are 1:1 with what beacon's CEL expresses declaratively.

2. **The integration is not greenfield and not a port.** It is: adopt beacon's
   best *disciplines* (embedded fixtures + conformance gate + maturity ladder +
   pinned version) and its *ordered-correlation* idea, plus emit a value-free
   beacon-shaped projection for out-of-process consumption — while keeping the
   JOIN, scoring, ingestion, retro, and redaction bespoke in Rust.

3. **Do NOT** pull `cel-go` or any CEL evaluator into the Rust binary, **do NOT**
   shell out to the `beacon` binary, and **do NOT** make beacon's output re-enter
   blastradius scoring. Each breaks a hard constraint (deps/single-binary,
   git-only shell-out, value-free, determinism, supply-chain brand).

4. **The moat is the JOIN.** beacon detects *intent* over an event stream;
   blastradius scores *reachability* by joining observed events against the real
   local probe baseline. CEL has no variable to bind a `FindingId` or the
   `confidence ≥ Likely ∨ severity ≥ Notable` present-gate. That asymmetry is the
   product and is not replaceable.

---

## 1. Ground truth: agent-beacon Threat Rules

### 1.1 Rule format (`pkg/asymptoteobserve/threatrules/rule.go`, `spec/threat-rules/`)

```yaml
id: secret-read-then-egress      # kebab slug ^[a-z0-9][a-z0-9-]*$
version: 1                       # int >= 1
title: ...
description: ...
severity: high                  # info|low|medium|high|critical
status: stable                  # experimental|stable|deprecated  (maturity ladder)
posture: detect                 # detect|enforce-capable  (only detect acted on)
taxonomy: { owasp_llm: LLM06, mitre_atlas: AML.T0024 }
# EXACTLY ONE of match / correlation:
match: '<CEL bool expr>'                       # single-event rule
correlation:                                   # multi-event rule
  scope: session                               # only 'session' in v1
  window: 120s                                  # Go duration
  steps:                                        # ordered, >= 2
    - { id: read_secret, match: '<CEL>' }
    - { id: egress,      match: '<CEL>' }
emit: { reason: '...' }          # required, non-empty
tests:                           # embedded fixtures, >= 1
  - name: ...
    verdict: match|no_match
    events: [ <BeaconEvent>, ... ]
```

- **CEL** binds the event as `e` with JSON field paths mirroring the Go `Event`
  struct: `e.event.action`, `e.command.command`, `e.file.path`, `e.session.id`,
  `e.prompt.text`, … Null-safe (missing sub-objects → zero value). RE2 via
  `.matches()`. Compiled & type-checked at load (`CompileMatch`); unknown fields
  or non-bool expressions are load-time errors.
- **Maturity ladder** (`lint.go::CheckMaturity`): `experimental` ≥1 fixture;
  `stable` requires **≥1 `match` AND ≥1 `no_match`** fixture, all passing;
  `deprecated` relaxes fixtures.
- **Correlation algorithm** (`correlation.go`): group by `session.id`; for each
  session try every step-0 match as an **anchor**; greedily match later steps in
  order (step *i+1* on an event strictly after step *i*); accept if
  `last_step_time − anchor_time ≤ window`. **Window is silently skipped if any
  timestamp is missing/unparseable.** Multiple anchors are tried.
- **Conformance** (`conformance.go::CheckRule`): validate → maturity gate →
  compile CEL → run every fixture → `[]FixtureResult`. `Verdict` is two-state
  (`match`/`no_match`).

### 1.2 The corpus — **19 rules** (not 6) **[CORRECTION]**

The first scouting pass undercounted. The shipping corpus is **19** `.rule.yaml`
across 7 categories:

| category | rules |
|---|---|
| context-exfiltration | browser-session-read-then-egress, credentials-in-curl-data, secret-read-then-egress *(correlation)*, secret-read-then-external-mcp |
| credential-access | browser-session-store-read, credential-file-read |
| external-access | external-mcp-tool-call |
| prompt-injection | exfiltrate-system-prompt, ignore-previous-instructions |
| risky-command | base64-decode-piped-to-shell, curl-pipe-to-shell, privileged-container-run, recursive-root-delete, world-writable-chmod |
| sensitive-edit | auth-security-code-modified, cicd-pipeline-file-modified, dependency-manifest-modified |
| source-control | git-config-exec-injection, git-remote-tampering |

`event.action` values therefore span at least `command.executed`, `file.read`,
`file.modified`, `prompt.submitted`, `mcp.tool_invoked`. **Any field-pinning or
drift test must be derived against all 19 rules, not 6.**

### 1.3 The CLI (PR #158)

- `beacon scan [--json --min-severity --session --fail-on --rules --log-path]` —
  reads beacon's **own** runtime JSONL (`~/.beacon/endpoint/runtime.jsonl` via
  `lifecycle.ResolveRuntimeLog`), evaluates the local rule pack, emits an array of
  `Finding{rule_id, title, severity, posture, session_id, reason, events[]}`.
  **`Finding.events[]` carry RAW `command.command` / `file.path` / `prompt.text`.**
- `beacon rules {list,add,remove,pull,lint,fields}` — `pull` is the **only**
  networked subcommand.

### 1.4 Known upstream bug (inherited-risk note)

`cli/beacon/internal/endpoint/detect/detect.go:192` `InstallFiles` calls
`CheckRule` but checks only the **error** return, ignoring per-`FixtureResult.OK()`
verdicts — so a rule whose embedded fixtures *fail* can still be installed into the
store. **blastradius's port must iterate per-fixture and fail on `!ok`.** It also
means a hypothetical `beacon scan` consumer cannot assume the active pack is
conformance-clean.

---

## 2. Ground truth: the existing blastradius session engine

```
discover_sessions()                  # discovery/ — 14 agents' NATIVE transcripts → AgentEvent (Layer-0, values dropped)
  → normalize()                      # normalize.rs — Layer-1 redaction; AgentEvent → NormalizedEvent{signal,event_ix,approved,join_key}
    → classify()                     # classify.rs — THE JOIN: Signal × baseline Finding → Reason{finding_ref}
      → toxic_combinations::evaluate # 6 rules: event_triggers (unordered) × finding_triggers (FindingId set)
        → score_session / simulate_containment  # score.rs — weights × multipliers + path-weights; 5-control sim
          → build_session_report     # report.rs — SessionReport (Layer-2 swept)
retro.rs / history.rs                # retro-hazard: re-resolve historical combos vs today's baseline
```

Key facts that constrain the integration:

- `AgentEvent` (`trace.rs`) has **no per-event timestamp**; only
  `SessionTrace.started_at` (session-level, optional). `NormalizedEvent.event_ix`
  is the only ordering key.
- The hardcoded predicates (`is_secret_store_path`, `is_deploy_path`,
  `is_container_runtime_shape`, `reduce_command`, `reduce_host`, …) are
  hand-written equivalents of beacon CEL match expressions.
- Three redaction layers: Layer-0 (`discovery/extract.rs` drops `diff`/`input`/
  `reason`, `reduce_command` argv allowlist + `looks_secretish` entropy guard),
  Layer-1 (`normalize.rs` drops `diff`, sweeps command/host, path/url shape gate),
  Layer-2 (`report::redaction::sweep` before render) + the **canary self-test**.
- **[CORRECTION]** The canary test is **not** a general invariant. It is a single
  test (`report.rs::canary_does_not_leak_through_session_renderers`) that iterates
  exactly `[render_terminal, render_json]`. Any *new* egress path is not covered
  until explicitly added to that loop.
- **Authored impact layer** (`src/dashboard/impact.rs`, commit `0368556`):
  `finding_impact(id) -> (why, how)` for all **78 FindingIds** and
  `signal_impact(signal) -> why` for all **9 Signal names** — hand-authored,
  value-free, reachability-framed, **engine-owned** (per-class fallback only as a
  safety net for future ids). **It is keyed by the Rust `FindingId` + `Signal`-name
  taxonomy — the same identity the JOIN (`classify.rs`) and the toxic catalog use.**
  This is independent evidence that the event taxonomy must stay bespoke: making
  rules CEL/YAML-keyed would orphan the entire hand-authored explanation layer.

---

## 3. Integration thesis

> beacon = the sensor + intent rules (the numerator's raw signal).
> blastradius = the static reachability map (the denominator) + the JOIN that turns
> a match into a blast-radius path *on this machine*.

What is **replaceable / adoptable** (all event-side, pre-JOIN):

- the hardcoded event-classification + the unordered toxic-gate → beacon's
  *ordered correlation* idea and *rule discipline*;
- ad-hoc `#[cfg(test)]` rule tests → beacon's embedded fixtures + conformance gate
  + maturity ladder + pinned version.

What is **the moat / keep-bespoke** (the JOIN and everything downstream):

- `classify.rs` (Signal × live `Finding` baseline, gated by `finding_is_present`);
- `score.rs` (deterministic weights/multipliers/path-weights + containment sim);
- `discovery/` (14 native transcript formats — a strict superset of what
  `beacon scan` could see);
- `retro.rs` / `history.rs` (remediation lifecycle + recency decay);
- the three redaction layers + canary.

CEL has no `FindingId` variable, no reachability gate, no numeric score, and no
remediation/recency model. The asymmetry is structural, not incidental.

---

## 4. Seam-by-seam design

Five seams, with verdicts from the grounding + cross-walk and the adversarial fixes
folded in as **MUST** requirements.

### Seam A — Event-schema projection (`AgentEvent → BeaconEvent`) — **AUGMENT**

**Goal.** Emit a value-free, beacon-shaped JSONL projection so blastradius sessions
can be consumed *out-of-process* by `beacon scan` / the beacon ecosystem, and so
beacon rule ids can be cross-referenced. This is an **export**, one-way.

**Direction is fixed: projection, never ingest.** beacon CEL reads value-bearing
fields blastradius destroys at Layer-0/1. A faithful `BeaconEvent → AgentEvent`
ingest would re-introduce raw bytes — forbidden. So the seam is `AgentEvent`
(via the normalized stream) → `BeaconEvent`, **lossy by design**.

**New module** `src/session/beacon_view.rs`:
`pub fn to_beacon_events(trace, normalized) -> Vec<BeaconEvent>` + a
`#[derive(Serialize)] BeaconEvent` mirroring only the subset of beacon JSON tags
the **19-rule** corpus touches.

**Canonical mapping** (value-free sources only):

| AgentEvent | `event.action` | populated value-free fields |
|---|---|---|
| `FileRead{path}` | `file.read` | `file.path` ← path shape gate |
| `FileWrite{path,diff}` | `file.modified` | `file.path` ← shape gate; `diff` **dropped** |
| `ShellCommand{command}` | `command.executed` | `command.command` ← swept reduced shape |
| `NetworkAccess{host,port}` | *(no native beacon action)* | host token only; **gap** |
| `Approval{…}` | `approval.*` | decision only; `reason` dropped |
| `McpCall{server,tool,input}` | `mcp.tool_invoked` | gated `server`/`tool`; `input` dropped |

Plus `session.id` ← `SessionTrace.session_id`, `event.timestamp` ← deterministic
synthesized offset (below).

**MUST (value-free):**
- **A1.** Populate every value-bearing field **exclusively from
  `NormalizedEvent.join_key`** (already `sweep(reduce_command(...))` / `shape_gate`
  / `reduce_host`). **Never read `AgentEvent::{ShellCommand.command, FileRead.path,
  FileWrite.path, NetworkAccess.host, McpCall.*}` value strings.** `reduce_command`
  *alone* is insufficient — `normalize.rs:341` wraps it in `sweep()` precisely
  because short/path-ish operands survive the reducer. Add a test asserting
  `beacon_view` references no raw value field.
- **A2.** Route the entire serialized JSONL through `crate::report::redaction::sweep`
  as the **last** step before return (mirror `report/json.rs:75`).
- **A3.** **Multi-signal collapse rule:** `normalize()` fans one source event into
  several `NormalizedEvent`s (e.g. `ShellCommand` + `DangerousShellPattern`). Emit
  **one** `BeaconEvent` per source `event_ix`; for a shell event,
  `command.command := join_key of the Signal::ShellCommand normalized event`;
  other signals contribute only to action/category tags. This makes the value-free
  source explicit so nobody "fixes" false-negatives by substituting raw text.
- **A4.** Extend the canary loop in
  `report.rs::canary_does_not_leak_through_session_renderers` to include the beacon
  JSONL renderer, planting `br_test_SHOULD_NOT_LEAK` + a `ghp_`-shaped token in
  `FileWrite.diff`, `McpCall.input`, `ShellCommand.command`, `FileRead.path`, **and**
  `NetworkAccess.host`.

**MUST (determinism):**
- **A5.** Synthesize `event.timestamp` as a **pure function** of
  `(started_at-or-fixed-epoch, event_ix)` — e.g. `base + event_ix*1s` — with no
  clock read and no map iteration. Add a byte-for-byte determinism test.

**Known lossy gaps (document, do not "fix" by enriching `AgentEvent`):**
- No prompt variant → the 2 prompt-injection rules are **unreachable** via
  projection. Adding a prompt variant would retain prompt text → value-free
  violation. Leave open.
- No `network.*` action → structured `NetworkAccess` has no 1:1 beacon action.
- Redacted shapes (`[redacted:len:N]`, `[custom egress target]`) **defeat**
  value-hungry beacon regexes (`curl\s+.*https?://`, `-d @.env`). Projected events
  therefore produce **false negatives** against value-matching rules. **Never
  present beacon-CEL pass/fail over the projection as equivalent to blastradius
  classification.**

Effort: **M**.

### Seam B — Ordered correlation in the toxic-gate — **AUGMENT**

**Goal.** Bring beacon's *ordered, session-scoped* correlation to the toxic-combo
event-leg, which today is an **unordered** AND-gate.

**Design.**
- Add optional `event_order: Option<Vec<EventPredicate>>` to
  `ToxicCombinationRule` (default `None` = today's behavior; fully backward-compat).
- Refactor `event_trigger_matched` → `event_trigger_ixs(tag, &[NormalizedEvent]) ->
  Vec<usize>` so the bool any()-gate and the ordered walk share one predicate
  source (no second classifier, no value-free regression).
- When `event_order` is `Some`, after clause (a) membership + clause (b)
  `finding_triggers`, additionally require an assignment where each tag matches a
  `NormalizedEvent` with `event_ix` strictly greater than the prior — **greedy
  earliest-match anchored on earliest `event_ix`** (mirror beacon `completeFrom`,
  over `event_ix` instead of timestamps).

**MUST / boundaries:**
- **B1.** **Drop beacon's `window`.** `AgentEvent` has no per-event timestamp, so a
  time window is structurally inexpressible. Dropping it is *faithful* to beacon's
  own no-timestamp degraded mode (it skips the window then). Do **not** invent an
  event-count proxy window.
- **B2.** `finding_triggers` (the JOIN leg) stays 100% Rust — beacon CEL cannot
  express it.
- **B3.** Keep `retro.rs::LegOrdering` a **reported confidence signal, not a gate**,
  to avoid double-counting order. Do not add an ordering multiplier in `score.rs`.
- **B4.** Ship `event_order: None` on all current rules in this change (pure
  additive plumbing). Flip individual rules to ordered only in a follow-up that
  ships new fixtures, so no currently-passing retro fixture regresses.
- **B5.** Determinism: iterate in `event_ix` order; never HashMap iteration.

Effort: **M**.

### Seam C — Conformance / maturity / embedded fixtures — **AUGMENT**

**Goal.** Replace scattered `#[cfg(test)]` rule tests with beacon's
fixtures-per-rule + conformance gate + maturity ladder + pinned version — but
**Rust-native, not shared YAML.**

**Design.** New `src/session/conformance.rs`:
- `enum Verdict { Match, NoMatch }`
- `struct RuleFixture { name, verdict, events: Vec<AgentEvent>, baseline:
  Vec<Finding>, expect_rules: Vec<&'static str> }` — note fixtures carry **both**
  events **and** a baseline `Finding` set, because the JOIN gate needs both
  (beacon fixtures carry only events; beacon has no JOIN).
- `check_rule` / `check_maturity` ported from `conformance.go` / `lint.go`
  (`stable` ⇒ ≥1 Match **and** ≥1 NoMatch).
- `fn pack_conformance()` `#[test]`: iterate `catalog()`, assert every rule has
  fixtures, run each through `normalize` + `evaluate` (or `classify`), assert
  produced rule/signal names == `expect_rules`.
- Add `status: Status` + `version: u32` to `ToxicCombinationRule`.
- `pub const RULE_PACK_VERSION: &str = "session-rules/v1"` in `session/mod.rs`,
  surfaced into `SessionReport`/JSON `schema_version`, asserted by a test so any
  corpus change forces a deliberate bump.

**MUST:**
- **C1.** **Iterate per-fixture results and fail on `!ok`** — explicitly avoid the
  beacon `InstallFiles` bug (§1.4).
- **C2.** Fixtures hold raw `AgentEvent` test input; keep them `#[cfg(test)]`/
  harness-only consts that flow through `normalize` and are **never rendered**. A
  fixture that escapes into rendered output is itself a defect.
- **C3.** Phase the build-failing gate to where engine logic is real: seed it on the
  6 toxic rules whose `evaluate()` is implemented; expand to Stage-A/B signal rules
  as coverage lands. (Avoid a gate over not-yet-real logic blocking the build.)
- **C4.** **Extend the gate to the authored-impact layer** (`impact.rs`, commit
  `0368556`). beacon's maturity ladder maps naturally onto the explanation layer: a
  finding/signal at `stable` MUST have hand-authored `(why, how)` — **not** the
  per-class fallback. Add a test asserting `finding_impact(id).is_some()` for every
  catalog `FindingId` and `signal_impact(name).is_some()` for every `Signal` name,
  so a new id can never silently ship on fallback copy. Nearly free; keeps the
  "no generic text" promise from the commit enforced by CI rather than by vigilance.

**[CORRECTION] Reject shared YAML — for the right reason.** The first pass justified
Rust-native by "avoids a `serde_yaml` dep." **`serde_yaml 0.9` is already a direct
dependency** (`Cargo.toml:21`, used in `probes/store.rs`, `git_write.rs`,
`github.rs`). The real reasons to keep the corpus Rust-native:
(a) the gate binds `Signal`+`join_key` to live `FindingId`s — inexpressible in
CEL-over-event-JSON; and (b) a runtime YAML loader would be a mutable, unversioned
rule path that breaks determinism.

Effort: **M**.

### Seam D — `beacon scan --json` shell-out as a live source — **DROP**

Shelling out to the Go `beacon` binary is **rejected**. Grounded reasons:

1. **Narrower source.** `beacon scan` reads beacon's own `runtime.jsonl` (presumes
   beacon installed as collector). `discovery/` already parses 14 agents' native
   transcripts with zero prerequisites — a strict superset.
2. **Reintroduces raw values.** `Finding.events[]` carry raw command/path/prompt;
   ingesting them bypasses Layer-0 and re-routes raw bytes into the engine — any
   unmapped field of the 60+-field `Event` is a leak path.
3. **Wrong epistemics.** beacon emits intent/pattern detections not joined to local
   reachability — importing them smuggles intent-framing into a reachability product
   (violates "reachability, not intent").
4. **Determinism.** Output depends on beacon's version + mutable
   `~/.beacon/endpoint/rules` + mutable `runtime.jsonl`.
5. **Single-binary / git-only shell-out.** Adds a second external binary (not even
   guaranteed present) and a new untrusted-resolution surface.
6. **[CORRECTION] Supply-chain brand (first-class).** blastradius's identity is
   anti-fetch-and-run (`npm/bin` ships **no** postinstall; SPEC §16; "shell out only
   for git", SPEC §8/§9). A tool that *shames* fetch-on-install and second-binary
   dependencies cannot itself spawn an unverified external Go binary or risk a
   networked `beacon rules pull`.

**If ever revisited (opt-in, additive, offline):** a passive Layer-0 file reader
`src/session/discovery/parse/jsonl_beacon.rs` over `runtime.jsonl`, registered as a
new `AgentSpec{ agent_tag: "beacon" }` in `registry.rs`, routed through the **same**
`normalize`+`classify` JOIN as every other agent. **Never a subprocess, never
auto-refresh.** Reading the file beacon produces buys everything; spawning beacon
buys nothing and costs every constraint.

Effort to drop: **S**.

### Seam E — The moat — **KEEP-BESPOKE**

`classify.rs` (JOIN), `score.rs` (weights/multipliers/containment), `discovery/`,
`retro.rs`/`history.rs`, and the three redaction layers stay bespoke Rust. beacon
has no equivalent for any of them (§3). **Rule-data-drivenness must STOP before the
JOIN:** `candidate_ids` (Signal→FindingId), `finding_is_present`, `compute_score`,
`simulate_containment`, and retro re-resolution remain hardcoded Rust bound to the
live probe baseline and deterministic numeric scoring.

### Seam F — Standardized taxonomy enrichment of the impact layer — **AUGMENT (optional)**

**Goal.** The one place beacon's *rule metadata* genuinely adds to the authored
why/how (`impact.rs`). beacon rules carry
`taxonomy: { owasp_llm: LLM06, mitre_atlas: AML.T0024 }`; `impact.rs` carries no
standardized ids today. Cross-referencing the beacon rule that expresses the same
pattern lets the dashboard show **OWASP-LLM / MITRE-ATLAS badges** next to the
hand-authored `(why, how)`.

**Design.** Add an optional `taxonomy: &'static [(&str, &str)]` to the impact entry
(or a parallel `finding_taxonomy(id)` / `signal_taxonomy(name)` accessor), populated
by hand from the corresponding beacon rule. Surface it on dashboard finding/signal
detail. Value-free by construction — these are public standard labels, not secrets.

**MUST:**
- **F1.** **[CORRECTION] Vendor the taxonomy ids as committed Rust consts** inside
  `src/dashboard/` (with a `beacon rule <id> as of <commit>` comment). **Do not read
  them from the live `agent-beacon` submodule** at build/test time — that would
  couple a Rust release to a Go submodule checkout (see §5, Go/Rust boundary).
- **F2.** Keep blastradius's authored `(why, how)` authoritative; taxonomy is an
  *additional badge*, never a replacement for the reachability-framed prose, and the
  beacon `emit.reason` is **not** imported (it is intent-framed and terser).

Effort: **S** (data entry + a render slot).

---

## 5. Hard constraints → wired controls

Every constraint and the *mechanism* (not aspiration) that upholds it:

| Constraint | Control |
|---|---|
| **Value-free** | Seam A reads only `join_key` (A1), terminal `sweep` (A2), explicit collapse source (A3), extended canary covering the new JSONL path (A4). Conformance fixtures never rendered (C2). Shell-out dropped (D). |
| **Determinism** | Synthesized timestamps are pure `fn(epoch, event_ix)` (A5); ordered walk iterates by `event_ix` (B5); window dropped (B1); pinned `RULE_PACK_VERSION` (C); **beacon output never re-enters classify/score** (one-way export contract — add a test asserting the score has zero dependence on any beacon artifact). |
| **Deps / single static binary (4 targets incl. musl)** | **No CEL in the binary** — reject *both* the `cel-interpreter` crate *and* a from-scratch evaluator; zero CEL needed because the gate is the Rust JOIN. No `cel-go`. Corpus stays compiled-in. |
| **Go/Rust boundary (submodule)** | **[CORRECTION]** `agent-beacon` is a git submodule (Go; `cel-go v0.28.1` + ~24 transitive Go modules). **No build step or test may reference the live submodule path.** Vendor any needed beacon strings (rule ids, `event.action` tag set) as a const list committed *inside* `src/session/` with a "beacon contract as of <commit>; not auto-synced" comment. Add CI: `cargo build --target *-musl` must succeed **with the submodule absent**. |
| **Reachability, not intent** | JOIN stays bound to the live baseline (E); ordered evidence worded "precedes/composes", never "then exfiltrated" (B). |
| **No telemetry / offline** | No network added; `beacon rules pull` never invoked; shell-out dropped. |

---

## 6. Staged plan

| Stage | Seam(s) | Deliverable | New deps | Effort | Risk |
|---|---|---|---|---|---|
| **0** | — | Reconcile SPEC §11/§23/§24 to reflect the built `src/session/`; note `score` command not built, retro is built | none | S | none |
| **1** | C | `conformance.rs` + per-rule `RuleFixture`s + maturity + `RULE_PACK_VERSION` + build-failing `pack_conformance` (per-fixture `!ok`) + **impact-coverage gate (C4)** over `impact.rs` | none | M | low |
| **2** | A | `beacon_view.rs` value-free projection + extended canary + determinism test + emit JSONL (advisory/export only) | none | M | med (value-free) |
| **3** | B | optional `event_order` ordered correlation (plumbing `None`-default; per-rule enablement later with fixtures) | none | M | low |
| **4** | D | **explicitly decline** the shell-out; document the passive-file alternative as the only future shape | none | S | n/a |
| **5** | F | optional taxonomy (OWASP-LLM / MITRE-ATLAS) enrichment of `impact.rs`, vendored consts, dashboard badges | none | S | low |

Recommended order: **0 → 1 → 2 → 3**, with 4 recorded as a deliberate non-goal and
5 as optional polish. The impact-coverage gate (C4) and Stage 5 both build on the
`0368556` why/how layer, so they pair naturally once Stage 1 lands.
Stages are independent enough to land separately; none introduces a dependency.

---

## 7. Open decisions

1. **Does the beacon JSONL export ship at all, or stay test-only?** It is pure
   value-add for ecosystem interop but its only consumer is out-of-process beacon
   tooling. If no consumer is committed, keep `beacon_view.rs` but gate the
   on-disk emission behind an explicit flag (it is an *export*, not part of scan).
2. **Which toxic rules genuinely benefit from ordered gating?** `exfiltration_path`
   and `saas_session_hijack` are candidates, but flipping them changes activation
   sets — defer to a fixture-backed follow-up (B4).
3. **Cross-reference depth.** Map blastradius rule ids ↔ beacon rule ids in doc
   comments only (no code coupling), or maintain a vendored const table? Lean to
   doc comments to avoid any submodule contract surface.

---

## Appendix A — beacon → blastradius concept map

| beacon | blastradius equivalent | relationship |
|---|---|---|
| CEL `match` predicate | `normalize.rs` `is_*` predicates → `Signal` | hardcoded Rust equivalent (keep) |
| `correlation` ordered steps | `toxic_combinations` `event_triggers` + new `event_order` | adopt ordering (B); drop window |
| `correlation.window` | — | inexpressible (no per-event timestamp); drop |
| `finding`-less detection | `classify.rs` JOIN against baseline | **moat — no beacon analog** |
| `severity` enum | `score.rs` numeric score + `RiskLevel` | richer; keep |
| embedded `tests` + `CheckRule` | `conformance.rs` `RuleFixture` + `pack_conformance` | adopt discipline (C) |
| `status` ladder | `ToxicCombinationRule.status` | adopt (C) |
| `spec/threat-rules/VERSION` | `RULE_PACK_VERSION` const | adopt (C) |
| `beacon scan --json` | `discovery/` + `audit-history` | discovery is superset; **drop shell-out** (D) |
| `Event` (raw values) | `AgentEvent` → `BeaconEvent` projection | value-free, lossy, one-way (A) |

## Appendix B — touchpoints (file:item)

- **New:** `src/session/beacon_view.rs` (`BeaconEvent`, `to_beacon_events`),
  `src/session/conformance.rs` (`Verdict`, `RuleFixture`, `check_rule`,
  `check_maturity`, `pack_conformance`).
- **Edit:** `src/session/toxic_combinations.rs` (`ToxicCombinationRule` +`event_order`/`status`/`version`; `event_trigger_matched`→`event_trigger_ixs`;
  `evaluate` ordered walk; `build_catalog` defaults), `src/session/mod.rs`
  (`pub mod conformance; pub const RULE_PACK_VERSION`), `src/session/report.rs`
  (surface version; extend canary loop A4), `src/session/normalize.rs` (consumer of
  `join_key` for A — read-only), `src/dashboard/impact.rs` (C4 impact-coverage gate;
  Seam F taxonomy consts — both keyed by the existing `FindingId`/`Signal` identity).
- **Unchanged (moat):** `src/session/classify.rs`, `src/session/score.rs`,
  `src/session/discovery/*`, `src/session/retro.rs`, `src/session/history.rs`.
- **Docs/CI:** SPEC §11/§23/§24 reconcile (Stage 0); CI musl-build-without-submodule
  assertion.
