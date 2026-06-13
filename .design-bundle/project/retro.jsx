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

function HazardCard({ hz, res, dead }) {
  const BR = window.BR;
  const combo = BR.COMBOS[hz.combo];
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
  const combo = BR.COMBOS[rg.combo];
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
            <IllustrativeBadge />
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
              DISCOVERED LOCALLY — {totalSessions} SESSIONS ACROSS {BR.AGENTS_DISCOVERED.length} AGENTS
              <span style={{ color: "var(--txt-dim)", letterSpacing: 0, fontWeight: 400 }}> · extended demo roster</span>
            </span>
            <IllustrativeBadge />
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
