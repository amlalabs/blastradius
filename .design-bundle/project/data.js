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

  // Flat index of findings for joins
  const FINDINGS = {};
  RINGS.forEach((r) => r.findings.forEach((f) => { FINDINGS[f.id] = Object.assign({ ring: r.id }, f); }));

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
  // Map each live finding's `severity` value to the `sev` field the viz expects,
  // and its `metric` (summary). Falls back to the canonical RINGS literal when
  // no live rings are present so every fixture node id always resolves.
  const LIVE_RINGS = (D.rings && D.rings.length)
    ? D.rings.map((r) => ({
        id: r.id, n: r.n, label: r.label, blurb: r.blurb,
        findings: (r.findings || []).map((f) => ({
          id: f.id, title: f.title, sev: f.severity,
          metric: f.metric, detail: f.detail || [],
        })),
      }))
    : RINGS;

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
  const AGENTS_DISCOVERED = [
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
  const HAZARDS = [
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
  const REVIEW_GAPS = [
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
  function reachTier(legs) {
    const liveLegs = legs.filter((l) => l.live);
    if (liveLegs.length === 0) return 0.10;
    const allLive = liveLegs.length === legs.length;
    if (!allLive) return 0.45;
    const sevOf = (fid) => {
      const lf = FINDINGS[fid];
      return lf ? lf.sev : null;
    };
    const allExposed = liveLegs.every((l) => sevOf(l.ref) === "exposed");
    return allExposed ? 1.00 : 0.70;
  }

  // Re-resolve one hazard against today's findings minus the `dead` set.
  function retroResolve(hz, dead) {
    const deadAll = new Set([...(hz.deadAtStart || []), ...dead]);
    const legs = hz.required.map((fid) => ({ ref: fid, live: !deadAll.has(fid) }));
    const liveCount = legs.filter((l) => l.live).length;
    const allLive = liveCount === hz.required.length;
    const status = allLive ? "still_reachable" : (liveCount > 0 ? "partial" : "remediated");
    const reach = reachTier(legs);
    const durability = 1 + 0.15 * (liveCount / hz.required.length);
    const realized = Math.max(0, Math.min(100, Math.round(hz.base * reach * durability * retroDecay(hz.ageDays) * 2.5)));
    return { legs, status, realized, live: allLive };
  }

  window.BR = {
    RINGS, LIVE_RINGS, FINDINGS, SESSIONS, COMBOS, CONTROLS, LADDER, RESIDUAL,
    AGENTS_DISCOVERED, REMEDIATIONS, HAZARDS, REVIEW_GAPS, retroResolve,
    counts, levelOf, simulate,
    BREADTH: {
      probes: (D.stats && D.stats.breadth && D.stats.breadth.probes) || 35,
      stores: (D.stats && D.stats.breadth && D.stats.breadth.stores) || 30,
    },
    SEV_LABEL: { exposed: "Exposed", notable: "Notable", info: "Info" },
  };
})();
