#!/usr/bin/env python3
"""Generate src/dashboard/page.rs from the edited design sources.

This assembles the 8-section scrollytelling experience described in
.design-bundle/project/ into ONE bundled HTML document (which still references
React/Babel + webfonts from a CDN at runtime) and writes it as the
`pub const PAGE` raw string inside src/dashboard/page.rs.

EDIT THE DESIGN SOURCES, NOT page.rs. The Rust file is generated; any manual
edit to it will be clobbered the next time this script runs.

Assembly contract (must match src/dashboard/mod.rs::render_html and the
WINDOW.BR / LIVE-DATA contracts):
  - <head> meta + title + Google Fonts v1 link + the VERBATIM <style> block
    from "Blast Radius.html".
  - <body>: div.grain, div#root, the three unpkg CDN <script src> tags for
    react / react-dom / babel-standalone (crossorigin, NO integrity/SRI), then
    <script id="br-data" type="application/json"> containing exactly the marker
    /*__BR_DATA__*/ (render_html .replace()s this once), then the full data.js
    inlined in a plain <script>, then one <script type="text/babel"> per
    jsx file in order: viz, narrative, dashboard, retro, app.
"""

import os
import sys
import re

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.join(HERE, "project")
REPO = os.path.dirname(HERE)
OUT = os.path.join(REPO, "src", "dashboard", "page.rs")

DATA_MARKER = "/*__BR_DATA__*/"
RUST_RAW_TERMINATOR = '"##'


def read(name):
    with open(os.path.join(PROJECT, name), "r", encoding="utf-8") as fh:
        return fh.read()


def extract_style_block(html):
    """Return the entire <style>...</style> block (inclusive) from the head."""
    m = re.search(r"<style>.*?</style>", html, re.DOTALL | re.IGNORECASE)
    if not m:
        sys.exit("FAIL: no <style> block found in Blast Radius.html")
    return m.group(0)


def main():
    blast_html = read("Blast Radius.html")
    data_js = read("data.js")
    viz = read("viz.jsx")
    narrative = read("narrative.jsx")
    dashboard = read("dashboard.jsx")
    retro = read("retro.jsx")
    app = read("app.jsx")

    style_block = extract_style_block(blast_html)

    # The CDN script tags, copied verbatim from the design (crossorigin, NO SRI).
    cdn_scripts = (
        '  <script src="https://unpkg.com/react@18.3.1/umd/react.development.js" crossorigin="anonymous"></script>\n'
        '  <script src="https://unpkg.com/react-dom@18.3.1/umd/react-dom.development.js" crossorigin="anonymous"></script>\n'
        '  <script src="https://unpkg.com/@babel/standalone@7.29.0/babel.min.js" crossorigin="anonymous"></script>\n'
    )

    fonts_link = (
        '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
        '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
        '<link href="https://fonts.googleapis.com/css?family=Space+Grotesk:400,500,600,700|'
        'JetBrains+Mono:400,500,700&display=swap" rel="stylesheet">'
    )

    html = (
        "<!DOCTYPE html>\n"
        '<html lang="en">\n'
        "<head>\n"
        '<meta charset="UTF-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1.0">\n'
        "<title>blastradius — what your coding agent can reach</title>\n"
        f"{fonts_link}\n"
        f"{style_block}\n"
        "</head>\n"
        "<body>\n"
        '  <div class="grain"></div>\n'
        '  <div id="root"></div>\n\n'
        f"{cdn_scripts}\n"
        f'  <script id="br-data" type="application/json">{DATA_MARKER}</script>\n\n'
        f'  <script>\n{data_js}\n  </script>\n\n'
        f'  <script type="text/babel">\n{viz}\n  </script>\n'
        f'  <script type="text/babel">\n{narrative}\n  </script>\n'
        f'  <script type="text/babel">\n{dashboard}\n  </script>\n'
        f'  <script type="text/babel">\n{retro}\n  </script>\n'
        f'  <script type="text/babel">\n{app}\n  </script>\n'
        "</body>\n"
        "</html>\n"
    )

    # --- Invariant checks before we commit to the raw string ---------------
    marker_count = html.count(DATA_MARKER)
    if marker_count != 1:
        sys.exit(
            f"FAIL: data marker {DATA_MARKER!r} appears {marker_count} times, expected exactly 1"
        )

    if RUST_RAW_TERMINATOR in html:
        sys.exit(
            f"FAIL: assembled HTML contains the Rust raw-string terminator {RUST_RAW_TERMINATOR!r}; "
            "cannot embed safely in r##\"...\"##"
        )

    header = (
        "//! Cinematic blast-radius dashboard page (served by `blastradius dashboard`).\n"
        "//!\n"
        "//! GENERATED FILE — do not edit by hand. This is assembled from the design\n"
        "//! sources in `.design-bundle/project/` by `.design-bundle/gen_page.py`.\n"
        "//! Edit those sources (Blast Radius.html, data.js, viz/narrative/dashboard/\n"
        "//! retro/app.jsx) and re-run the generator instead.\n"
        "//!\n"
        "//! The page loads its UI runtime (React 18 + ReactDOM + Babel-standalone)\n"
        "//! and webfonts from a CDN; those requests carry no scan data. The live,\n"
        "//! value-free finding inventory is injected at the data marker in the\n"
        "//! #br-data script tag by `render_html`, and the whole document is run\n"
        "//! through the Layer-2 redaction sweep before any byte is written to a\n"
        "//! socket, so secret values never leave the machine.\n"
        "\n"
    )

    rust = f'{header}pub const PAGE: &str = r##"{html}"##;\n'

    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write(rust)

    print(f"wrote {OUT} ({len(rust.encode('utf-8'))} bytes)")


if __name__ == "__main__":
    main()
