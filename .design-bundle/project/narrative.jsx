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
