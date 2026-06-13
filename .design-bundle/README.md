# Dashboard design sources

The web dashboard (`blastradius dashboard`) is the storytelling experience
originally mocked up in Claude Design. **`src/dashboard/page.rs` is generated
from this directory** — do not hand-edit `page.rs`.

## Layout

- `project/` — the editable design sources (the source of truth for the page):
  - `Blast Radius.html` — document shell: design tokens, keyframes, `.grain`
    overlay, the Google Fonts (v1 API) `<link>`, and the React/ReactDOM/Babel
    CDN `<script>` tags.
  - `data.js` — `window.BR` model. Reads the live, value-free scan JSON injected
    at `/*__BR_DATA__*/` (the reachable-surface *denominator*); the per-session
    scoring (§23) and retro-hazard (§24) data are frozen illustrative fixtures.
  - `viz.jsx`, `narrative.jsx`, `dashboard.jsx`, `retro.jsx`, `app.jsx` — the
    React components (transpiled in-browser by Babel-standalone).
- `gen_page.py` — assembles the single HTML document and writes
  `src/dashboard/page.rs` (a `pub const PAGE` raw string with the
  `/*__BR_DATA__*/` injection marker `mod.rs::render_html` replaces and sweeps).

## Regenerating the page

After editing anything in `project/`:

```sh
python3 .design-bundle/gen_page.py
cargo test            # dashboard tests assert the rings, labels, and value-free sweep
```

## Invariants the sources must keep

- **Value-free** — paths / command shapes / hosts / counts / names / lengths
  only, never secret values. The final HTML is run through the Layer-2 redaction
  sweep before it is served, so secret values never leave the machine.
- Fonts load via the Google Fonts **v1 API** (`css?family=…:400,500`) — the
  `css2` form (`:wght@…`) contains `:…@` which the credential-shape sweep would
  redact, breaking the link.
- The §23/§24 sections are post-MVP fixtures and must stay visibly labeled
  `illustrative — post-MVP, not from your scan`; the `REACHABILITY, NOT
  EXPLOITATION` disclaimer stays verbatim.

> Ignored (see root `.gitignore`): `chats/`, `project/screenshots/`,
> `project/.thumbnail`, `*_workflow.js` —
> reference/scratch, not build inputs.
