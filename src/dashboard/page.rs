//! Cinematic blast-radius dashboard page (served by `blastradius dashboard`).
//!
//! GENERATED FILE — do not edit by hand. This is assembled from the design
//! sources in `.design-bundle/project/` by `.design-bundle/gen_page.py`.
//! Edit those sources (Blast Radius.html, data.js, viz/narrative/dashboard/
//! retro/app.jsx) and re-run the generator instead.
//!
//! The page loads its UI runtime (React 18 + ReactDOM + Babel-standalone)
//! and webfonts from a CDN; those requests carry no scan data. The live,
//! value-free finding inventory is injected at the data marker in the
//! #br-data script tag by `render_html`, and the whole document is run
//! through the Layer-2 redaction sweep before any byte is written to a
//! socket, so secret values never leave the machine.

pub const PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>blastradius — what your coding agent can reach</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css?family=Space+Grotesk:400,500,600,700|JetBrains+Mono:400,500,700&display=swap" rel="stylesheet">
<style>
  :root {
    /* situation-room palette */
    --bg:        #07090d;
    --bg-1:      #0b0e14;
    --bg-2:      #11151e;
    --surface:   #141925;
    --surface-2: #1a2030;
    --line:      rgba(255,255,255,0.08);
    --line-2:    rgba(255,255,255,0.14);

    --txt:       #eef2f8;
    --txt-mid:   #9aa4b6;
    --txt-dim:   #5c6678;

    --hot:       #ff5b35;   /* blast / reach */
    --hot-2:     #ff8a5c;
    --crit:      #ff3d57;   /* critical */
    --warn:      #f5a623;   /* notable */
    --safe:      #2ee6a6;   /* contained */
    --safe-deep: #14b888;
    --info:      #5aa2ff;

    --glow-hot:  0 0 0 1px rgba(255,91,53,.4), 0 0 24px rgba(255,91,53,.35);
    --glow-safe: 0 0 0 1px rgba(46,230,166,.35), 0 0 20px rgba(46,230,166,.25);

    --mono: 'JetBrains Mono', ui-monospace, monospace;
    --sans: 'Space Grotesk', system-ui, sans-serif;

    --ring1: 1100px;
  }

  * { box-sizing: border-box; }
  html, body { margin: 0; padding: 0; background: var(--bg); color: var(--txt); }
  body {
    font-family: var(--sans);
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
    overflow-x: hidden;
  }
  ::selection { background: rgba(255,91,53,.3); }

  /* scrollbar */
  ::-webkit-scrollbar { width: 10px; height: 10px; }
  ::-webkit-scrollbar-track { background: var(--bg); }
  ::-webkit-scrollbar-thumb { background: #20283a; border-radius: 6px; border: 2px solid var(--bg); }
  ::-webkit-scrollbar-thumb:hover { background: #2c3852; }

  .mono { font-family: var(--mono); }
  .sev-exposed { color: var(--hot); }
  .sev-notable { color: var(--warn); }
  .sev-info    { color: var(--info); }

  /* film grain / vignette overlay for cinematic feel */
  .grain {
    position: fixed; inset: 0; pointer-events: none; z-index: 9999;
    background:
      radial-gradient(120% 90% at 50% 10%, transparent 55%, rgba(0,0,0,.55) 100%);
    mix-blend-mode: multiply;
  }

  #root { position: relative; z-index: 1; }

  /* generic button */
  .btn {
    font-family: var(--sans); font-weight: 600; font-size: 14px;
    color: var(--txt); background: var(--surface);
    border: 1px solid var(--line-2); border-radius: 10px;
    padding: 10px 18px; cursor: pointer; transition: all .18s ease;
    letter-spacing: .2px;
  }
  .btn:hover { background: var(--surface-2); border-color: var(--txt-dim); transform: translateY(-1px); }
  .btn-hot { background: linear-gradient(180deg, var(--hot-2), var(--hot)); color: #1a0a04; border: none; box-shadow: var(--glow-hot); }
  .btn-hot:hover { filter: brightness(1.06); }
  .btn-ghost { background: transparent; }

  @keyframes pulseRing {
    0%   { transform: scale(.6); opacity: .7; }
    100% { transform: scale(2.6); opacity: 0; }
  }
  @keyframes breathe {
    0%,100% { opacity: .55; }
    50%     { opacity: 1; }
  }
  @keyframes dashFlow { to { stroke-dashoffset: -1000; } }
  @keyframes fadeUp {
    from { opacity: 0; transform: translateY(18px); }
    to   { opacity: 1; transform: translateY(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    * { animation-duration: .001ms !important; animation-iteration-count: 1 !important; }
  }

  /* ---- RadiusScene chip marquee (vertical auto-scroll, overflow-gated) ----
     Applies to every ring; only the .is-scrolling state (set by JS when the
     chip set overflows the height budget) animates or masks. */
  .chip-marquee { position: relative; max-height: clamp(240px, 52vh, 520px); overflow: hidden; }
  .chip-marquee.is-scrolling {
    -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 16px, #000 calc(100% - 24px), transparent 100%);
            mask-image: linear-gradient(to bottom, transparent 0, #000 16px, #000 calc(100% - 24px), transparent 100%);
  }
  .chip-marquee__track { display: flex; flex-direction: column; gap: 8px; }
  .chip-marquee__set   { display: flex; flex-direction: column; gap: 8px; }
  .chip-marquee.is-scrolling:hover { overflow-y: auto; }
  .chip-marquee.is-scrolling .chip-marquee__track {
    animation: chipCrawl 24s linear infinite;   /* duration overridden inline per-ring */
  }
  .chip-marquee.is-scrolling:hover .chip-marquee__track { animation-play-state: paused; }
  @keyframes chipCrawl {
    from { transform: translateY(0); }
    to   { transform: translateY(calc(-50% - 4px)); }   /* one set + half the 8px track gap */
  }
  @media (prefers-reduced-motion: reduce) {
    .chip-marquee.is-scrolling .chip-marquee__track { animation: none !important; }
    .chip-marquee.is-scrolling { overflow-y: auto; -webkit-mask-image: none; mask-image: none; }
  }
</style>
</head>
<body>
  <div class="grain"></div>
  <div id="root"></div>

  <script src="https://unpkg.com/react@18.3.1/umd/react.development.js" crossorigin="anonymous"></script>
  <script src="https://unpkg.com/react-dom@18.3.1/umd/react-dom.development.js" crossorigin="anonymous"></script>
  <script src="https://unpkg.com/@babel/standalone@7.29.0/babel.min.js" crossorigin="anonymous"></script>

  <script id="br-data" type="application/json">/*__BR_DATA__*/</script>

  <script>
/* blastradius storytelling — data model
 * Realistic illustrative data for a plausible developer machine.
 * No real secrets. Everything is value-free metadata, mirroring the CLI's contract.
 */
(function () {
  "use strict";

  // ---- LIVE DATA from the running scan (injected by build_data) ----------
  // Read the JSON the CLI inlines at #br-data. Default to {} so the page still
  // renders from the canonical fixtures when served standalone / without a scan.
  let D = {};
  try {
    const el = document.getElementById("br-data");
    if (el && el.textContent) D = JSON.parse(el.textContent) || {};
  } catch (e) { D = {}; }

  // ---- Ambient reachable surface (the DENOMINATOR) -----------------------
  // Grouped into concentric "rings" — each ring is one step further from the task.
  const RINGS = [
    {
      id: "shell",
      n: 1,
      label: "This shell",
      blurb: "The environment the agent was handed.",
      findings: [
        { id: "env.secret_names", title: "Secret-named env vars", sev: "exposed",
          metric: "4 reachable", detail: ["GITHUB_TOKEN — 40 chars", "OPENAI_API_KEY — 51 chars", "STRIPE_SECRET_KEY — 31 chars", "DATABASE_URL — present"] },
        { id: "credentials.shell_history", title: "Shell history", sev: "notable",
          metric: "3 secret-looking lines", detail: ["~/.zsh_history — 3 matches", "token-prefix + export patterns"] },
        { id: "cross_repo.dotenv", title: ".env files (this repo)", sev: "exposed",
          metric: "1 file · 12 keys", detail: ["./.env — 12 keys", "values never read"] },
      ],
    },
    {
      id: "identity",
      n: 2,
      label: "Your identity",
      blurb: "The keys and tokens that say you are you.",
      findings: [
        { id: "ssh.private_keys", title: "SSH private keys", sev: "exposed",
          metric: "3 keys readable", detail: ["id_ed25519, id_rsa, work_rsa", "passphrase status not checked"] },
        { id: "ssh.agent_socket", title: "ssh-agent", sev: "notable",
          metric: "2 identities loaded", detail: ["usable without the key files"] },
        { id: "github.token_source", title: "GitHub auth source", sev: "exposed",
          metric: "present for github.com", detail: ["gh hosts.yml — user: octocat", "scopes not verified (local introspection only)"] },
        { id: "git.credential_store", title: "Git credential store", sev: "exposed",
          metric: "2 hosts", detail: ["github.com — stored", "api.heroku.com — .netrc"] },
      ],
    },
    {
      id: "cloud",
      n: 3,
      label: "The cloud",
      blurb: "Provider identities mounted into your shell.",
      findings: [
        { id: "aws.credentials.profiles", title: "AWS profiles", sev: "exposed",
          metric: "2 profiles", detail: ["default", "prod"] },
        { id: "egress.mediation", title: "Cloud metadata", sev: "notable",
          metric: "reachable", detail: ["169.254.169.254 — reachability always checked"] },
      ],
    },
    {
      id: "neighbors",
      n: 4,
      label: "Neighboring repos",
      blurb: "Everything else sitting next to the task on disk.",
      findings: [
        { id: "cross_repo.sibling_repos", title: "Sibling repos", sev: "notable",
          metric: "23 readable", detail: ["~/code/api, ~/code/web, ~/code/infra …", "+20 more"] },
        { id: "cross_repo.lateral_secrets", title: "Secrets in siblings", sev: "exposed",
          metric: "7 repos · 91 keys", detail: [".env, *.pem, service-account.json", "counted, never read"] },
      ],
    },
    {
      id: "network",
      n: 5,
      label: "The network",
      blurb: "Where data could go, and where code could land.",
      findings: [
        { id: "egress.connectivity", title: "Outbound egress", sev: "exposed",
          metric: "open · 19 ms", detail: ["DNS + TLS to 1.1.1.1 ok", "no findings sent"] },
        { id: "git.push_likelihood", title: "Push likelihood", sev: "exposed",
          metric: "likely", detail: ["ssh remote + readable keys", "branch protection is server-side, unverified"] },
      ],
    },
    {
      id: "host",
      n: 6,
      label: "The whole machine",
      blurb: "Beyond the task: the box itself.",
      findings: [
        { id: "host.privilege_escalation", title: "Privilege escalation", sev: "exposed",
          metric: "docker group → root", detail: ["NOPASSWD sudo entries", "container runtime reachable"] },
        { id: "process.memory_introspection", title: "Process memory", sev: "notable",
          metric: "ptrace permitted", detail: ["dump ssh-agent / browser RAM"] },
        { id: "browser.session_stores", title: "Browser sessions", sev: "exposed",
          metric: "cookie jars present", detail: ["session hijack past password + MFA"] },
      ],
    },
  ];

  function counts() {
    // Prefer the live scan's tallies when present.
    if (D.stats && (typeof D.stats.exposed === "number" || typeof D.stats.notable === "number")) {
      const exposed = D.stats.exposed || 0;
      const notable = D.stats.notable || 0;
      const total = (typeof D.stats.total === "number") ? D.stats.total : (exposed + notable);
      return { exposed, notable, total };
    }
    let exposed = 0, notable = 0;
    Object.values(FINDINGS).forEach((f) => { if (f.sev === "exposed") exposed++; else if (f.sev === "notable") notable++; });
    return { exposed, notable, total: Object.keys(FINDINGS).length };
  }

  // ---- LIVE per-ring overlay (the live denominator) ----------------------
  // The canonical RINGS literal is now ONLY a layout / combo-node namespace
  // (ids + titles + severities the constellation needs). Its old metric/detail
  // were fake placeholders ("~/code/api", "23 readable", etc.) — blank them so
  // no placeholder data can ever render. Every real number, path, and why/how
  // comes from the live scan (LIVE_RINGS / FINDINGS / build_data).
  RINGS.forEach((r) => r.findings.forEach((f) => { f.metric = ""; f.detail = []; }));

  // Map each live finding's `severity` value to the `sev` field the viz expects,
  // and its `metric` (summary). Falls back to the (now placeholder-free) canonical
  // RINGS literal when no live rings are present so every node id still resolves.
  const LIVE_RINGS = (D.rings && D.rings.length)
    ? D.rings.map((r) => ({
        id: r.id, n: r.n, label: r.label, blurb: r.blurb,
        findings: (r.findings || []).map((f) => ({
          id: f.id, title: f.title, sev: f.severity,
          metric: f.metric, detail: f.detail || [],
          // carry the live finding-impact teaching copy + containment onto the node
          why: f.why, how: f.how, remediation: f.remediation || [], class: f.class,
        })),
      }))
    : RINGS;

  // Flat index of findings for joins (FindingDetail popover, toxic-combo nodes).
  // PREFER LIVE: iterate LIVE_RINGS first so live metric/detail/sev/why/how/
  // remediation/class win; then fold in canonical RINGS entries only for ids NOT
  // already present, so fixture toxic-combo node ids still resolve when served
  // standalone or partially. Every entry keeps its `ring`.
  const FINDINGS = {};
  LIVE_RINGS.forEach((r) => r.findings.forEach((f) => {
    FINDINGS[f.id] = Object.assign({ ring: r.id }, f);
  }));
  RINGS.forEach((r) => r.findings.forEach((f) => {
    if (!FINDINGS[f.id]) FINDINGS[f.id] = Object.assign({ ring: r.id }, f);
  }));

  // ---- Sessions (the NUMERATOR) ------------------------------------------
  // Each event carries paths/commands/hosts only — never values.
  const SESSIONS = {
    benign: {
      id: "benign",
      label: "Refactor: extract date helpers",
      sub: "A routine, well-behaved task.",
      score: 12,
      level: "low",
      decision: "allow",
      events: [
        { t: "fileRead", signal: null, title: "read", arg: "src/dates.ts", weight: 0 },
        { t: "fileRead", signal: null, title: "read", arg: "src/format.ts", weight: 0 },
        { t: "shell", signal: "shell_command", title: "run", arg: "cargo test", weight: 10, ref: null },
        { t: "fileWrite", signal: null, title: "write", arg: "src/util/dates.ts", weight: 0 },
        { t: "fileWrite", signal: null, title: "write", arg: "src/util/dates.test.ts", weight: 0 },
      ],
      combos: [],
      reasons: [
        { signal: "shell_command", weight: 10, ref: null, evidence: ["1 shell command: cargo test", "no ambient finding joined — stays in the denominator"] },
        { signal: "production_repo", weight: "×1.4", ref: null, evidence: ["repo is flagged production"] },
      ],
    },
    risky: {
      id: "risky",
      label: "Fix: flaky deploy + add telemetry",
      sub: "Looks ordinary. Reaches everywhere.",
      score: 96,
      level: "critical",
      decision: "block",
      events: [
        { t: "fileRead", signal: "read_secret", title: "read", arg: "~/.aws/credentials", weight: 30, ref: "aws.credentials.profiles", hot: true },
        { t: "shell", signal: "read_secret", title: "run", arg: "cat .env  ·  env | grep KEY", weight: 0, ref: "env.secret_names", note: "value-free: reduced to key + length" },
        { t: "fileWrite", signal: "modified_production_deploy", title: "write", arg: ".github/workflows/deploy.yml", weight: 25, ref: "git.push_likelihood", hot: true },
        { t: "network", signal: "network_access", title: "connect", arg: "telemetry-sink.example.io:443", weight: 15, ref: "egress.connectivity", hot: true },
        { t: "shell", signal: "dangerous_shell_pattern", title: "run", arg: "pattern: curl | sh", weight: 25, ref: "host.privilege_escalation", note: "pattern: curl | sh (substring stripped)", hot: true },
        { t: "fileWrite", signal: "edited_auth/payment/security_code", title: "write", arg: "src/auth/session.ts", weight: 20, ref: "git.push_likelihood", hot: true },
      ],
      combos: ["exfiltration_path", "production_deployment_path", "source_control_mutation_path", "high_review_risk"],
      reasons: [
        { signal: "read_secret", weight: 30, ref: "aws.credentials.profiles", evidence: ["file_read ~/.aws/credentials", "joins AWS profiles (2)"] },
        { signal: "modified_production_deploy", weight: 25, ref: "git.push_likelihood", evidence: ["file_write .github/workflows/deploy.yml", "joins push likelihood: likely"] },
        { signal: "dangerous_shell_pattern", weight: 25, ref: "host.privilege_escalation", evidence: ["shell pattern: curl | sh", "category only — substring stripped"] },
        { signal: "edited_auth/payment/security_code", weight: 20, ref: "git.push_likelihood", evidence: ["file_write src/auth/session.ts"] },
        { signal: "network_access", weight: 15, ref: "egress.connectivity", evidence: ["network_access telemetry-sink.example.io:443", "joins egress: open"] },
        { signal: "exfiltration_path", weight: "+40 path", ref: "egress.connectivity", evidence: ["critical toxic combination"] },
        { signal: "production_deployment_path", weight: "+40 path", ref: "git.push_likelihood", evidence: ["critical toxic combination"] },
        { signal: "production_repo", weight: "×1.4", ref: null, evidence: ["repo is flagged production"] },
        { signal: "escalation_amplifier", weight: "×1.5", ref: "host.privilege_escalation", evidence: ["escalation reachable AND a shell command ran"] },
      ],
    },
  };

  // ---- Toxic-combination catalog (event(s) × ambient finding(s) → PATH) ---
  const COMBOS = {
    exfiltration_path: {
      name: "exfiltration_path",
      title: "Credential exfiltration path",
      sev: "critical",
      derived: "A secret was read and an outbound route is open. Anything read here can leave the machine.",
      legs: ["read_secret", "egress.connectivity"],
      nodes: ["aws.credentials.profiles", "egress.connectivity"],
      evidence: ["read ~/.aws/credentials", "+ outbound egress open", "→ reachable secret can leave the host"],
    },
    production_deployment_path: {
      name: "production_deployment_path",
      title: "Production deployment path",
      sev: "critical",
      derived: "A deploy workflow was edited and a push is likely to be accepted. A change could ship to production.",
      legs: ["modified_production_deploy", "git.push_likelihood"],
      nodes: ["git.push_likelihood"],
      evidence: ["wrote .github/workflows/deploy.yml", "+ push likelihood: likely", "→ change composes into a deploy"],
    },
    source_control_mutation_path: {
      name: "source_control_mutation_path",
      title: "Source-control mutation path",
      sev: "high",
      derived: "A tracked file was written and an ssh-agent identity can authenticate the push.",
      legs: ["ssh.agent_socket", "git.push_likelihood"],
      nodes: ["ssh.agent_socket", "git.push_likelihood"],
      evidence: ["file-write to tracked src/auth/session.ts (event trigger)", "+ ssh-agent identity loaded · push likelihood: likely", "→ commit + push composes"],
    },
    high_review_risk: {
      name: "high_review_risk",
      title: "Unreviewed sensitive-code change",
      sev: "high",
      derived: "Auth/payment/security code was changed with no covering approval. A review-control gap, not ambient reach.",
      legs: ["edited_auth/payment/security_code"],
      nodes: ["git.push_likelihood"],
      evidence: ["wrote src/auth/session.ts", "no covering approval event", "→ ships without human review"],
    },
    saas_session_hijack: {
      name: "saas_session_hijack",
      title: "SaaS session hijack",
      sev: "high",
      derived: "A browser session/cookie store was read and an outbound route is open — a live session can be replayed past password + MFA.",
      legs: ["browser.session_stores", "egress.connectivity"],
      nodes: ["browser.session_stores", "egress.connectivity"],
      evidence: ["read a browser session store", "+ outbound egress open", "→ session token can leave the host"],
    },
    post_root_host_visibility: {
      name: "post_root_host_visibility",
      title: "Post-root host visibility",
      sev: "critical",
      derived: "An escalation path plus cross-repo reach means a foothold here composes into broader host + neighbor visibility.",
      legs: ["host.privilege_escalation", "cross_repo.sibling_repos"],
      nodes: ["host.privilege_escalation", "cross_repo.sibling_repos"],
      evidence: ["escalation path reachable", "+ cross-repo reach present", "→ foothold composes into the host"],
    },
  };

  // ---- Containment simulator ---------------------------------------------
  // Stacked ladder (fixed order) and independent single-control deltas.
  const CONTROLS = [
    { id: "repo_only_filesystem", label: "Repo-only filesystem", cat: "Filesystem isolation",
      desc: "Mount only the task repo + explicit deps. No broad $HOME or sibling-repo access.",
      indep: 35, suppresses: ["cross_repo.sibling_repos", "cross_repo.lateral_secrets", "cross_repo.dotenv", "browser.session_stores", "credentials.shell_history"],
      kills: [] },
    { id: "no_egress", label: "No egress", cat: "Egress control",
      desc: "Default-deny outbound, then allowlist what the task needs.",
      indep: 13, suppresses: ["egress.connectivity", "egress.mediation"],
      kills: ["exfiltration_path"] },
    { id: "no_ssh_agent", label: "No ssh-agent", cat: "Credential substitution",
      desc: "Don't forward the ssh-agent socket into the agent's environment.",
      indep: 6, suppresses: ["ssh.agent_socket"],
      kills: ["source_control_mutation_path"] },
    { id: "scoped_temp_cloud_creds", label: "Scoped temp creds", cat: "Credential substitution",
      desc: "Short-lived, narrowly-scoped creds per agent instead of your full identity.",
      indep: 14, suppresses: ["aws.credentials.profiles", "github.token_source", "git.credential_store", "env.secret_names"],
      kills: ["exfiltration_path"] },
    { id: "process_isolation", label: "Process isolation", cat: "Process isolation",
      desc: "Prevent same-user process inspection and access to other local dev tools.",
      indep: 17, suppresses: ["process.memory_introspection", "host.privilege_escalation"],
      kills: [] },
  ];

  // Headline stacked ladder for the risky session (spec figures).
  const LADDER = [
    { label: "baseline (no controls)", control: null, score: 96, delta: 0 },
    { label: "+ repo-only filesystem", control: "repo_only_filesystem", score: 61, delta: -35 },
    { label: "+ no egress", control: "no_egress", score: 48, delta: -13 },
    { label: "+ no ssh-agent", control: "no_ssh_agent", score: 42, delta: -6 },
    { label: "+ scoped temp creds", control: "scoped_temp_cloud_creds", score: 28, delta: -14 },
    { label: "+ process isolation", control: "process_isolation", score: 11, delta: -17 },
  ];
  const RESIDUAL = {
    floor: 11,
    reason: "in-repo auth-code edit, unreviewed — needs human review / server-side enforcement.",
  };

  function levelOf(score) {
    if (score >= 75) return "critical";
    if (score >= 50) return "high";
    if (score >= 25) return "medium";
    return "low";
  }

  // Recompute a session score given a set of active control ids.
  // Illustrative model: start from baseline, subtract the stacked deltas for the
  // controls that are on, in the fixed ladder order, but never below the floor.
  function simulate(activeIds) {
    const order = LADDER.slice(1); // skip baseline
    let score = 96;
    const applied = [];
    order.forEach((step) => {
      if (activeIds.has(step.control)) {
        score += step.delta;
        applied.push(step.control);
      }
    });
    score = Math.max(RESIDUAL.floor, Math.min(96, score));
    if (activeIds.size === 0) score = 96;
    // which combos survive
    const killed = new Set();
    CONTROLS.forEach((c) => { if (activeIds.has(c.id)) c.kills.forEach((k) => killed.add(k)); });
    return { score, level: levelOf(score), killed };
  }

  // ---- §24 AUTO-SLURP + RETRO-HAZARD -------------------------------------
  // Passively discovered agent transcripts on disk (§24.1) and the historical
  // hazards re-resolved against TODAY's reachable surface (§24.3).
  const AGENTS_DISCOVERED_FIXTURE = [
    { tag: "claude-code", name: "Claude Code", sessions: 14 },
    { tag: "codex",       name: "Codex CLI",   sessions: 6 },
    { tag: "cursor",      name: "Cursor",      sessions: 9 },
    { tag: "copilot",     name: "Copilot CLI", sessions: 3 },
    { tag: "opencode",    name: "opencode",    sessions: 2 },
    { tag: "antigravity", name: "Antigravity", sessions: 1 },
    { tag: "factory",     name: "Factory Droid", sessions: 1 },
    { tag: "aider",       name: "Aider",       sessions: 4 },
  ];

  // Remediations the operator can apply now (each removes an ambient finding leg).
  const REMEDIATIONS = [
    { id: "aws.credentials.profiles", label: "Rotate cloud credentials" },
    { id: "egress.connectivity",      label: "Close outbound egress" },
    { id: "browser.session_stores",   label: "Clear browser sessions" },
    { id: "git.push_likelihood",      label: "Scope the push token" },
  ];

  // Historical hazards. `required` = finding ids that must still fire for the
  // hazard to be live. `deadAtStart` = legs already remediated since the session.
  const HAZARDS_FIXTURE = [
    { hid: "3f2a…e1", agent: "claude-code", source: "~/.claude/projects/app/3f2a…e1.jsonl",
      age: "3 days ago", ageDays: 3, combo: "exfiltration_path", sev: "critical",
      required: ["aws.credentials.profiles", "egress.connectivity"], base: 40,
      ordering: "read → egress",
      events: ["file_read  ~/.aws/credentials", "shell_command  curl [custom egress target]"],
      summary: "Read an AWS credential store, then ran an external-egress command." },
    { hid: "b71c…90", agent: "codex", source: "~/.codex/sessions/2026/06/07/rollout-…jsonl",
      age: "6 days ago", ageDays: 6, combo: "production_deployment_path", sev: "critical",
      required: ["git.push_likelihood"], base: 40,
      events: ["file_write  .github/workflows/deploy.yml"],
      summary: "Edited a deploy workflow; a push to the remote is still likely to be accepted." },
    { hid: "cc4f…12", agent: "cursor", source: "~/.cursor/projects/web/agent-transcripts/…jsonl",
      age: "9 days ago", ageDays: 9, combo: "saas_session_hijack", sev: "high",
      required: ["browser.session_stores", "egress.connectivity"], base: 25,
      events: ["file_read  Cookies (browser store)", "network_access  [custom egress target]"],
      summary: "Read a browser session store, then opened an outbound connection." },
    { hid: "a0d7…e3", agent: "copilot", source: "~/.copilot/session-state/…/events.jsonl",
      age: "12 days ago", ageDays: 12, combo: "exfiltration_path", sev: "critical",
      required: ["env.secret_names", "egress.connectivity"], base: 40,
      deadAtStart: ["env.secret_names"],
      events: ["shell_command  env | grep TOKEN", "network_access  [custom egress target]"],
      summary: "Read secret-named env vars, then egressed — but that env var is gone now." },
  ];

  function retroDecay(d) { return Math.max(Math.pow(0.5, d / 14), 0.25); }

  // Review-control-gap lane (§23.8 high_review_risk): NO reachability claim.
  // review_score (§24.3 #9b) = clamp(round(combo_base * decay(ageDays) * 1.5), 0, 60),
  // combo_base = 25 (high). This is a review-control signal, not a reachability claim.
  function reviewScore(ageDays) {
    return Math.max(0, Math.min(60, Math.round(25 * retroDecay(ageDays) * 1.5)));
  }
  const REVIEW_GAPS_FIXTURE = [
    { hid: "77aa…10", agent: "claude-code", age: "5 days ago", ageDays: 5,
      combo: "high_review_risk", sev: "high",
      review_score: reviewScore(5),
      events: ["file_write  src/auth/session.ts", "— no covering approval event"],
      summary: "Auth code changed with no covering approval. A review-control gap — not a reachability claim." },
  ];

  // 4-TIER reach ladder (§24.3 #9a):
  //   1.00 — all legs live AND every live leg is Exposed (fully realized reach)
  //   0.70 — all legs live but not all are Exposed (mixed-severity reach)
  //   0.45 — only some legs live (partial)
  //   0.10 — no legs live (remediated)
  // Severity is read from the live finding when present, else the fixture FINDING.
  // Severity of a leg: prefer the hazard's own per-leg severity (real report),
  // else the live/fixture FINDINGS table.
  function reachTier(legs, hz) {
    const liveLegs = legs.filter((l) => l.live);
    if (liveLegs.length === 0) return 0.10;
    const allLive = liveLegs.length === legs.length;
    if (!allLive) return 0.45;
    const sevOf = (fid) => {
      if (hz && hz.legSev && hz.legSev[fid]) return hz.legSev[fid];
      const lf = FINDINGS[fid];
      return lf ? lf.sev : null;
    };
    const allExposed = liveLegs.every((l) => sevOf(l.ref) === "exposed");
    return allExposed ? 1.00 : 0.70;
  }

  // Re-resolve one hazard against today's findings minus the `dead` set.
  // For a real CLI hazard with NO user remediation toggled, reflect the report's
  // authoritative status/score verbatim (§24.6: the dashboard renders, never
  // re-scores). Toggling a remediation switches to the interactive what-if model.
  function retroResolve(hz, dead) {
    const deadAll = new Set([...(hz.deadAtStart || []), ...dead]);
    const legs = hz.required.map((fid) => ({ ref: fid, live: !deadAll.has(fid) }));
    const liveCount = legs.filter((l) => l.live).length;
    const allLive = liveCount === hz.required.length;

    const noUserToggle = !dead || dead.size === 0;
    if (noUserToggle && hz.realizedFromReport != null && hz.statusFromReport) {
      const map = { still_reachable: "still_reachable", partially_remediated: "partial",
        remediated_since: "remediated", review_gap: "remediated" };
      const status = map[hz.statusFromReport] || (allLive ? "still_reachable" : (liveCount > 0 ? "partial" : "remediated"));
      return { legs, status, realized: hz.realizedFromReport, live: status === "still_reachable" };
    }

    const status = allLive ? "still_reachable" : (liveCount > 0 ? "partial" : "remediated");
    const reach = reachTier(legs, hz);
    const durability = 1 + 0.15 * (liveCount / hz.required.length);
    const realized = Math.max(0, Math.min(100, Math.round(hz.base * reach * durability * retroDecay(hz.ageDays) * 2.5)));
    return { legs, status, realized, live: allLive };
  }

  // ---- Real retro history from the CLI (D.history = HistoryAuditReport) ----
  // When `dashboard --history` is served, D.history is the value-free
  // HistoryAuditReport. We map it into the same fixture SHAPE the ledger renders
  // so retroResolve()/HazardCard/ReviewGapCard work unchanged. Everything here is
  // value-free already (ids, severities, counts, shortened labels, RFC3339).
  const AGENT_NAMES = {
    "claude-code": "Claude Code", "codex": "Codex CLI", "cursor": "Cursor",
    "cursor-ide": "Cursor IDE", "copilot": "Copilot CLI", "opencode": "opencode",
    "gemini": "Gemini CLI", "antigravity": "Antigravity", "factory": "Factory Droid",
    "devin": "Devin", "windsurf": "Windsurf", "aider": "Aider", "hermes": "Hermes", "amp": "Amp",
  };
  const agentName = (tag) => AGENT_NAMES[tag] || tag;

  // Human "N days ago" from an age in days (value-free count).
  function ageLabel(days) {
    const d = Math.max(0, Math.round(days));
    if (d === 0) return "today";
    if (d === 1) return "1 day ago";
    return d + " days ago";
  }

  // base path-weight from the toxic-combo severity (mirrors §24.3.4 combo_base).
  const COMBO_BASE = { critical: 40, high: 25, medium: 15, low: 0 };

  // Map one HistoricalHazard JSON -> the fixture HAZARD shape the ledger renders.
  function mapHazard(h) {
    const sess = h.session || {};
    const legs = (h.reachability && h.reachability.legs) || [];
    const required = legs.filter((l) => l.required).map((l) => l.finding_ref);
    // legs already remediated NOW (required but not currently present) seed deadAtStart.
    const deadAtStart = legs
      .filter((l) => l.required && !l.current)
      .map((l) => l.finding_ref);
    // per-leg current severity straight from the report (value-free).
    const legSev = {};
    legs.forEach((l) => { if (l.current && l.current.severity) legSev[l.finding_ref] = l.current.severity; });
    // value-free observed-event lines: reduce the combo evidence (shape-only).
    const events = ((h.combination && h.combination.evidence) || []).slice(0, 4);
    const orderingLabel = {
      secret_read_precedes_egress: "read → egress",
      egress_precedes_secret_read: "egress → read",
      unordered: null,
    }[h.ordering];
    return {
      hid: h.hazard_id,
      agent: sess.agent || "unknown",
      source: sess.source_label || "",
      age: ageLabel((h.recency && h.recency.age_days) || 0),
      ageDays: (h.recency && h.recency.age_days) || 0,
      combo: (h.combination && h.combination.name) || "",
      sev: (h.combination && h.combination.severity) || "high",
      required,
      base: COMBO_BASE[(h.combination && h.combination.severity) || "high"] || 0,
      ordering: orderingLabel || undefined,
      deadAtStart,
      legSev,
      events,
      summary: h.summary || "",
      // the CLI already ranked & resolved this; carry its authoritative values so
      // the page reflects (never re-scores) the report on first paint (§24.6).
      realizedFromReport: h.realized_score,
      statusFromReport: h.status,
    };
  }

  function mapReviewGap(h) {
    const sess = h.session || {};
    return {
      hid: h.hazard_id,
      agent: sess.agent || "unknown",
      age: ageLabel((h.recency && h.recency.age_days) || 0),
      ageDays: (h.recency && h.recency.age_days) || 0,
      combo: (h.combination && h.combination.name) || "high_review_risk",
      sev: (h.combination && h.combination.severity) || "high",
      review_score: h.realized_score,
      events: ((h.combination && h.combination.evidence) || []).slice(0, 4),
      summary: h.summary || "",
    };
  }

  // Roll the discovered hazards' sessions up into a per-agent discovery roster.
  function rosterFromHistory(H) {
    const all = [...(H.hazards || []), ...(H.review_gaps || [])];
    const byAgent = {};
    const seen = {};
    all.forEach((h) => {
      const tag = (h.session && h.session.agent) || "unknown";
      const sid = (h.session && h.session.session_id) || h.hazard_id;
      const key = tag + ":" + sid;
      if (seen[key]) return;
      seen[key] = true;
      byAgent[tag] = (byAgent[tag] || 0) + 1;
    });
    return Object.keys(byAgent).sort().map((tag) => ({
      tag, name: agentName(tag), sessions: byAgent[tag],
    }));
  }

  const HAS_HISTORY = !!(D.history && (
    (D.history.hazards && D.history.hazards.length) ||
    (D.history.review_gaps && D.history.review_gaps.length) ||
    (D.history.discovery_diagnostics && D.history.discovery_diagnostics.length)
  ));

  const HAZARDS = HAS_HISTORY ? (D.history.hazards || []).map(mapHazard) : HAZARDS_FIXTURE;
  const REVIEW_GAPS = HAS_HISTORY ? (D.history.review_gaps || []).map(mapReviewGap) : REVIEW_GAPS_FIXTURE;
  const AGENTS_DISCOVERED = HAS_HISTORY ? rosterFromHistory(D.history) : AGENTS_DISCOVERED_FIXTURE;

  // §23 real ranked sessions (top-N) from the engine; null → demo fallback.
  const SESSIONS_RANKED = (D.sessions && D.sessions.ranked && D.sessions.ranked.length)
    ? D.sessions.ranked : null;

  window.BR = {
    RINGS, LIVE_RINGS, FINDINGS, SESSIONS, COMBOS, CONTROLS, LADDER, RESIDUAL,
    AGENTS_DISCOVERED, REMEDIATIONS, HAZARDS, REVIEW_GAPS, retroResolve,
    HAS_HISTORY, SESSIONS_RANKED,
    counts, levelOf, simulate,
    BREADTH: {
      probes: (D.stats && D.stats.breadth && D.stats.breadth.probes) || 35,
      stores: (D.stats && D.stats.breadth && D.stats.breadth.stores) || 30,
    },
    SEV_LABEL: { exposed: "Exposed", notable: "Notable", info: "Info" },
  };
})();

  </script>

  <script type="text/babel">
/* viz.jsx — the two hero visualizations:
 *   RadiusViz      : concentric expanding rings (scrollytelling denominator)
 *   Constellation  : node graph that lights up as a session plays (dashboard)
 */
const { useMemo, useRef, useEffect, useState } = React;

const SEV_COLOR = { exposed: "var(--hot)", notable: "var(--warn)", info: "var(--info)" };

/* ---- shared layout: place every finding at a deterministic angle/ring ---- */
function useLayout() {
  return useMemo(() => {
    const RINGS = window.BR.RINGS;
    const ringR = { shell: 0.20, identity: 0.36, cloud: 0.50, neighbors: 0.66, network: 0.82, host: 0.97 };
    const startAngle = { shell: -90, identity: -50, cloud: 30, neighbors: -110, network: 20, host: -70 };
    const pos = {};
    RINGS.forEach((ring) => {
      const r = ringR[ring.id];
      const n = ring.findings.length;
      const spread = Math.min(300, 70 * n);
      const base = startAngle[ring.id];
      ring.findings.forEach((f, i) => {
        // n<2: avoid i/(n-1) (division by zero -> NaN); pin to the base angle.
        const a = (base + (n < 2 ? 0 : (i / (n - 1) - 0.5) * spread)) * Math.PI / 180;
        pos[f.id] = { r, a, x: 0.5 + r * 0.5 * Math.cos(a), y: 0.5 + r * 0.5 * Math.sin(a), ring: ring.id, sev: f.sev };
      });
    });
    return { ringR, pos, RINGS };
  }, []);
}

/* ============================ RADIUS VIZ ============================ */
/* revealed: how many rings are shown (0..6); float allowed for smooth scroll */
function RadiusViz({ revealed }) {
  const { ringR, pos, RINGS } = useLayout();
  const S = 1000, C = S / 2;
  const ringRadii = { shell: 0.20, identity: 0.36, cloud: 0.50, neighbors: 0.66, network: 0.82, host: 0.97 };

  return (
    <svg viewBox={`0 0 ${S} ${S}`} style={{ width: "100%", height: "100%", display: "block" }}>
      <defs>
        <radialGradient id="coreGlow" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="var(--hot-2)" stopOpacity="0.9" />
          <stop offset="40%" stopColor="var(--hot)" stopOpacity="0.5" />
          <stop offset="100%" stopColor="var(--hot)" stopOpacity="0" />
        </radialGradient>
        <filter id="soft"><feGaussianBlur stdDeviation="3" /></filter>
      </defs>

      {/* faint full-field glow scales with reveal */}
      <circle cx={C} cy={C} r={C * Math.min(1, revealed / 6) * 0.98}
        fill="url(#coreGlow)" opacity={0.18 + 0.12 * Math.min(1, revealed / 6)} />

      {RINGS.map((ring, idx) => {
        const ord = idx + 1;
        const show = revealed >= idx + 0.15;
        const local = Math.max(0, Math.min(1, revealed - idx));
        const rr = ringRadii[ring.id] * C;
        return (
          <g key={ring.id} style={{ opacity: show ? 1 : 0, transition: "opacity .5s ease" }}>
            <circle cx={C} cy={C} r={rr} fill="none"
              stroke={SEV_COLOR.exposed} strokeOpacity={0.10 + 0.10 * local}
              strokeWidth={1.2} strokeDasharray="2 7" />
            {ring.findings.map((f) => {
              const p = pos[f.id];
              const x = C + (p.x - 0.5) * S, y = C + (p.y - 0.5) * S;
              const col = SEV_COLOR[f.sev];
              return (
                <g key={f.id} transform={`translate(${x},${y})`}
                   style={{ opacity: local > 0.2 ? 1 : 0, transition: "opacity .5s ease" }}>
                  <line x1={(C - x)} y1={(C - y)} x2={0} y2={0}
                    stroke={col} strokeOpacity={0.18} strokeWidth={1} />
                  <circle r={9} fill={col} opacity={0.18} filter="url(#soft)" />
                  <circle r={4.5} fill={col} />
                </g>
              );
            })}
          </g>
        );
      })}

      {/* core: YOU */}
      <circle cx={C} cy={C} r={26} fill="url(#coreGlow)" />
      <circle cx={C} cy={C} r={11} fill="var(--hot-2)" />
      <circle cx={C} cy={C} r={11} fill="none" stroke="#1a0a04" strokeWidth={2} />
    </svg>
  );
}

/* ============================ CONSTELLATION ============================ */
/* props:
 *   activeFindings : Set of finding ids currently "touched"
 *   combos         : array of combo objects to draw paths for
 *   suppressed     : Set of finding ids greyed out (containment)
 *   onPick(id)     : click a node
 *   dim            : overall dim factor for ambient (untouched) nodes
 */
function Constellation({ activeFindings, combos, suppressed, onPick, picked }) {
  const { pos, RINGS } = useLayout();
  const S = 1000, C = S / 2;
  activeFindings = activeFindings || new Set();
  suppressed = suppressed || new Set();
  combos = combos || [];

  // Guarded position lookup: a live scan may omit a fixture-referenced id, so
  // return null instead of dereferencing undefined (would white-screen React).
  const P = (id) => { const p = pos[id]; return p ? { x: C + (p.x - 0.5) * S, y: C + (p.y - 0.5) * S } : null; };

  return (
    <svg viewBox={`0 0 ${S} ${S}`} style={{ width: "100%", height: "100%", display: "block" }}>
      <defs>
        <radialGradient id="cCore" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="var(--hot-2)" stopOpacity="1" />
          <stop offset="60%" stopColor="var(--hot)" stopOpacity="0.4" />
          <stop offset="100%" stopColor="var(--hot)" stopOpacity="0" />
        </radialGradient>
        <filter id="cglow" x="-80%" y="-80%" width="260%" height="260%">
          <feGaussianBlur stdDeviation="6" result="b" />
          <feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>

      {/* faint ring guides */}
      {[0.20,0.36,0.50,0.66,0.82,0.97].map((r,i)=>(
        <circle key={i} cx={C} cy={C} r={r*C} fill="none" stroke="rgba(255,255,255,0.05)" strokeWidth="1" />
      ))}

      {/* spokes to active nodes */}
      {RINGS.map((ring) => ring.findings.map((f) => {
        const isActive = activeFindings.has(f.id);
        const isSupp = suppressed.has(f.id);
        if (!isActive || isSupp) return null;
        const p = P(f.id);
        if (!p) return null;
        return <line key={"sp"+f.id} x1={C} y1={C} x2={p.x} y2={p.y}
          stroke="var(--hot)" strokeOpacity={0.5} strokeWidth={1.6} />;
      }))}

      {/* toxic combination paths */}
      {combos.map((combo) => {
        const allLive = combo.nodes.every((n) => !suppressed.has(n));
        if (!allLive) return null;
        const col = combo.sev === "critical" ? "var(--crit)" : "var(--warn)";
        // A live scan may not surface every fixture node; drop any that don't
        // resolve to a position so a missing id can't white-screen the tree.
        const pts = combo.nodes.map(P).filter(Boolean);
        if (pts.length < 2) return null;
        return (
          <g key={"cmb"+combo.name}>
            {pts.slice(0, -1).map((p, i) => {
              const q = pts[i + 1];
              return <line key={i} x1={p.x} y1={p.y} x2={q.x} y2={q.y}
                stroke={col} strokeWidth={2.6} strokeLinecap="round"
                strokeDasharray="6 8" style={{ animation: "dashFlow 14s linear infinite" }}
                opacity={0.95} filter="url(#cglow)" />;
            })}
          </g>
        );
      })}

      {/* nodes */}
      {RINGS.map((ring) => ring.findings.map((f) => {
        const p = P(f.id);
        if (!p) return null;
        const isActive = activeFindings.has(f.id);
        const isSupp = suppressed.has(f.id);
        const isPicked = picked === f.id;
        const col = SEV_COLOR[f.sev];
        const baseOp = isSupp ? 0.14 : (isActive ? 1 : 0.42);
        return (
          <g key={f.id} transform={`translate(${p.x},${p.y})`}
             onClick={() => onPick && onPick(f.id)}
             style={{ cursor: "pointer", opacity: baseOp, transition: "opacity .4s ease" }}>
            {isActive && !isSupp && (
              <circle r={18} fill={col} opacity={0.22}>
                <animate attributeName="r" values="14;26;14" dur="2.4s" repeatCount="indefinite" />
                <animate attributeName="opacity" values="0.28;0.05;0.28" dur="2.4s" repeatCount="indefinite" />
              </circle>
            )}
            <circle r={isActive ? 11 : 7} fill={isSupp ? "#3a4252" : col}
              filter={isActive && !isSupp ? "url(#cglow)" : "none"} />
            {isPicked && <circle r={16} fill="none" stroke="#fff" strokeWidth="2" />}
            <circle r={isActive ? 4 : 3} fill={isSupp ? "#5a6478" : "#0b0e14"} opacity={isActive?0.0:0.0} />
          </g>
        );
      }))}

      {/* core: the agent */}
      <circle cx={C} cy={C} r={34} fill="url(#cCore)" />
      <circle cx={C} cy={C} r={13} fill="var(--hot-2)" />
      <circle cx={C} cy={C} r={13} fill="none" stroke="#1a0a04" strokeWidth={2.5} />
    </svg>
  );
}

Object.assign(window, { RadiusViz, Constellation, SEV_COLOR, useLayout });

  </script>
  <script type="text/babel">
/* narrative.jsx — the cinematic scrollytelling intro (Acts 1-3) */
const { useState: useStateN } = React;

/* progress (0..1) of an element scrolling through the viewport */
function useScrollProgress(ref) {
  const [p, setP] = React.useState(0);
  React.useEffect(() => {
    let raf = null;
    const onScroll = () => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = null;
        const el = ref.current; if (!el) return;
        const rect = el.getBoundingClientRect();
        const vh = window.innerHeight;
        // 0 when top hits top of viewport, 1 when bottom reaches bottom
        const total = rect.height - vh;
        const scrolled = -rect.top;
        setP(Math.max(0, Math.min(1, scrolled / Math.max(1, total))));
      });
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    return () => { window.removeEventListener("scroll", onScroll); window.removeEventListener("resize", onScroll); };
  }, [ref]);
  return p;
}

/* fade-in when scrolled into view */
function Reveal({ children, className, style, delay = 0 }) {
  const ref = React.useRef(null);
  const [seen, setSeen] = React.useState(false);
  React.useEffect(() => {
    const io = new IntersectionObserver(([e]) => { if (e.isIntersecting) setSeen(true); }, { threshold: 0.25 });
    if (ref.current) io.observe(ref.current);
    return () => io.disconnect();
  }, []);
  return (
    <div ref={ref} className={className}
      style={{ ...style, opacity: seen ? 1 : 0, transform: seen ? "translateY(0)" : "translateY(24px)",
        transition: `opacity .8s ease ${delay}s, transform .8s ease ${delay}s` }}>
      {children}
    </div>
  );
}

/* Vertical auto-scroll marquee for a ring's finding chips. Applies to EVERY
   ring: animates ONLY when the chip set overflows the height budget (measured,
   not a fixed count), so short/empty rings render one static set with no motion
   and any ring with too many chips gently crawls instead of running off-screen.
   Pure CSS animation, so the global prefers-reduced-motion rule disables it. */
function ChipMarquee({ findings }) {
  const boxRef = React.useRef(null);
  const setRef = React.useRef(null);
  const [over, setOver] = React.useState(false);
  const [dur, setDur] = React.useState(24);
  React.useLayoutEffect(() => {
    const box = boxRef.current, set = setRef.current;
    if (!box || !set) return;
    const measure = () => {
      const sh = set.scrollHeight;
      const ov = sh > box.clientHeight + 2;
      setOver(ov);
      if (ov) setDur(Math.min(90, Math.max(24, sh / 12))); // ~12px/sec (readable), clamped
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(box);
    return () => ro.disconnect();
  }, [findings]);
  // Each finding is a readable card: title · metric, then the authored WHY and
  // WHAT-AN-AGENT-CAN-DO text (never the old generic "title · metric" chip).
  const Card = (f) => {
    const col = SEV_COLOR[f.sev];
    return (
      <div key={f.id} style={{ borderLeft: `2px solid ${col}`, borderRadius: 7,
        background: "rgba(255,255,255,0.02)", padding: "10px 13px", maxWidth: 460 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontSize: 13.5, fontWeight: 600, color: col }}>{f.title}</span>
          <span className="mono" style={{ fontSize: 11.5, color: "var(--txt-dim)" }}>{f.metric}</span>
        </div>
        {f.why && <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5, marginTop: 6 }}>{f.why}</div>}
        {f.how && (
          <div style={{ fontSize: 12.5, color: "var(--txt)", lineHeight: 1.5, marginTop: 6 }}>
            <span className="mono" style={{ color: col, fontSize: 10, letterSpacing: 1, marginRight: 6 }}>AGENT CAN</span>
            {f.how}
          </div>
        )}
      </div>
    );
  };
  return (
    <div ref={boxRef} className={"chip-marquee" + (over ? " is-scrolling" : "")}
      style={{ marginTop: 22, pointerEvents: "auto" }}>
      <div className="chip-marquee__track" style={over ? { animationDuration: dur + "s" } : undefined}>
        <div className="chip-marquee__set" ref={setRef}>{findings.map(Card)}</div>
        {over && <div className="chip-marquee__set" aria-hidden="true">{findings.map(Card)}</div>}
      </div>
    </div>
  );
}

const sceneWrap = { minHeight: "100vh", display: "flex", flexDirection: "column", alignItems: "center",
  justifyContent: "center", padding: "0 24px", position: "relative" };

/* ---------- Act 0: hook ---------- */
function Hook() {
  return (
    <section style={{ ...sceneWrap, minHeight: "100vh" }}>
      <div style={{ position: "relative", width: 120, height: 120, marginBottom: 48 }}>
        <div style={{ position: "absolute", inset: 0, borderRadius: "50%", border: "1px solid var(--hot)",
          animation: "pulseRing 3s ease-out infinite" }} />
        <div style={{ position: "absolute", inset: 0, borderRadius: "50%", border: "1px solid var(--hot)",
          animation: "pulseRing 3s ease-out infinite 1.5s" }} />
        <div style={{ position: "absolute", inset: "44px", borderRadius: "50%", background: "var(--hot-2)",
          boxShadow: "var(--glow-hot)" }} />
      </div>
      <div className="mono" style={{ color: "var(--txt-dim)", letterSpacing: 4, fontSize: 12, textTransform: "uppercase", marginBottom: 24 }}>
        blastradius
      </div>
      <h1 style={{ fontSize: "clamp(40px, 7vw, 92px)", fontWeight: 600, lineHeight: 1.02, margin: 0,
        textAlign: "center", letterSpacing: "-0.02em", maxWidth: 1000 }}>
        You gave it<br />one&nbsp;small&nbsp;task.
      </h1>
      <p style={{ color: "var(--txt-mid)", fontSize: "clamp(16px,2vw,21px)", maxWidth: 560, textAlign: "center",
        marginTop: 28, lineHeight: 1.5 }}>
        A coding agent. A clean checkout. A few files to touch.
        It feels small, and contained. Let's see how far it can actually reach.
      </p>
      <div style={{ position: "absolute", bottom: 36, display: "flex", flexDirection: "column", alignItems: "center", gap: 8,
        color: "var(--txt-dim)", animation: "breathe 2.4s ease-in-out infinite" }}>
        <span className="mono" style={{ fontSize: 11, letterSpacing: 2 }}>SCROLL</span>
        <span style={{ fontSize: 20 }}>↓</span>
      </div>
    </section>
  );
}

/* ---------- Act 1: the calm worktree ---------- */
function CalmScene() {
  return (
    <section style={sceneWrap}>
      <Reveal style={{ textAlign: "center", maxWidth: 880 }}>
        <div className="mono" style={{ color: "var(--safe)", letterSpacing: 3, fontSize: 12, marginBottom: 20 }}>
          ✓ ISOLATED WORKTREE
        </div>
        <h2 style={{ fontSize: "clamp(28px,4.4vw,52px)", fontWeight: 600, margin: 0, lineHeight: 1.08, letterSpacing: "-0.02em" }}>
          It runs in its own directory.<br />Looks contained.
        </h2>
        <div style={{ margin: "48px auto 0", maxWidth: 420, position: "relative" }}>
          <div style={{ border: "1px solid rgba(46,230,166,.4)", borderRadius: 16, padding: "30px 26px",
            background: "linear-gradient(180deg, rgba(46,230,166,.05), transparent)", boxShadow: "var(--glow-safe)" }}>
            <div className="mono" style={{ fontSize: 13, color: "var(--txt-mid)", textAlign: "left", lineHeight: 1.9 }}>
              <div style={{ color: "var(--safe)" }}>&lt;your-project&gt;/.worktrees/&lt;task&gt;</div>
              <div>├─ src/</div>
              <div>├─ package.json</div>
              <div>└─ <span style={{ color: "var(--txt)" }}>agent</span> <span style={{ color: "var(--txt-dim)" }}>// scoped to this folder, right?</span></div>
            </div>
          </div>
        </div>
        <p style={{ color: "var(--txt-mid)", fontSize: 18, marginTop: 36, lineHeight: 1.55 }}>
          A worktree changes the working directory. <strong style={{ color: "var(--txt)" }}>And nothing else.</strong>
        </p>
      </Reveal>
    </section>
  );
}

/* ---------- Act 2: the reveal ---------- */
function RevealScene() {
  return (
    <section style={{ ...sceneWrap, minHeight: "92vh" }}>
      <Reveal style={{ textAlign: "center", maxWidth: 980 }}>
        <h2 style={{ fontSize: "clamp(34px,6vw,84px)", fontWeight: 700, margin: 0, lineHeight: 1.0, letterSpacing: "-0.03em" }}>
          But the agent<br />runs <span style={{ color: "var(--hot)" }}>as you.</span>
        </h2>
        <p style={{ color: "var(--txt-mid)", fontSize: "clamp(17px,2vw,22px)", maxWidth: 660, margin: "32px auto 0", lineHeight: 1.55 }}>
          Same user. Same shell. Same keys. The moment it starts, it inherits every
          credential, identity, and route your account already holds — far past the folder it's "in."
        </p>
        <div className="mono" style={{ color: "var(--txt-dim)", fontSize: 13, marginTop: 40, letterSpacing: 2 }}>
          here is everything within reach ↓
        </div>
      </Reveal>
    </section>
  );
}

/* ---------- Act 3: the expanding blast radius (sticky, scroll-driven) ---------- */
const RING_COPY = [
  { k: "shell",     t: "The shell it was handed", d: "Secret-named env vars, .env keys, shell history — already loaded before the first prompt." },
  { k: "identity",  t: "Your identity",            d: "SSH private keys, a live ssh-agent, GitHub auth, the git credential store. The keys that say you are you." },
  { k: "cloud",     t: "Your cloud",               d: "AWS profiles mounted into the shell. Provider identity, no extra step required." },
  { k: "neighbors", t: "Every neighboring repo",   d: "Sibling repos sit on disk right next to the task — some carrying their own secrets. The task was one folder; the reach is the whole workspace." },
  { k: "network",   t: "The open network",         d: "Outbound egress works. A push is likely to be accepted. Data has somewhere to go; code has somewhere to land." },
  { k: "host",      t: "The whole machine",        d: "Docker group to root. Process memory. Browser sessions. Past the repo, past the cloud — the box itself." },
];

function RadiusScene({ onEnter }) {
  const ref = React.useRef(null);
  const p = useScrollProgress(ref);
  const revealed = p * 6.2;
  const activeIdx = Math.max(0, Math.min(5, Math.floor(revealed - 0.15)));
  const BR = window.BR;
  const counts = BR.counts();

  return (
    <section ref={ref} style={{ height: "560vh", position: "relative" }}>
      <div style={{ position: "sticky", top: 0, height: "100vh", display: "grid",
        gridTemplateColumns: "minmax(0,1fr) minmax(340px, 440px)", alignItems: "center", overflow: "hidden" }}>

        {/* left: the radius */}
        <div style={{ position: "relative", height: "100%", minHeight: 0 }}>
          <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
            <div style={{ width: "min(86vh, 100%)", aspectRatio: "1", maxWidth: 820 }}>
              <RadiusViz revealed={revealed} />
            </div>
          </div>
          {/* center label */}
          <div style={{ position: "absolute", left: "50%", top: "50%", transform: "translate(-50%,calc(-50% + 44px))",
            textAlign: "center", pointerEvents: "none" }}>
            <div className="mono" style={{ fontSize: 12, color: "#1a0a04", fontWeight: 700, background: "var(--hot-2)",
              padding: "2px 8px", borderRadius: 5, letterSpacing: 1 }}>YOU</div>
          </div>
        </div>

        {/* right: stepped captions */}
        <div style={{ padding: "0 clamp(24px,4vw,64px)", position: "relative" }}>
          <div className="mono" style={{ color: "var(--txt-dim)", fontSize: 12, letterSpacing: 2, marginBottom: 18 }}>
            THE REACHABLE SURFACE — <span className="sev-exposed">{counts.exposed} exposed</span> · <span className="sev-notable">{counts.notable} notable</span>
          </div>
          <div style={{ position: "relative", minHeight: 220 }}>
            {RING_COPY.map((rc, i) => {
              const on = activeIdx === i;
              return (
                <div key={rc.k} style={{ position: on ? "relative" : "absolute", inset: on ? "auto" : 0,
                  opacity: on ? 1 : 0, transform: on ? "translateY(0)" : "translateY(14px)",
                  transition: "opacity .45s ease, transform .45s ease", pointerEvents: "none" }}>
                  <div className="mono" style={{ color: "var(--hot)", fontSize: 13, marginBottom: 10 }}>
                    {String(i + 1).padStart(2, "0")} / 06
                  </div>
                  <h3 style={{ fontSize: "clamp(26px,3vw,40px)", fontWeight: 600, margin: "0 0 16px", lineHeight: 1.08, letterSpacing: "-0.02em" }}>
                    {rc.t}
                  </h3>
                  <p style={{ color: "var(--txt-mid)", fontSize: 17, lineHeight: 1.55, margin: 0, maxWidth: 420 }}>{rc.d}</p>
                  <ChipMarquee findings={(BR.LIVE_RINGS[i] || BR.RINGS[i]).findings} />
                </div>
              );
            })}
          </div>

          {/* progress rail */}
          <div style={{ display: "flex", gap: 6, marginTop: 40 }}>
            {RING_COPY.map((_, i) => (
              <div key={i} style={{ height: 3, flex: 1, borderRadius: 2,
                background: i <= activeIdx ? "var(--hot)" : "rgba(255,255,255,0.1)", transition: "background .3s ease" }} />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

/* ---------- bridge into the dashboard ---------- */
function Bridge({ onEnter }) {
  return (
    <section style={{ ...sceneWrap, minHeight: "96vh" }}>
      <Reveal style={{ textAlign: "center", maxWidth: 900 }}>
        <div className="mono" style={{ color: "var(--txt-dim)", letterSpacing: 2, fontSize: 12, marginBottom: 20 }}>
          THAT NEVER CHANGES BETWEEN SESSIONS
        </div>
        <h2 style={{ fontSize: "clamp(30px,5vw,64px)", fontWeight: 600, margin: 0, lineHeight: 1.05, letterSpacing: "-0.02em" }}>
          The real question isn't<br />what it <em style={{ fontStyle: "normal", color: "var(--txt-dim)" }}>can</em> reach.
          <br />It's what it <span style={{ color: "var(--hot)" }}>actually touches.</span>
        </h2>
        <p style={{ color: "var(--txt-mid)", fontSize: 19, margin: "30px auto 40px", maxWidth: 600, lineHeight: 1.55 }}>
          One session reads two files and runs the tests. Another quietly chains a credential,
          an open route, and a deploy file into something far worse. Same machine. Same reach.
          Watch the difference.
        </p>
        <button className="btn btn-hot" style={{ fontSize: 16, padding: "14px 28px" }} onClick={onEnter}>
          Open the session blast-radius score →
        </button>
      </Reveal>
    </section>
  );
}

function Narrative({ onEnter }) {
  return (
    <div>
      <Hook />
      <CalmScene />
      <RevealScene />
      <RadiusScene />
      <Bridge onEnter={onEnter} />
    </div>
  );
}

Object.assign(window, { Narrative, useScrollProgress, Reveal });

  </script>
  <script type="text/babel">
/* dashboard.jsx — the interactive session blast-radius score (Acts 4-5) */

/* §23/§24 are FROZEN ILLUSTRATIVE FIXTURES — no Rust engine backs them. */
const ILLUSTRATIVE_NOTE = "illustrative — post-MVP, not from your scan";

function IllustrativeBadge({ style }) {
  return (
    <span className="mono" style={{ fontSize: 10, fontWeight: 600, color: "var(--info)",
      border: "1px solid var(--info)", padding: "2px 8px", borderRadius: 5, letterSpacing: 0.5,
      whiteSpace: "nowrap", ...style }}>
      {ILLUSTRATIVE_NOTE}
    </span>
  );
}

const LEVEL_COLOR = { low: "var(--safe)", medium: "var(--warn)", high: "var(--hot)", critical: "var(--crit)" };
const LEVEL_LABEL = { low: "LOW", medium: "MEDIUM", high: "HIGH", critical: "CRITICAL" };
const DECISION = {
  allow:         { t: "ALLOW",          c: "var(--safe)" },
  require_review:{ t: "REQUIRE REVIEW", c: "var(--warn)" },
  block:         { t: "BLOCK",          c: "var(--crit)" },
};

/* cumulative score after each played event (drama-tuned) */
const CUM = { risky: [18, 24, 49, 64, 89, 96], benign: [4, 6, 12, 12, 12] };
/* which combos become active after step N (1-indexed event count) */
const COMBO_AT = { production_deployment_path: 3, exfiltration_path: 4, source_control_mutation_path: 6, high_review_risk: 6 };

/* ---------- radial score gauge ---------- */
function ScoreGauge({ score, level }) {
  const R = 92, CX = 110, CY = 110, START = 135, SWEEP = 270;
  const polar = (ang, r) => [CX + r * Math.cos(ang * Math.PI / 180), CY + r * Math.sin(ang * Math.PI / 180)];
  const arc = (a0, a1, r) => {
    const [x0, y0] = polar(a0, r), [x1, y1] = polar(a1, r);
    const large = (a1 - a0) > 180 ? 1 : 0;
    return `M ${x0} ${y0} A ${r} ${r} 0 ${large} 1 ${x1} ${y1}`;
  };
  const valAng = START + SWEEP * (score / 100);
  const col = LEVEL_COLOR[level];
  return (
    <svg viewBox="0 0 220 220" style={{ width: "100%", maxWidth: 240, display: "block", margin: "0 auto" }}>
      <path d={arc(START, START + SWEEP, R)} fill="none" stroke="rgba(255,255,255,0.08)" strokeWidth="14" strokeLinecap="round" />
      {/* zone ticks */}
      {[25, 50, 75].map((z) => {
        const a = START + SWEEP * (z / 100);
        const [x0, y0] = polar(a, R - 9), [x1, y1] = polar(a, R + 9);
        return <line key={z} x1={x0} y1={y0} x2={x1} y2={y1} stroke="rgba(255,255,255,0.16)" strokeWidth="2" />;
      })}
      {score > 0 && (
        <path d={arc(START, valAng, R)} fill="none" stroke={col} strokeWidth="14" strokeLinecap="round"
          style={{ transition: "all .6s cubic-bezier(.3,1,.4,1)", filter: `drop-shadow(0 0 8px ${col})` }} />
      )}
      <text x="110" y="104" textAnchor="middle" fontFamily="var(--mono)" fontSize="52" fontWeight="700" fill={col}
        style={{ transition: "fill .4s" }}>{score}</text>
      <text x="110" y="132" textAnchor="middle" fontFamily="var(--mono)" fontSize="13" fill="var(--txt-dim)" letterSpacing="2">/ 100</text>
      <text x="110" y="166" textAnchor="middle" fontFamily="var(--sans)" fontSize="15" fontWeight="700" fill={col} letterSpacing="2"
        style={{ transition: "fill .4s" }}>{LEVEL_LABEL[level]}</text>
    </svg>
  );
}

/* ---------- session timeline ---------- */
const EVENT_ICON = { fileRead: "◧", fileWrite: "✎", shell: "❯", network: "⇅", approval: "✓" };
function Timeline({ session, playStep, onJump }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      {session.events.map((e, i) => {
        const played = i < playStep;
        const isLast = i === playStep - 1;
        const hot = e.hot && played;
        return (
          <div key={i} onClick={() => onJump(i + 1)}
            style={{ display: "grid", gridTemplateColumns: "26px 1fr auto", alignItems: "center", gap: 12,
              padding: "9px 12px", borderRadius: 9, cursor: "pointer",
              background: isLast ? "rgba(255,91,53,.10)" : "transparent",
              border: `1px solid ${isLast ? "rgba(255,91,53,.3)" : "transparent"}`,
              opacity: played ? 1 : 0.32, transition: "all .35s ease" }}>
            <span className="mono" style={{ fontSize: 15, color: hot ? "var(--hot)" : "var(--txt-dim)", textAlign: "center" }}>
              {EVENT_ICON[e.t]}
            </span>
            <div style={{ minWidth: 0 }}>
              <div className="mono" style={{ fontSize: 13, color: hot ? "var(--txt)" : "var(--txt-mid)",
                whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                <span style={{ color: "var(--txt-dim)" }}>{e.title}&nbsp;</span>{e.arg}
              </div>
              {e.note && played && <div className="mono" style={{ fontSize: 11, color: "var(--safe-deep)", marginTop: 2 }}>↳ {e.note}</div>}
            </div>
            {e.weight > 0 && played
              ? <span className="mono" style={{ fontSize: 12, color: "var(--hot)", fontWeight: 700 }}>+{e.weight}</span>
              : <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>{played ? "—" : ""}</span>}
          </div>
        );
      })}
    </div>
  );
}

/* ---------- toxic combinations ---------- */
function ToxicPanel({ combos, picked, onPick }) {
  if (combos.length === 0) {
    return <div className="mono" style={{ fontSize: 13, color: "var(--txt-dim)", padding: "18px 4px", lineHeight: 1.6 }}>
      No toxic combinations activated. Reachable authority stayed in the denominator — nothing chained.
    </div>;
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {combos.map((c) => {
        const col = c.sev === "critical" ? "var(--crit)" : "var(--hot)";
        const sel = picked === c.name;
        return (
          <div key={c.name} onClick={() => onPick(sel ? null : c.name)}
            style={{ border: `1px solid ${sel ? col : "var(--line-2)"}`, borderRadius: 12, padding: "14px 16px",
              background: sel ? "rgba(255,61,87,.07)" : "var(--surface)", cursor: "pointer", transition: "all .2s",
              animation: "fadeUp .5s ease" }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
              <span style={{ fontWeight: 600, fontSize: 15 }}>{c.title}</span>
              <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: col, border: `1px solid ${col}`,
                padding: "2px 7px", borderRadius: 5, letterSpacing: 1 }}>{c.sev.toUpperCase()}</span>
            </div>
            <p style={{ color: "var(--txt-mid)", fontSize: 13, lineHeight: 1.5, margin: "8px 0 0" }}>{c.derived}</p>
            {sel && (
              <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 5, borderTop: "1px solid var(--line)", paddingTop: 12 }}>
                {c.evidence.map((ev, i) => (
                  <div key={i} className="mono" style={{ fontSize: 12, color: "var(--txt-mid)" }}>{ev}</div>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

/* ---------- containment simulator ---------- */
function Containment({ active, toggle, sessionId }) {
  const BR = window.BR;
  const sim = BR.simulate(active);
  const enabled = sessionId === "risky";
  const baseline = 96;
  // Each control's per-button number is the SAME stacked-ladder delta the
  // simulator applies (BR.simulate sums LADDER step.delta), so the label always
  // equals the score change when the control is toggled.
  const ladderDelta = {};
  BR.LADDER.forEach((s) => { if (s.control) ladderDelta[s.control] = s.delta; });
  if (!enabled) {
    return (
      <div>
        <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2, marginBottom: 12 }}>BLAST RADIUS UNDER CONTAINMENT</div>
        <p style={{ fontSize: 13.5, color: "var(--txt-mid)", lineHeight: 1.6, margin: 0 }}>
          This session is already low-risk — there's almost nothing to contain. Load the
          <span style={{ color: "var(--crit)", fontWeight: 600 }}> risky session</span> to see each control
          peel points off the same score.
        </p>
      </div>
    );
  }
  return (
    <div>
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2 }}>BLAST RADIUS UNDER CONTAINMENT</div>
        </div>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span className="mono" style={{ fontSize: 13, color: "var(--txt-dim)", textDecoration: "line-through" }}>{baseline}</span>
          <span style={{ color: "var(--txt-dim)" }}>→</span>
          <span className="mono" style={{ fontSize: 30, fontWeight: 700, color: LEVEL_COLOR[sim.level], transition: "color .3s" }}>{sim.score}</span>
        </div>
      </div>

      {/* descending bar */}
      <div style={{ height: 8, borderRadius: 5, background: "rgba(255,255,255,.07)", overflow: "hidden", marginBottom: 18 }}>
        <div style={{ height: "100%", width: `${sim.score}%`, background: LEVEL_COLOR[sim.level],
          transition: "width .55s cubic-bezier(.3,1,.4,1), background .3s" }} />
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {BR.CONTROLS.map((c) => {
          const on = active.has(c.id);
          return (
            <button key={c.id} onClick={() => toggle(c.id)} disabled={!enabled}
              style={{ textAlign: "left", display: "grid", gridTemplateColumns: "auto 1fr auto", gap: 12, alignItems: "center",
                padding: "11px 14px", borderRadius: 10, cursor: enabled ? "pointer" : "default",
                background: on ? "rgba(46,230,166,.08)" : "var(--surface)",
                border: `1px solid ${on ? "rgba(46,230,166,.45)" : "var(--line)"}`,
                opacity: enabled ? 1 : 0.4, transition: "all .2s", fontFamily: "var(--sans)", color: "var(--txt)" }}>
              <span style={{ width: 34, height: 19, borderRadius: 11, background: on ? "var(--safe)" : "rgba(255,255,255,.14)",
                position: "relative", transition: "background .2s", flexShrink: 0 }}>
                <span style={{ position: "absolute", top: 2, left: on ? 17 : 2, width: 15, height: 15, borderRadius: "50%",
                  background: on ? "#06281e" : "#fff", transition: "left .2s" }} />
              </span>
              <span>
                <span style={{ fontSize: 14, fontWeight: 600 }}>{c.label}</span>
                <span style={{ display: "block", fontSize: 11.5, color: "var(--txt-dim)", marginTop: 1 }}>{c.cat}</span>
              </span>
              <span className="mono" style={{ fontSize: 12, color: on ? "var(--safe)" : "var(--txt-dim)", whiteSpace: "nowrap" }}>
                −{ladderDelta[c.id] != null ? Math.abs(ladderDelta[c.id]) : c.indep}
              </span>
            </button>
          );
        })}
      </div>

      {/* residual */}
      <div style={{ marginTop: 16, padding: "12px 14px", borderRadius: 10, border: "1px dashed var(--line-2)", background: "rgba(255,255,255,.02)" }}>
        <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 1, marginBottom: 6 }}>IRREDUCIBLE RESIDUAL · {BR.RESIDUAL.floor}</div>
        <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>{BR.RESIDUAL.reason}</div>
      </div>
    </div>
  );
}

/* ---------- finding detail popover ---------- */
function FindingDetail({ id, onClose }) {
  const f = window.BR.FINDINGS[id];
  if (!f) return null;
  const col = SEV_COLOR[f.sev];
  return (
    <div style={{ position: "absolute", left: 20, bottom: 20, width: 320, background: "var(--bg-2)",
      border: `1px solid ${col}`, borderRadius: 14, padding: 18, boxShadow: "0 20px 50px rgba(0,0,0,.6)", zIndex: 30 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
        <div>
          <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)" }}>{f.id}</div>
          <div style={{ fontSize: 17, fontWeight: 600, marginTop: 3 }}>{f.title}</div>
        </div>
        <button onClick={onClose} className="btn btn-ghost" style={{ padding: "2px 9px", fontSize: 16 }}>×</button>
      </div>
      <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
        <span className="mono" style={{ fontSize: 11, color: col, border: `1px solid ${col}`, padding: "2px 8px", borderRadius: 5 }}>
          {window.BR.SEV_LABEL[f.sev]}
        </span>
        <span className="mono" style={{ fontSize: 12, color: "var(--txt-mid)", padding: "2px 0" }}>{f.metric}</span>
      </div>
      {f.why && (
        <div style={{ marginTop: 13 }}>
          <div className="mono" style={{ fontSize: 10, color: "var(--txt-dim)", letterSpacing: 1.5, marginBottom: 4 }}>WHY IT'S RISKY</div>
          <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>{f.why}</div>
        </div>
      )}
      {f.how && (
        <div style={{ marginTop: 12 }}>
          <div className="mono" style={{ fontSize: 10, color: "var(--txt-dim)", letterSpacing: 1.5, marginBottom: 4 }}>WHAT AN AGENT CAN DO</div>
          <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>{f.how}</div>
        </div>
      )}
      {f.remediation && f.remediation.length > 0 && (
        <div style={{ marginTop: 12 }}>
          <div className="mono" style={{ fontSize: 10, color: "var(--safe)", letterSpacing: 1.5, marginBottom: 4 }}>CONTAIN IT</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {f.remediation.map((r, i) => <div key={i} style={{ fontSize: 12, color: "var(--txt-mid)", lineHeight: 1.45 }}>· {r}</div>)}
          </div>
        </div>
      )}
      {f.detail && f.detail.length > 0 && (
        <div style={{ marginTop: 12, paddingTop: 10, borderTop: "1px solid var(--line)", display: "flex", flexDirection: "column", gap: 4 }}>
          <div className="mono" style={{ fontSize: 10, color: "var(--txt-dim)", letterSpacing: 1.5, marginBottom: 2 }}>OBSERVED</div>
          {f.detail.map((d, i) => <div key={i} className="mono" style={{ fontSize: 12, color: "var(--txt-mid)" }}>· {d}</div>)}
        </div>
      )}
    </div>
  );
}

/* ====== DEMO fallback (illustrative benign/risky fixture, no transcripts) ====== */
function DemoDashboard() {
  const BR = window.BR;
  const [sessionId, setSessionId] = React.useState("risky");
  const [playStep, setPlayStep] = React.useState(0);
  const [playing, setPlaying] = React.useState(false);
  const [active, setActive] = React.useState(new Set());
  const [pickedNode, setPickedNode] = React.useState(null);
  const [pickedCombo, setPickedCombo] = React.useState(null);
  const session = BR.SESSIONS[sessionId];
  const total = session.events.length;

  // auto-play on mount / session change
  React.useEffect(() => {
    setPlayStep(0); setActive(new Set()); setPickedNode(null); setPickedCombo(null);
    const start = setTimeout(() => setPlaying(true), 500);
    return () => clearTimeout(start);
  }, [sessionId]);

  React.useEffect(() => {
    if (!playing) return;
    if (playStep >= total) { setPlaying(false); return; }
    const id = setTimeout(() => setPlayStep((s) => s + 1), 950);
    return () => clearTimeout(id);
  }, [playing, playStep, total]);

  // derived: which combos are active given playStep
  const activeCombos = session.combos
    .filter((name) => (COMBO_AT[name] || 99) <= playStep)
    .map((name) => BR.COMBOS[name]);

  // suppressed findings from active controls (containment)
  const suppressed = new Set();
  if (sessionId === "risky") BR.CONTROLS.forEach((c) => { if (active.has(c.id)) c.suppresses.forEach((s) => suppressed.add(s)); });

  // active findings = played events' refs + active combo nodes
  const activeFindings = new Set();
  session.events.slice(0, playStep).forEach((e) => { if (e.ref) activeFindings.add(e.ref); });
  activeCombos.forEach((c) => c.nodes.forEach((n) => activeFindings.add(n)));

  // combos that survive containment (for the constellation paths)
  const sim = BR.simulate(active);
  const liveCombos = activeCombos.filter((c) => !c.legs.some((leg) => sim.killed.has(c.name)))
    .filter((c) => !c.nodes.some((n) => suppressed.has(n)));

  // current score: during play follow CUM; once contained, follow simulator
  const anyControl = active.size > 0 && sessionId === "risky";
  const playedScore = playStep === 0 ? 0 : (CUM[sessionId][playStep - 1] || session.score);
  const curScore = anyControl ? sim.score : playedScore;
  const curLevel = anyControl ? sim.level : (playStep < total ? BR.levelOf(playedScore) : session.level);
  const decision = anyControl
    ? (curScore >= 75 ? "block" : curScore >= 50 ? "require_review" : "allow")
    : (playStep < total ? "allow" : session.decision);

  const toggle = (id) => setActive((prev) => { const n = new Set(prev); n.has(id) ? n.delete(id) : n.add(id); return n; });

  return (
    <section style={{ minHeight: "100vh", padding: "20px clamp(16px,3vw,40px) 60px" }}>
      {/* header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: 16,
        padding: "8px 0 22px", borderBottom: "1px solid var(--line)", marginBottom: 24 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <div style={{ width: 30, height: 30, borderRadius: "50%", background: "radial-gradient(circle, var(--hot-2), var(--hot) 55%, transparent)" }} />
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span className="mono" style={{ fontSize: 15, fontWeight: 700, letterSpacing: 1 }}>blastradius</span>
              <IllustrativeBadge />
            </div>
            <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)" }}>session blast-radius score</div>
          </div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
          {/* session selector */}
          <div style={{ display: "flex", background: "var(--surface)", border: "1px solid var(--line-2)", borderRadius: 10, padding: 3 }}>
            {["benign", "risky"].map((s) => (
              <button key={s} onClick={() => setSessionId(s)} className="mono"
                style={{ border: "none", borderRadius: 7, padding: "8px 16px", cursor: "pointer", fontSize: 13, fontWeight: 600,
                  background: sessionId === s ? (s === "risky" ? "var(--crit)" : "var(--safe)") : "transparent",
                  color: sessionId === s ? "#0b0e14" : "var(--txt-mid)", transition: "all .18s", letterSpacing: 1 }}>
                {s === "risky" ? "RISKY SESSION" : "BENIGN SESSION"}
              </button>
            ))}
          </div>
          <button className="btn" onClick={() => { setPlayStep(0); setActive(new Set()); setTimeout(() => setPlaying(true), 200); }}>
            ↺ Replay
          </button>
        </div>
      </div>

      {/* session label */}
      <div style={{ marginBottom: 18 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>session:</span>
          <span style={{ fontSize: 18, fontWeight: 600 }}>{session.label}</span>
          <span style={{ color: "var(--txt-dim)", fontSize: 14 }}>— {session.sub}</span>
        </div>
      </div>

      {/* main grid */}
      <div style={{ display: "grid", gridTemplateColumns: "minmax(0,1.55fr) minmax(360px, 1fr)", gap: 24, alignItems: "start" }}>

        {/* LEFT: constellation + timeline */}
        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          <div style={{ position: "relative", background: "radial-gradient(circle at 50% 45%, #0d111a, #07090d 75%)",
            border: "1px solid var(--line)", borderRadius: 18, aspectRatio: "1.15", minHeight: 380, overflow: "hidden" }}>
            <Constellation activeFindings={activeFindings} combos={liveCombos} suppressed={suppressed}
              picked={pickedNode} onPick={(id) => { setPickedNode(id); setPickedCombo(null); }} />
            <div style={{ position: "absolute", top: 16, left: 18 }}>
              <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2 }}>REACHABLE SURFACE</div>
              <div style={{ fontSize: 13, color: "var(--txt-mid)", marginTop: 3 }}>
                <span style={{ color: "var(--hot)" }}>●</span> touched ·
                <span style={{ color: "var(--txt-dim)" }}> ○ reachable, untouched</span>
              </div>
              <div className="mono" style={{ fontSize: 10.5, color: "var(--txt-dim)", marginTop: 5 }}>
                sample of ~{window.BR.BREADTH.probes} probes · ~{window.BR.BREADTH.stores} credential stores
              </div>
            </div>
            {pickedNode && <FindingDetail id={pickedNode} onClose={() => setPickedNode(null)} />}
            <div style={{ position: "absolute", bottom: 14, right: 18, textAlign: "right" }}>
              <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)" }}>click any node to inspect</div>
            </div>
          </div>

          {/* timeline */}
          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "16px 16px 18px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
              <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2 }}>SESSION TIMELINE</div>
              <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>{Math.min(playStep, total)} / {total} events</div>
            </div>
            <Timeline session={session} playStep={playStep} onJump={(n) => { setPlaying(false); setPlayStep(n); }} />
          </div>
        </div>

        {/* RIGHT: score + combos + containment */}
        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          {/* score card */}
          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "22px 18px 18px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, gap: 8 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2 }}>BLAST-RADIUS SCORE</span>
                <IllustrativeBadge />
              </div>
              <span className="mono" style={{ fontSize: 11, fontWeight: 700, color: DECISION[decision].c,
                border: `1px solid ${DECISION[decision].c}`, padding: "3px 9px", borderRadius: 6, letterSpacing: 1 }}>
                {DECISION[decision].t}
              </span>
            </div>
            <ScoreGauge score={curScore} level={curLevel} />
            <p style={{ fontSize: 13, color: "var(--txt-mid)", lineHeight: 1.55, textAlign: "center", margin: "6px 14px 0" }}>
              {sessionId === "benign"
                ? "Enormous ambient reach. Almost none of it touched. The score follows what the agent did — not what it could do."
                : (anyControl ? "Recomputed under containment — the same session, fewer reachable legs."
                   : "What's reachable is the denominator. What the agent touched, and how it chains, is the score.")}
            </p>
          </div>

          {/* toxic combinations */}
          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "16px 16px 18px" }}>
            <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2, marginBottom: 12 }}>
              TOXIC COMBINATIONS {activeCombos.length > 0 && <span style={{ color: "var(--crit)" }}>· {liveCombos.length} active</span>}
            </div>
            <ToxicPanel combos={liveCombos} picked={pickedCombo} onPick={(n) => { setPickedCombo(n); setPickedNode(null); }} />
          </div>

          {/* containment */}
          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "18px 16px" }}>
            <Containment active={active} toggle={toggle} sessionId={sessionId} />
          </div>
        </div>
      </div>
    </section>
  );
}

/* ============================ REAL RANKED SESSIONS ============================ */
/* Driven by D.sessions.ranked — the top-N real discovered sessions, ranked by
 * blast-radius score in the engine (session::report::rank_sessions). Each card is
 * a real, value-free SessionReport; the `how` rows are the aggregated breakdown of
 * exactly how that transcript earned its score. No illustrative badge — this is
 * the user's own data. */

const SIGNAL_ICON = {
  read_secret: "◧", network_access: "⇅", modified_production_deploy: "✎",
  edited_auth_payment_security_code: "✎", edited_auth_payment_security: "✎",
  dangerous_shell_pattern: "❯", shell_command: "❯", modified_dependency_manifest: "✎",
  external_mcp_call: "⇄", human_approved_risky_action: "✓",
};
const signalLabel = (s) => String(s || "").replace(/_/g, " ");

const CONTROL_LABEL = {
  repo_only_filesystem: "Repo-only filesystem", no_egress: "No egress",
  no_ssh_agent: "No ssh-agent", scoped_temp_cloud_creds: "Scoped temp creds",
  process_isolation: "Process isolation", all_controls: "All controls",
};

/* "HOW this transcript is risky" — the aggregated scored signals. */
function HowPanel({ how }) {
  if (!how || !how.length) {
    return <div className="mono" style={{ fontSize: 13, color: "var(--txt-dim)", padding: "14px 4px", lineHeight: 1.6 }}>
      Nothing scored — the agent's actions stayed inside the denominator.
    </div>;
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      {how.map((h, i) => {
        const hot = h.weight_total > 0;
        return (
          <div key={i} style={{ display: "grid", gridTemplateColumns: "26px 1fr auto", alignItems: "center", gap: 12,
            padding: "9px 12px", borderRadius: 9, background: i % 2 ? "transparent" : "rgba(255,255,255,.02)" }}>
            <span className="mono" style={{ fontSize: 15, color: hot ? "var(--hot)" : "var(--txt-dim)", textAlign: "center" }}>
              {SIGNAL_ICON[h.signal] || "•"}
            </span>
            <div style={{ minWidth: 0 }}>
              <div className="mono" style={{ fontSize: 13, color: "var(--txt)" }}>
                {signalLabel(h.signal)}
                {h.count > 1 && <span style={{ color: "var(--txt-dim)" }}> ×{h.count}</span>}
              </div>
              {h.why && <div style={{ fontSize: 11.5, color: "var(--txt-dim)", marginTop: 3, lineHeight: 1.45 }}>{h.why}</div>}
              {h.finding_ref && <div className="mono" style={{ fontSize: 11, color: "var(--safe-deep)", marginTop: 2 }}>↳ joins {h.finding_ref}</div>}
            </div>
            <span className="mono" style={{ fontSize: 12, fontWeight: 700, color: hot ? "var(--hot)" : "var(--txt-dim)" }}>
              {h.weight_total > 0 ? "+" + h.weight_total : h.weight_total}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/* Real toxic-combination panel (from SessionReport.toxic_combinations). */
function RealToxic({ combos, picked, onPick }) {
  if (!combos || !combos.length) {
    return <div className="mono" style={{ fontSize: 13, color: "var(--txt-dim)", padding: "18px 4px", lineHeight: 1.6 }}>
      No toxic combinations — risky actions never chained with reachable findings.
    </div>;
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {combos.map((c) => {
        const meta = comboMeta(c.name);
        const sev = (c.severity || meta.sev || "high");
        const col = sev === "critical" ? "var(--crit)" : "var(--hot)";
        const sel = picked === c.name;
        return (
          <div key={c.name} onClick={() => onPick(sel ? null : c.name)}
            style={{ border: `1px solid ${sel ? col : "var(--line-2)"}`, borderRadius: 12, padding: "14px 16px",
              background: sel ? "rgba(255,61,87,.07)" : "var(--surface)", cursor: "pointer", transition: "all .2s" }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
              <span style={{ fontWeight: 600, fontSize: 15 }}>{meta.title || c.name}</span>
              <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: col, border: `1px solid ${col}`,
                padding: "2px 7px", borderRadius: 5, letterSpacing: 1 }}>{String(sev).toUpperCase()}</span>
            </div>
            {meta.derived && (
              <div style={{ marginTop: 7, fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>{meta.derived}</div>
            )}
            {sel && (c.evidence || []).length > 0 && (
              <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 5, borderTop: "1px solid var(--line)", paddingTop: 12 }}>
                {c.evidence.map((ev, i) => <div key={i} className="mono" style={{ fontSize: 12, color: "var(--txt-mid)" }}>{ev}</div>)}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

/* Real containment ladder (from SessionReport.containment_simulation — the engine
 * computed it; the page only renders it, never recomputes §23.13). */
function RealContainment({ sim }) {
  if (!sim || !sim.stacked || !sim.stacked.length) return null;
  const baseline = sim.baseline_score;
  const floor = sim.residual_floor;
  return (
    <div>
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", marginBottom: 14 }}>
        <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2 }}>BLAST RADIUS UNDER CONTAINMENT</div>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span className="mono" style={{ fontSize: 13, color: "var(--txt-dim)", textDecoration: "line-through" }}>{baseline}</span>
          <span style={{ color: "var(--txt-dim)" }}>→</span>
          <span className="mono" style={{ fontSize: 30, fontWeight: 700, color: LEVEL_COLOR[BR_LEVEL(floor)] }}>{floor}</span>
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {sim.stacked.filter((r) => r.control).map((r, i) => (
          <div key={i} style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: 12, alignItems: "center",
            padding: "11px 14px", borderRadius: 10, background: "var(--surface)", border: "1px solid var(--line)" }}>
            <span style={{ fontSize: 14, fontWeight: 600 }}>{CONTROL_LABEL[r.control] || r.control}</span>
            <span className="mono" style={{ fontSize: 12, color: "var(--safe)" }}>−{r.reduction}</span>
            <span className="mono" style={{ fontSize: 13, color: "var(--txt-mid)", width: 34, textAlign: "right" }}>{r.score}</span>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 16, padding: "12px 14px", borderRadius: 10, border: "1px dashed var(--line-2)", background: "rgba(255,255,255,.02)" }}>
        <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 1, marginBottom: 6 }}>IRREDUCIBLE RESIDUAL · {floor}</div>
        <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>
          {(sim.residual_reasons && sim.residual_reasons.length)
            ? sim.residual_reasons.join(" · ")
            : "event-intrinsic risk survives every control — needs human review / server-side enforcement."}
        </div>
      </div>
    </div>
  );
}

function BR_LEVEL(score) { return window.BR.levelOf(score); }

function RankedSessions() {
  const BR = window.BR;
  const sessions = BR.SESSIONS_RANKED;
  const [sel, setSel] = React.useState(0);
  const [pickedNode, setPickedNode] = React.useState(null);
  const [pickedCombo, setPickedCombo] = React.useState(null);
  const s = sessions[Math.min(sel, sessions.length - 1)];

  const level = s.risk_level;
  const decision = s.policy_decision;
  const touched = new Set(s.touched || []);
  // Map real toxic combos to the catalog for node geometry (constellation paths).
  const combos = (s.toxic_combinations || []).map((t) => {
    const meta = BR.COMBOS[t.name] || { name: t.name, nodes: [], sev: t.severity };
    return Object.assign({}, meta, { name: t.name, sev: t.severity, evidence: t.evidence });
  });
  const liveCombos = combos.filter((c) => (c.nodes || []).length >= 2 && c.nodes.every((n) => BR.FINDINGS[n]));
  const activeFindings = new Set(touched);
  liveCombos.forEach((c) => c.nodes.forEach((n) => activeFindings.add(n)));

  return (
    <section style={{ minHeight: "100vh", padding: "20px clamp(16px,3vw,40px) 60px" }}>
      {/* header — REAL data, no illustrative badge */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: 16,
        padding: "8px 0 22px", borderBottom: "1px solid var(--line)", marginBottom: 20 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <div style={{ width: 30, height: 30, borderRadius: "50%", background: "radial-gradient(circle, var(--hot-2), var(--hot) 55%, transparent)" }} />
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span className="mono" style={{ fontSize: 15, fontWeight: 700, letterSpacing: 1 }}>blastradius</span>
              <span className="mono" style={{ fontSize: 10, fontWeight: 600, color: "var(--safe)", border: "1px solid var(--safe)",
                padding: "2px 8px", borderRadius: 5, letterSpacing: 0.5 }}>from your scan</span>
            </div>
            <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)" }}>session blast-radius score · top {sessions.length} ranked</div>
          </div>
        </div>
      </div>

      {/* ranked picker */}
      <div style={{ display: "flex", gap: 8, overflowX: "auto", paddingBottom: 14, marginBottom: 20 }}>
        {sessions.map((c, i) => {
          const on = i === sel;
          const col = LEVEL_COLOR[c.risk_level];
          return (
            <button key={c.session_id} onClick={() => { setSel(i); setPickedNode(null); setPickedCombo(null); }}
              style={{ flex: "0 0 auto", textAlign: "left", borderRadius: 12, padding: "10px 14px", cursor: "pointer",
                background: on ? "var(--surface-2)" : "var(--surface)", border: `1px solid ${on ? col : "var(--line)"}`,
                transition: "all .15s", fontFamily: "var(--sans)", color: "var(--txt)", minWidth: 132 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span className="mono" style={{ fontSize: 11, color: "var(--txt-dim)" }}>#{c.rank}</span>
                <span className="mono" style={{ fontSize: 22, fontWeight: 700, color: col }}>{c.risk_score}</span>
                <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: col, letterSpacing: 1 }}>{LEVEL_LABEL[c.risk_level]}</span>
              </div>
              <div className="mono" style={{ fontSize: 11, color: "var(--txt-mid)", marginTop: 4, whiteSpace: "nowrap" }}>{c.label}</div>
              <div className="mono" style={{ fontSize: 10.5, color: "var(--txt-dim)", marginTop: 2 }}>
                {(c.toxic_combinations || []).length} toxic · {(c.how || []).length} signals
              </div>
            </button>
          );
        })}
      </div>

      {/* selected session label */}
      <div style={{ marginBottom: 18, display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>#{s.rank} · session:</span>
        <span style={{ fontSize: 18, fontWeight: 600 }}>{s.label}</span>
        <span style={{ color: "var(--txt-dim)", fontSize: 14 }}>— {s.summary}</span>
      </div>

      {/* main grid */}
      <div style={{ display: "grid", gridTemplateColumns: "minmax(0,1.55fr) minmax(360px, 1fr)", gap: 24, alignItems: "start" }}>
        {/* LEFT: constellation + how-it's-risky */}
        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          <div style={{ position: "relative", background: "radial-gradient(circle at 50% 45%, #0d111a, #07090d 75%)",
            border: "1px solid var(--line)", borderRadius: 18, aspectRatio: "1.15", minHeight: 380, overflow: "hidden" }}>
            <Constellation activeFindings={activeFindings} combos={liveCombos} suppressed={new Set()}
              picked={pickedNode} onPick={(id) => setPickedNode(id)} />
            <div style={{ position: "absolute", top: 16, left: 18 }}>
              <div className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2 }}>REACHABLE SURFACE</div>
              <div style={{ fontSize: 13, color: "var(--txt-mid)", marginTop: 3 }}>
                <span style={{ color: "var(--hot)" }}>●</span> touched by this session ·
                <span style={{ color: "var(--txt-dim)" }}> ○ reachable, untouched</span>
              </div>
            </div>
            {pickedNode && <FindingDetail id={pickedNode} onClose={() => setPickedNode(null)} />}
          </div>

          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "16px 16px 18px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
              <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2 }}>HOW THIS TRANSCRIPT IS RISKY</div>
              <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>{(s.how || []).length} scored signals</div>
            </div>
            <HowPanel how={s.how} />
          </div>
        </div>

        {/* RIGHT: score + toxic + containment */}
        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "22px 18px 18px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, gap: 8 }}>
              <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2 }}>BLAST-RADIUS SCORE</span>
              <span className="mono" style={{ fontSize: 11, fontWeight: 700, color: DECISION[decision].c,
                border: `1px solid ${DECISION[decision].c}`, padding: "3px 9px", borderRadius: 6, letterSpacing: 1 }}>
                {DECISION[decision].t}
              </span>
            </div>
            <ScoreGauge score={s.risk_score} level={level} />
            <p style={{ fontSize: 13, color: "var(--txt-mid)", lineHeight: 1.55, textAlign: "center", margin: "6px 14px 0" }}>
              What's reachable is the denominator. What this session actually touched, and how it chains, is the score.
            </p>
          </div>

          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "16px 16px 18px" }}>
            <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2, marginBottom: 12 }}>
              TOXIC COMBINATIONS {liveCombos.length + (s.toxic_combinations || []).length - liveCombos.length > 0 && (s.toxic_combinations || []).length > 0 && <span style={{ color: "var(--crit)" }}>· {(s.toxic_combinations || []).length} active</span>}
            </div>
            <RealToxic combos={s.toxic_combinations} picked={pickedCombo} onPick={(n) => setPickedCombo(n)} />
          </div>

          <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "18px 16px" }}>
            <RealContainment sim={s.containment_simulation} />
          </div>
        </div>
      </div>
    </section>
  );
}

/* ============================ DASHBOARD (wrapper) ============================ */
function Dashboard() {
  const ranked = window.BR.SESSIONS_RANKED;
  if (ranked && ranked.length) return <RankedSessions />;
  return <DemoDashboard />;
}

Object.assign(window, { Dashboard, DemoDashboard, RankedSessions, IllustrativeBadge, ILLUSTRATIVE_NOTE });

  </script>
  <script type="text/babel">
/* retro.jsx — §24 AUTO-SLURP + RETRO-HAZARD: "it already happened, and it still matters" */

const AGENT_COLOR = {
  "claude-code": "#d97757", "codex": "#10a37f", "cursor": "#7aa2ff", "copilot": "#c8a8ff",
  "opencode": "#f5a623", "antigravity": "#5ad1c0", "factory": "#ff8a5c", "aider": "#9aa4b6",
};

function StatusPill({ status }) {
  const map = {
    still_reachable: { t: "STILL REACHABLE", c: "var(--crit)" },
    partial:         { t: "PARTIALLY REMEDIATED", c: "var(--warn)" },
    remediated:      { t: "REMEDIATED SINCE", c: "var(--safe)" },
  };
  const m = map[status];
  return <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: m.c, border: `1px solid ${m.c}`,
    padding: "2px 8px", borderRadius: 5, letterSpacing: 1, whiteSpace: "nowrap" }}>{m.t}</span>;
}

