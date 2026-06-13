//! The self-contained dashboard page. `/*__BR_DATA__*/` is replaced with the
//! value-free JSON inventory at serve time. No external assets — works offline.

pub const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>blastradius — blast radius</title>
<style>
  :root{
    --bg:#0a0e14; --panel:#11161f; --panel2:#0d1219; --border:#1d2530;
    --text:#cdd6e3; --muted:#6c7889; --dim:#454f5e;
    --exposed:#ff4d57; --notable:#ffb020; --info:#3a4453;
    --accent:#36c5f0; --good:#2ec27e;
    --mono:ui-monospace,"SF Mono",Menlo,Consolas,"Liberation Mono",monospace;
    --sans:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;
  }
  *{box-sizing:border-box}
  body{margin:0;background:radial-gradient(1200px 600px at 50% -10%,#141b26 0,var(--bg) 60%);
    color:var(--text);font-family:var(--sans);line-height:1.5;-webkit-font-smoothing:antialiased}
  .wrap{max-width:1120px;margin:0 auto;padding:32px 24px 80px}
  a{color:var(--accent)}
  header{display:flex;align-items:flex-start;justify-content:space-between;gap:24px;flex-wrap:wrap;
    border-bottom:1px solid var(--border);padding-bottom:22px;margin-bottom:28px}
  .brand{font-family:var(--mono);font-weight:700;font-size:13px;letter-spacing:.34em;color:var(--muted)}
  h1{margin:.18em 0 .1em;font-size:30px;letter-spacing:-.02em;
    background:linear-gradient(92deg,#fff 10%,#ff8a5c 55%,var(--exposed) 100%);
    -webkit-background-clip:text;background-clip:text;color:transparent}
  .sub{color:var(--muted);font-size:14px;max-width:48ch}
  .meta{font-family:var(--mono);font-size:12px;color:var(--muted);text-align:right;white-space:nowrap}
  .meta b{color:var(--text);font-weight:600}
  .pill{display:inline-flex;align-items:center;gap:7px;font-family:var(--mono);font-size:12px;
    padding:6px 12px;border-radius:999px;border:1px solid var(--border);margin-top:10px}
  .pill .dot{width:8px;height:8px;border-radius:50%}
  .pill.bad{border-color:#5a2330;background:#1c1014;color:#ff9aa0}
  .pill.ok{border-color:#1f4636;background:#0e1a14;color:#7ee0ad}
  .grid-stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin-bottom:30px}
  .stat{background:var(--panel);border:1px solid var(--border);border-radius:12px;padding:16px 18px}
  .stat .n{font-size:30px;font-weight:700;font-family:var(--mono);line-height:1}
  .stat .l{font-size:11px;letter-spacing:.16em;text-transform:uppercase;color:var(--muted);margin-top:8px}
  .stat.exposed .n{color:var(--exposed)} .stat.notable .n{color:var(--notable)}
  .stat .ex{color:var(--exposed)}
  .panel{background:linear-gradient(180deg,var(--panel) 0,var(--panel2) 100%);
    border:1px solid var(--border);border-radius:16px;padding:24px;margin-bottom:24px}
  .panel h2{margin:0 0 4px;font-size:13px;letter-spacing:.2em;text-transform:uppercase;color:var(--muted)}
  .panel .hint{color:var(--dim);font-size:12px;margin-bottom:18px;font-family:var(--mono)}
  .radial{display:grid;grid-template-columns:minmax(0,1fr) 320px;gap:20px;align-items:center}
  @media(max-width:820px){.radial{grid-template-columns:1fr}}
  svg{width:100%;height:auto;display:block}
  .node{cursor:pointer;transition:transform .12s}
  .node:hover circle{stroke:#fff}
  .detail{background:var(--panel2);border:1px solid var(--border);border-radius:12px;padding:18px;min-height:170px}
  .detail .dt{font-family:var(--mono);font-size:11px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted)}
  .detail .dtitle{font-size:16px;font-weight:600;margin:8px 0}
  .detail .dsum{font-size:13.5px;color:var(--muted)}
  .sevtag{display:inline-block;font-family:var(--mono);font-size:10.5px;letter-spacing:.08em;
    text-transform:uppercase;padding:3px 8px;border-radius:6px;font-weight:600}
  .sev-exposed{background:#2a1316;color:var(--exposed)} .sev-notable{background:#2a2110;color:var(--notable)}
  .sev-info{background:#161b22;color:var(--muted)}
  .sev-critical{background:#2a1316;color:#ff4d57} .sev-high{background:#2a1316;color:#ff7a4d}
  .sev-medium{background:#2a2110;color:var(--notable)} .sev-low{background:#161b22;color:var(--accent)}
  .scen{border:1px solid var(--border);border-left:3px solid var(--dim);border-radius:12px;
    padding:18px 20px;margin-bottom:14px;background:var(--panel2)}
  .scen.s-critical,.scen.s-high{border-left-color:var(--exposed)}
  .scen.s-medium{border-left-color:var(--notable)} .scen.s-low{border-left-color:var(--accent)}
  .scen .top{display:flex;align-items:center;gap:12px;margin-bottom:8px}
  .scen h3{margin:0;font-size:17px}
  .scen .narr{color:var(--text);font-size:14px;margin:6px 0 14px}
  .chain{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:12px 0}
  .step{font-family:var(--mono);font-size:12px;background:#0c1118;border:1px solid var(--border);
    border-radius:7px;padding:6px 10px;position:relative}
  .arrow{color:var(--dim);font-size:14px}
  .chips{display:flex;flex-wrap:wrap;gap:6px;margin:10px 0}
  .chip{font-family:var(--mono);font-size:11px;color:var(--accent);background:#0a1620;
    border:1px solid #143042;border-radius:999px;padding:4px 10px}
  .impact{font-size:13px;color:#ffb9a3;background:#1a0f0c;border:1px solid #3a1d14;
    border-radius:8px;padding:10px 12px;margin:12px 0}
  .contain{list-style:none;padding:0;margin:8px 0 0}
  .contain li{font-size:13px;color:var(--muted);padding:4px 0 4px 22px;position:relative}
  .contain li::before{content:"⛨";position:absolute;left:0;color:var(--good)}
  .label{font-family:var(--mono);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;
    color:var(--dim);margin:14px 0 6px}
  table{width:100%;border-collapse:collapse;font-size:13.5px}
  .cls{font-family:var(--mono);font-size:11px;letter-spacing:.16em;color:var(--muted);
    padding:18px 0 6px;text-transform:uppercase}
  tr.f{border-top:1px solid var(--border)}
  tr.f td{padding:9px 8px;vertical-align:top}
  .ftitle{font-weight:500} .fsum{color:var(--muted);font-size:12.5px}
  td.sevcell{width:90px} td.confcell{width:96px;color:var(--dim);font-family:var(--mono);font-size:11px;text-align:right}
  .notice{font-size:12.5px;color:var(--muted);background:var(--panel2);border:1px dashed var(--border);
    border-radius:10px;padding:14px 16px;margin-bottom:24px}
  .notice b{color:var(--text)}
  footer{color:var(--dim);font-size:12px;font-family:var(--mono);border-top:1px solid var(--border);
    padding-top:18px;margin-top:30px}
  .err{color:var(--exposed)}
</style>
</head>
<body>
<div class="wrap">
  <header>
    <div>
      <div class="brand">● BLASTRADIUS</div>
      <h1 id="h1">blast radius</h1>
      <div class="sub">What a coding agent running as you can reach on this machine — and how that reach could be chained.</div>
      <div id="verdict"></div>
    </div>
    <div class="meta" id="meta"></div>
  </header>

  <div class="grid-stats" id="stats"></div>

  <div class="panel">
    <h2>Reachable surface</h2>
    <div class="hint">credentials, identities & routes reachable from this process — hover a node</div>
    <div class="radial">
      <div id="radial"></div>
      <div class="detail" id="detail">
        <div class="dt">select a node</div>
        <div class="dtitle">Hover the map</div>
        <div class="dsum">Each node is a reachable credential store, identity, repo, or egress route. Distance from the core reflects severity.</div>
      </div>
    </div>
  </div>

  <div class="panel" id="aipanel"></div>

  <div class="panel">
    <h2>Full inventory</h2>
    <div class="hint">every probe result, value-free</div>
    <table id="findings"></table>
  </div>

  <footer id="footer"></footer>
</div>

<script id="br-data" type="application/json">/*__BR_DATA__*/</script>
<script>
const D = JSON.parse(document.getElementById('br-data').textContent);
const esc = s => String(s==null?'':s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const sevClass = s => 'sev-'+s;

// ---- header / meta ----
document.getElementById('meta').innerHTML =
  `<b>${esc(D.tool.name)}</b> v${esc(D.tool.version)}<br>${esc(D.platform)}<br>${esc(D.generated)}`;
const v = D.verdict || 'unknown';
const sandboxed = v.indexOf('not') === -1 && v !== 'unknown';
document.getElementById('verdict').innerHTML =
  `<span class="pill ${sandboxed?'ok':'bad'}"><span class="dot" style="background:${sandboxed?'var(--good)':'var(--exposed)'}"></span>`+
  `host: ${esc(v)}</span>`;

// ---- stat tiles ----
const stats = [];
stats.push(`<div class="stat exposed"><div class="n">${D.stats.exposed}</div><div class="l">exposed</div></div>`);
stats.push(`<div class="stat notable"><div class="n">${D.stats.notable}</div><div class="l">notable</div></div>`);
for(const c of D.stats.classes){
  stats.push(`<div class="stat"><div class="n">${c.count}${c.exposed?` <span class="ex" style="font-size:15px">▲${c.exposed}</span>`:''}</div><div class="l">${esc(c.label)}</div></div>`);
}
document.getElementById('stats').innerHTML = stats.join('');

// ---- radial map ----
(function(){
  const reach = D.findings.filter(f=>f.reachable);
  const W=520,H=520,cx=W/2,cy=H/2;
  const exposed = reach.filter(f=>f.severity==='exposed');
  const notable = reach.filter(f=>f.severity==='notable');
  let svg = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="blast radius map">`;
  // rings
  [120,180,235].forEach((r,i)=>{svg+=`<circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="var(--border)" stroke-dasharray="2 6"/>`;});
  const nodes=[];
  const place=(list,radius)=>{
    const n=list.length||1;
    list.forEach((f,i)=>{
      const a=(-Math.PI/2)+(i/n)*Math.PI*2;
      const x=cx+Math.cos(a)*radius, y=cy+Math.sin(a)*radius;
      nodes.push({f,x,y});
    });
  };
  place(exposed,135); place(notable,222);
  // connectors
  nodes.forEach(nd=>{
    const col = nd.f.severity==='exposed'?'var(--exposed)':'var(--notable)';
    svg+=`<line x1="${cx}" y1="${cy}" x2="${nd.x}" y2="${nd.y}" stroke="${col}" stroke-opacity="0.22"/>`;
  });
  // core
  svg+=`<circle cx="${cx}" cy="${cy}" r="46" fill="#160d12" stroke="var(--exposed)" stroke-opacity="0.5"/>`;
  svg+=`<circle cx="${cx}" cy="${cy}" r="46" fill="none" stroke="var(--exposed)"><animate attributeName="r" values="46;62;46" dur="3s" repeatCount="indefinite"/><animate attributeName="stroke-opacity" values="0.5;0;0.5" dur="3s" repeatCount="indefinite"/></circle>`;
  svg+=`<text x="${cx}" y="${cy-4}" text-anchor="middle" fill="#fff" font-size="12" font-family="var(--mono)">agent</text>`;
  svg+=`<text x="${cx}" y="${cy+12}" text-anchor="middle" fill="var(--muted)" font-size="10" font-family="var(--mono)">runs as you</text>`;
  // nodes
  nodes.forEach((nd,i)=>{
    const col = nd.f.severity==='exposed'?'var(--exposed)':'var(--notable)';
    const r = nd.f.severity==='exposed'?9:6.5;
    svg+=`<g class="node" data-i="${i}" tabindex="0">`+
      `<circle cx="${nd.x}" cy="${nd.y}" r="${r}" fill="${col}" fill-opacity="0.85" stroke="${col}" stroke-width="1.5"/>`+
      `</g>`;
  });
  if(!nodes.length){
    svg+=`<text x="${cx}" y="${cy+90}" text-anchor="middle" fill="var(--good)" font-size="13" font-family="var(--mono)">no reachable surface 🎉</text>`;
  }
  svg+=`</svg>`;
  document.getElementById('radial').innerHTML=svg;
  const det=document.getElementById('detail');
  const show=nd=>{det.innerHTML=`<div class="dt">${esc(nd.f.classLabel)} · ${esc(nd.f.scope)}</div>`+
    `<div class="dtitle">${esc(nd.f.title)}</div>`+
    `<span class="sevtag ${sevClass(nd.f.severity)}">${esc(nd.f.severity)}</span>`+
    `<div class="dsum" style="margin-top:10px">${esc(nd.f.summary)}</div>`;};
  document.querySelectorAll('.node').forEach(g=>{
    const nd=nodes[+g.dataset.i];
    g.addEventListener('mouseenter',()=>show(nd));
    g.addEventListener('focus',()=>show(nd));
    g.addEventListener('click',()=>show(nd));
  });
})();

// ---- AI scenarios ----
(function(){
  const p=document.getElementById('aipanel'); const ai=D.ai||{};
  if(ai.error){
    p.innerHTML=`<h2>Attack scenarios</h2><div class="hint">AI analysis</div>`+
      `<div class="notice err">AI analysis unavailable: ${esc(ai.error)}</div>`;
    return;
  }
  if(!ai.enabled){
    p.innerHTML=`<h2>Attack scenarios</h2><div class="hint">AI analysis (opt-in)</div>`+
      `<div class="notice">Re-run with <b>blastradius dashboard --ai</b> to generate attack-path narratives from the reachable surface. This sends the value-free inventory (severities, credential classes, names, counts) to the OpenAI API — never secret values.</div>`;
    return;
  }
  let h=`<h2>Attack scenarios</h2><div class="hint">AI-generated blast-radius narratives · ${esc(ai.model)} · grounded in the reachable surface above</div>`;
  if(ai.overall) h+=`<div class="notice"><b>Overall:</b> ${esc(ai.overall)}</div>`;
  for(const s of (ai.scenarios||[])){
    const sv=(s.severity||'medium').toLowerCase();
    h+=`<div class="scen s-${sv}">`+
       `<div class="top"><span class="sevtag ${sevClass(sv)}">${esc(sv)}</span><h3>${esc(s.title)}</h3></div>`+
       (s.narrative?`<div class="narr">${esc(s.narrative)}</div>`:'');
    if(s.attack_path&&s.attack_path.length){
      h+=`<div class="label">attack path</div><div class="chain">`;
      s.attack_path.forEach((st,i)=>{h+=(i?`<span class="arrow">→</span>`:'')+`<span class="step">${esc(st)}</span>`;});
      h+=`</div>`;
    }
    if(s.reachable_used&&s.reachable_used.length){
      h+=`<div class="label">relies on</div><div class="chips">`+
         s.reachable_used.map(r=>`<span class="chip">${esc(r)}</span>`).join('')+`</div>`;
    }
    if(s.impact) h+=`<div class="impact"><b>Impact:</b> ${esc(s.impact)}</div>`;
    if(s.containment&&s.containment.length){
      h+=`<div class="label">containment</div><ul class="contain">`+
         s.containment.map(c=>`<li>${esc(c)}</li>`).join('')+`</ul>`;
    }
    h+=`</div>`;
  }
  p.innerHTML=h;
})();

// ---- inventory table ----
(function(){
  const rows=[]; let last=null;
  const order={exposed:0,notable:1,info:2};
  const fs=[...D.findings].sort((a,b)=>(a.classLabel>b.classLabel?1:a.classLabel<b.classLabel?-1:0)||(order[a.severity]-order[b.severity]));
  for(const f of fs){
    if(f.classLabel!==last){rows.push(`<tr><td class="cls" colspan="3">${esc(f.classLabel)}</td></tr>`);last=f.classLabel;}
    rows.push(`<tr class="f"><td class="sevcell"><span class="sevtag ${sevClass(f.severity)}">${esc(f.severity)}</span></td>`+
      `<td><div class="ftitle">${esc(f.title)}</div><div class="fsum">${esc(f.summary)}</div></td>`+
      `<td class="confcell">${esc(f.confidence)}</td></tr>`);
  }
  document.getElementById('findings').innerHTML=rows.join('');
})();

document.getElementById('footer').innerHTML =
  `local only · no telemetry · secret values never leave this machine`+
  (D.ai&&D.ai.enabled?` · AI sent the value-free inventory to OpenAI (${esc(D.ai.model||'')})`:'');
</script>
</body>
</html>"##;
