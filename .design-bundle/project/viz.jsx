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
