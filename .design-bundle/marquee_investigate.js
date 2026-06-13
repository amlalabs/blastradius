export const meta = {
  name: 'investigate-ring-overflow',
  description: 'READ-ONLY investigation of the RadiusScene ring-caption overflow (ring 06 "The whole machine" runs off-screen) and the best auto-scroll / vertical-marquee fix; converge on ONE recommendation. No code is edited.',
  phases: [
    { title: 'Investigate', detail: 'parallel read-only: diagnose layout + two candidate fixes' },
    { title: 'Recommend', detail: 'synthesize one concrete recommended fix' },
  ],
}

const REPO = '/home/souvik/Projects/amlalabs/blastradius'
const DB = REPO + '/.design-bundle/project'

const RO = 'STRICTLY READ-ONLY. Use only Read, Grep, Glob. Do NOT use Edit/Write/NotebookEdit or modify any file. This is an investigation; output a recommendation, do not apply it.'

const CTX = `
PROBLEM: In the blastradius storytelling dashboard, the scroll-driven "expanding radius" section (RadiusScene) shows, on the right, a stepped caption per ring (01/06 .. 06/06) with a list of finding "chips" for that ring. The chips are now driven by LIVE scan data (BR.LIVE_RINGS), so the count is variable. Ring 06 "The whole machine" (the host ring) can have MANY findings (the user reports ~8: privilege escalation, post-escalation blast radius, docker.sock, process environ, local services, /proc cmdline secrets, ptrace, etc.). The right column overflows the sticky 100vh viewport and the items run off the bottom of the screen / past the end.

DESIRED: make the chip list not overflow — the user suggested an auto-scroll / vertical marquee (the list gently scrolls vertically so all items are seen without going past the screen). It must look intentional and on-brand (cinematic situation-room aesthetic), respect prefers-reduced-motion, and handle a VARIABLE item count (0 chips, a few, or many) gracefully — short lists should NOT scroll, only overflowing ones should.

SOURCE FILES (design sources; the served page src/dashboard/page.rs is generated from these by .design-bundle/gen_page.py — recommend edits to the SOURCES, not page.rs):
- ${DB}/narrative.jsx — RadiusScene component: the sticky 560vh section, the two-column 100vh sticky stage, the RING_COPY captions, and the chips list rendered from (BR.LIVE_RINGS[i] || BR.RINGS[i]).findings. THIS is the component to fix.
- ${DB}/Blast Radius.html — the <style> block: design tokens (--hot/--warn/--safe/--txt-mid etc.), keyframes (pulseRing/breathe/dashFlow/fadeUp), prefers-reduced-motion rule, .mono/.sev-* classes.
- ${DB}/viz.jsx — RadiusViz (the SVG; left column) for context on the sticky stage sizing.
`

phase('Investigate')
const findings = await parallel([
  () => agent(
    RO + '\n' + CTX + '\n\nYOUR TASK (DIAGNOSE). Read ' + DB + '/narrative.jsx (RadiusScene in full) and the relevant CSS in ' + DB + '/Blast Radius.html. Describe PRECISELY: (1) the exact JSX/DOM structure of the right column — the wrapper, the per-ring caption block (number, h3, blurb, the chips flex container), and the progress rail; quote the relevant lines. (2) The CSS/inline-style sizing: what gives the column its height, is the stage sticky 100vh, does the caption block have any max-height/overflow, and WHY do many chips overflow off-screen (no clamp, flex-wrap growing unbounded inside a 100vh sticky panel). (3) How the chips are produced and that the count is now LIVE/variable (BR.LIVE_RINGS). (4) The exact container element a fix should target (selector/inline-style location) and the realistic max chip count + approximate height budget available in the sticky stage. Be concrete with line numbers so a fix can be written against it.',
    { label: 'diagnose', phase: 'Investigate' }
  ),
  () => agent(
    RO + '\n' + CTX + '\n\nYOUR TASK (APPROACH A — CSS vertical marquee). Design a CSS-driven vertical auto-scroll marquee for the overflowing chip list, on-brand and robust. Specify: the exact wrapper (fixed/max height region with overflow hidden), the seamless-loop technique (duplicate the chip set vs translateY ping-pong), a @keyframes (translateY) with duration SCALED to content height/count so it reads gently (not too fast), pause-on-hover, a top/bottom fade mask (linear-gradient mask-image in the situation-room palette), and the prefers-reduced-motion fallback (no animation → a plain scrollable/clamped region). Critically address: it must only animate when content OVERFLOWS the height budget (short lists stay static) — describe how to gate that (measure vs a CSS max-height; or a small JS check toggling a class). Give the concrete CSS (using the existing tokens) + the minimal JSX change in RadiusScene (a wrapper div + conditional class). Note pros/cons and the per-ring-switch reset concern (the caption swaps as you scroll rings; the marquee must restart cleanly).',
    { label: 'approach-css-marquee', phase: 'Investigate' }
  ),
  () => agent(
    RO + '\n' + CTX + '\n\nYOUR TASK (APPROACH B — bounded scroll region, alternatives). Design at least two NON-marquee alternatives and weigh them: (B1) a fixed max-height scrollable container (overflow-y:auto) with top/bottom fade masks and a subtle scrollbar — simplest, accessible, no motion, user can scroll; (B2) the chip reveal tied to the existing scroll progress (the section is already scroll-driven over 560vh — distribute/scroll the chips as the user scrolls within that ring band) so no autoplay is needed; (B3) graceful cap — show the top N highest-severity chips + a "+K more" pill, optionally with a compact 2-column chip grid to fit more. For each: concrete impl sketch against RadiusScene (file/lines), how it handles variable/zero counts, reduced-motion, and on-brand feel. Recommend which non-marquee option is best and when it beats a marquee. Read ' + DB + '/narrative.jsx + the CSS first.',
    { label: 'approach-alternatives', phase: 'Investigate' }
  ),
])

phase('Recommend')
const rec = await agent(
  CTX + '\n\nThree read-only investigations (JSON/text): a layout diagnosis and two fix-approach explorations:\n\n' + JSON.stringify(findings.filter(Boolean), null, 2) + '\n\nProduce ONE decisive recommended fix for the ring-06 chip overflow. Requirements:\n- State the ROOT CAUSE in one line (cite narrative.jsx line(s)).\n- Pick ONE primary approach (the user asked for auto-scroll/vertical-marquee, so favor a marquee unless an alternative is clearly better — justify) and give the CONCRETE, ready-to-apply change: exact file (' + DB + '/narrative.jsx and, if needed, ' + DB + '/Blast Radius.html <style>), the JSX wrapper + class to add, the CSS/keyframes to add (using existing design tokens), how it is gated to ONLY animate on overflow, the prefers-reduced-motion fallback, and the clean reset when the active ring switches. Keep it minimal and on-brand.\n- Note that the page is generated: after editing the sources, run python3 ' + REPO + '/.design-bundle/gen_page.py and cargo test.\n- Include a one-paragraph FALLBACK (the best non-marquee option) in case the marquee feels wrong in review.\n- Do NOT apply anything — this is the recommendation a single editor will implement serially.',
  { label: 'recommend', phase: 'Recommend' }
)

return { recommendation: rec, investigations: findings.filter(Boolean) }