// Combo display fallback for real combos not in the illustrative catalog.
function comboMeta(combo) {
  const BR = window.BR;
  return BR.COMBOS[combo] || { title: combo, sev: "high" };
}

function HazardCard({ hz, res, dead }) {
  const BR = window.BR;
  const combo = comboMeta(hz.combo);
  const acol = AGENT_COLOR[hz.agent] || "var(--txt-mid)";
  const sevCol = hz.sev === "critical" ? "var(--crit)" : "var(--hot)";
  const live = res.status === "still_reachable";
  return (
    <div style={{ border: `1px solid ${live ? sevCol : "var(--line)"}`, borderRadius: 14,
      padding: "16px 18px", background: live ? "rgba(255,61,87,.05)" : "rgba(255,255,255,.02)",
      opacity: live ? 1 : 0.62, transition: "all .4s ease" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span style={{ width: 9, height: 9, borderRadius: "50%", background: acol, flexShrink: 0 }} />
          <span className="mono" style={{ fontSize: 12, color: acol, fontWeight: 600 }}>{hz.agent}</span>
          <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>· {hz.age}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <StatusPill status={res.status} />
          <span className="mono" style={{ fontSize: 20, fontWeight: 700, color: live ? sevCol : "var(--txt-dim)" }}>{res.realized}</span>
        </div>
      </div>

      <div style={{ marginTop: 12, fontSize: 16, fontWeight: 600 }}>{combo.title}</div>
      <p style={{ color: "var(--txt-mid)", fontSize: 13.5, lineHeight: 1.5, margin: "6px 0 0" }}>{hz.summary}</p>

      {/* observed events */}
      <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 4, paddingLeft: 12,
        borderLeft: "2px solid var(--line-2)" }}>
        {hz.events.map((e, i) => <div key={i} className="mono" style={{ fontSize: 12, color: "var(--txt-mid)" }}>{e}</div>)}
      </div>

      {/* legs re-resolved against today */}
      <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 5 }}>
        <div className="mono" style={{ fontSize: 10.5, color: "var(--txt-dim)", letterSpacing: 1 }}>RE-RESOLVED VS TODAY'S SURFACE</div>
        {res.legs.map((leg) => (
          <div key={leg.ref} style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className="mono" style={{ fontSize: 13, color: leg.live ? "var(--crit)" : "var(--safe)", width: 14 }}>
              {leg.live ? "●" : "○"}
            </span>
            <span className="mono" style={{ fontSize: 12, color: leg.live ? "var(--txt)" : "var(--txt-dim)",
              textDecoration: leg.live ? "none" : "line-through" }}>{leg.ref}</span>
            <span className="mono" style={{ fontSize: 11, color: leg.live ? "var(--hot)" : "var(--safe)" }}>
              {leg.live ? "still reachable" : "remediated"}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ReviewGapCard({ rg }) {
  const BR = window.BR;
  const combo = comboMeta(rg.combo);
  const acol = AGENT_COLOR[rg.agent] || "var(--txt-mid)";
  return (
    <div style={{ border: "1px dashed var(--line-2)", borderRadius: 14, padding: "16px 18px", background: "rgba(255,255,255,.02)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span style={{ width: 9, height: 9, borderRadius: "50%", background: acol }} />
          <span className="mono" style={{ fontSize: 12, color: acol, fontWeight: 600 }}>{rg.agent}</span>
          <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>· {rg.age}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: "var(--warn)", border: "1px solid var(--warn)",
            padding: "2px 8px", borderRadius: 5, letterSpacing: 1 }}>REVIEW GAP</span>
          <div style={{ textAlign: "right" }}>
            <div className="mono" style={{ fontSize: 20, fontWeight: 700, color: "var(--warn)", lineHeight: 1 }}>{rg.review_score}</div>
            <div className="mono" style={{ fontSize: 9.5, color: "var(--txt-dim)", letterSpacing: 1 }}>review score</div>
          </div>
        </div>
      </div>
      <div style={{ marginTop: 10, fontSize: 15, fontWeight: 600 }}>{combo.title}</div>
      <p style={{ color: "var(--txt-mid)", fontSize: 13, lineHeight: 1.5, margin: "6px 0 10px" }}>{rg.summary}</p>
      <div style={{ display: "flex", flexDirection: "column", gap: 4, paddingLeft: 12, borderLeft: "2px solid var(--line-2)" }}>
        {rg.events.map((e, i) => <div key={i} className="mono" style={{ fontSize: 12, color: "var(--txt-mid)" }}>{e}</div>)}
      </div>
    </div>
  );
}

function RetroSection() {
  const BR = window.BR;
  const real = !!BR.HAS_HISTORY;
  const [dead, setDead] = React.useState(new Set());
  const toggleDead = (id) => setDead((p) => { const n = new Set(p); n.has(id) ? n.delete(id) : n.add(id); return n; });

  const resolved = BR.HAZARDS.map((hz) => ({ hz, res: BR.retroResolve(hz, dead) }));
  const liveOnes = resolved.filter((r) => r.res.status === "still_reachable").sort((a, b) => b.res.realized - a.res.realized);
  const goneOnes = resolved.filter((r) => r.res.status !== "still_reachable").sort((a, b) => b.res.realized - a.res.realized);
  const totalSessions = BR.AGENTS_DISCOVERED.reduce((s, a) => s + a.sessions, 0);

  return (
    <section style={{ padding: "100px clamp(16px,3vw,40px) 70px", borderTop: "1px solid var(--line)",
      background: "linear-gradient(180deg, #090c12, #0a0d14)" }}>
      <div style={{ maxWidth: 1280, margin: "0 auto" }}>

        {/* framing */}
        <div style={{ maxWidth: 820 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap", marginBottom: 18 }}>
            <span className="mono" style={{ color: "var(--txt-dim)", letterSpacing: 3, fontSize: 12 }}>
              AND THE SESSIONS THAT ALREADY RAN
            </span>
            {real
              ? <span className="mono" style={{ fontSize: 10, fontWeight: 600, color: "var(--safe)",
                  border: "1px solid var(--safe)", padding: "2px 8px", borderRadius: 5, letterSpacing: 0.5, whiteSpace: "nowrap" }}>
                  from your scan
                </span>
              : <IllustrativeBadge />}
          </div>
          <h2 style={{ fontSize: "clamp(28px,4.4vw,52px)", fontWeight: 600, margin: 0, lineHeight: 1.08, letterSpacing: "-0.02em" }}>
            Your agents left transcripts<br />on this machine. We read what<br />they <span style={{ color: "var(--hot)" }}>already did.</span>
          </h2>
          <p style={{ color: "var(--txt-mid)", fontSize: 18, lineHeight: 1.55, marginTop: 24 }}>
            No hooks, no instrumentation — just the session logs Claude Code, Codex, Cursor and the
            rest leave on disk. The sharper question isn't whether a risky session is <em style={{ fontStyle: "normal", color: "var(--txt)" }}>possible</em>.
            It's which ones <strong style={{ color: "var(--txt)" }}>already happened — and still matter today.</strong>
          </p>
        </div>

        {/* discovery strip */}
        <div style={{ marginTop: 34, padding: "16px 18px", background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap", marginBottom: 12 }}>
            <span className="mono" style={{ fontSize: 11, color: "var(--txt-dim)", letterSpacing: 2 }}>
              {real
                ? `DISCOVERED LOCALLY — ${totalSessions} SESSION${totalSessions === 1 ? "" : "S"} WITH HAZARDS ACROSS ${BR.AGENTS_DISCOVERED.length} AGENT${BR.AGENTS_DISCOVERED.length === 1 ? "" : "S"}`
                : `DISCOVERED LOCALLY — ${totalSessions} SESSIONS ACROSS ${BR.AGENTS_DISCOVERED.length} AGENTS`}
              {!real && <span style={{ color: "var(--txt-dim)", letterSpacing: 0, fontWeight: 400 }}> · extended demo roster</span>}
            </span>
            {real
              ? <span className="mono" style={{ fontSize: 10, fontWeight: 600, color: "var(--safe)",
                  border: "1px solid var(--safe)", padding: "2px 8px", borderRadius: 5, letterSpacing: 0.5, whiteSpace: "nowrap" }}>
                  from your scan
                </span>
              : <IllustrativeBadge />}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
            {BR.AGENTS_DISCOVERED.map((a) => (
              <span key={a.tag} style={{ display: "inline-flex", alignItems: "center", gap: 7, padding: "6px 11px",
                borderRadius: 8, background: "var(--surface)", border: "1px solid var(--line)" }}>
                <span style={{ width: 8, height: 8, borderRadius: "50%", background: AGENT_COLOR[a.tag] }} />
                <span style={{ fontSize: 13, fontWeight: 600 }}>{a.name}</span>
                <span className="mono" style={{ fontSize: 12, color: "var(--txt-dim)" }}>{a.sessions}</span>
              </span>
            ))}
          </div>
        </div>

        {/* main: remediation rail + ledgers */}
        <div style={{ display: "grid", gridTemplateColumns: "minmax(0,1.7fr) minmax(280px,1fr)", gap: 28, marginTop: 36, alignItems: "start" }}>

          {/* ledgers */}
          <div>
            <div style={{ display: "flex", alignItems: "baseline", gap: 12, marginBottom: 16 }}>
              <span style={{ fontSize: 18, fontWeight: 600 }}>Still reachable</span>
              <span className="mono" style={{ fontSize: 24, fontWeight: 700, color: liveOnes.length ? "var(--crit)" : "var(--safe)",
                transition: "color .3s" }}>{liveOnes.length}</span>
              <span className="mono" style={{ fontSize: 13, color: "var(--txt-dim)" }}>of {resolved.length} historical hazards</span>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              {liveOnes.length === 0 && (
                <div className="mono" style={{ fontSize: 14, color: "var(--safe)", padding: "20px 4px", lineHeight: 1.6 }}>
                  Nothing left live. Every historical hazard's required legs have been remediated — each one
                  dropped into the archive below the moment you cut its reach.
                </div>
              )}
              {liveOnes.map(({ hz, res }) => <HazardCard key={hz.hid} hz={hz} res={res} dead={dead} />)}
            </div>

            {/* remediated ledger */}
            {goneOnes.length > 0 && (
              <div style={{ marginTop: 28 }}>
                <div className="mono" style={{ fontSize: 12, color: "var(--safe)", letterSpacing: 1, marginBottom: 14 }}>
                  ✓ ALREADY REMEDIATED (HISTORICAL) — {goneOnes.length}
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                  {goneOnes.map(({ hz, res }) => <HazardCard key={hz.hid} hz={hz} res={res} dead={dead} />)}
                </div>
              </div>
            )}

            {/* review gap lane */}
            <div style={{ marginTop: 28 }}>
              <div className="mono" style={{ fontSize: 12, color: "var(--warn)", letterSpacing: 1, marginBottom: 14 }}>
                REVIEW-CONTROL GAPS — SEPARATE LANE, NO REACHABILITY CLAIM
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                {BR.REVIEW_GAPS.map((rg) => <ReviewGapCard key={rg.hid} rg={rg} />)}
              </div>
            </div>
          </div>

          {/* remediation rail */}
          <div style={{ position: "sticky", top: 24, display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ background: "var(--bg-1)", border: "1px solid var(--line)", borderRadius: 16, padding: "18px 16px" }}>
              <div className="mono" style={{ fontSize: 12, color: "var(--txt-dim)", letterSpacing: 2, marginBottom: 6 }}>FIX IT NOW — WATCH IT DROP</div>
              <p style={{ fontSize: 13, color: "var(--txt-mid)", lineHeight: 1.5, margin: "0 0 14px" }}>
                Each control removes one reachable leg. A hazard whose required legs are all gone falls
                out of "still reachable" into the archive. <span style={{ color: "var(--txt)" }}>That asymmetry is the whole point.</span>
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {BR.REMEDIATIONS.map((r) => {
                  const on = dead.has(r.id);
                  return (
                    <button key={r.id} onClick={() => toggleDead(r.id)}
                      style={{ textAlign: "left", display: "grid", gridTemplateColumns: "auto 1fr", gap: 11, alignItems: "center",
                        padding: "11px 13px", borderRadius: 10, cursor: "pointer", fontFamily: "var(--sans)", color: "var(--txt)",
                        background: on ? "rgba(46,230,166,.08)" : "var(--surface)",
                        border: `1px solid ${on ? "rgba(46,230,166,.45)" : "var(--line)"}`, transition: "all .2s" }}>
                      <span style={{ width: 34, height: 19, borderRadius: 11, background: on ? "var(--safe)" : "rgba(255,255,255,.14)",
                        position: "relative", transition: "background .2s", flexShrink: 0 }}>
                        <span style={{ position: "absolute", top: 2, left: on ? 17 : 2, width: 15, height: 15, borderRadius: "50%",
                          background: on ? "#06281e" : "#fff", transition: "left .2s" }} />
                      </span>
                      <span style={{ fontSize: 13.5, fontWeight: 600 }}>{r.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* honesty disclaimer */}
            <div style={{ border: "1px solid var(--line-2)", borderRadius: 14, padding: "14px 16px", background: "rgba(90,162,255,.04)" }}>
              <div className="mono" style={{ fontSize: 11, color: "var(--info)", letterSpacing: 1, marginBottom: 7 }}>REACHABILITY, NOT EXPLOITATION</div>
              <p style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.55, margin: 0 }}>
                We are not claiming a secret left the machine. We're showing that a reachable capability
                <em style={{ fontStyle: "normal", color: "var(--txt)" }}> composed</em> with an action the agent actually took —
                the path was open then, and it's open now. Values are never read; only paths, command shapes, and counts.
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

Object.assign(window, { RetroSection });

  </script>
  <script type="text/babel">
/* app.jsx — orchestrates narrative -> dashboard -> retro-hazard -> close */
function Outro() {
  return (
    <section style={{ minHeight: "70vh", display: "flex", alignItems: "center", justifyContent: "center",
      padding: "80px 24px", borderTop: "1px solid var(--line)", background: "var(--bg)" }}>
      <div style={{ textAlign: "center", maxWidth: 860 }}>
        <div className="mono" style={{ color: "var(--txt-dim)", letterSpacing: 3, fontSize: 12, marginBottom: 26 }}>
          THE WHOLE STORY, IN ONE LINE
        </div>
        <p style={{ fontSize: "clamp(22px,3.2vw,38px)", lineHeight: 1.32, fontWeight: 600, letterSpacing: "-0.02em", margin: 0 }}>
          Worktrees hide the problem.<br />
          <span style={{ color: "var(--hot)" }}>blastradius</span> shows the reachable surface,
          the toxic-combination paths it composes into, and the controls that actually shrink the blast radius.
        </p>
        <p style={{ color: "var(--txt-mid)", fontSize: 15, marginTop: 28, lineHeight: 1.6 }}>
          Reachability, not intent. Clarity, not fear. No secret values, ever.
        </p>
      </div>
    </section>
  );
}

function App() {
  const dashRef = React.useRef(null);
  const enter = () => {
    if (dashRef.current) {
      const top = dashRef.current.getBoundingClientRect().top + window.scrollY;
      window.scrollTo({ top, behavior: "smooth" });
    }
  };
  return (
    <div>
      <Narrative onEnter={enter} />
      <div ref={dashRef} style={{ borderTop: "1px solid var(--line)",
        background: "linear-gradient(180deg, #07090d, #090c12)" }}>
        <Dashboard />
      </div>
      <RetroSection />
      <Outro />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);

  </script>
</body>
</html>
"##;
