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
        <div className="mono" style={{ fontSize: 11, color: floor > 0 ? "var(--txt-dim)" : "var(--safe)", letterSpacing: 1, marginBottom: 6 }}>
          {floor > 0 ? "IRREDUCIBLE RESIDUAL · " + floor : "FULLY CONTAINABLE · 0"}
        </div>
        <div style={{ fontSize: 12.5, color: "var(--txt-mid)", lineHeight: 1.5 }}>
          {floor > 0
            ? ((sim.residual_reasons && sim.residual_reasons.length)
                ? sim.residual_reasons.join(" · ")
                : "event-intrinsic risk survives every control — needs human review / server-side enforcement.")
            : "every scored signal in this session is removed by the controls above — the stacked set drops it to zero."}
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
              {/* the score caps at 100, so most top sessions tie there; the raw
                  weight magnitude is the real ranking key — show it to differentiate. */}
              <div className="mono" style={{ fontSize: 10.5, color: "var(--hot)", marginTop: 2 }}>
                Σ {c.weight_total != null ? c.weight_total.toLocaleString() : "—"} weight
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
