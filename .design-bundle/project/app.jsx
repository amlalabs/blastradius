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
  return (
    <div>
      <Narrative />
      <div style={{ borderTop: "1px solid var(--line)",
        background: "linear-gradient(180deg, #07090d, #090c12)" }}>
        <Dashboard />
      </div>
      <RetroSection />
      <Outro />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
