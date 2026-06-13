# Releasing blastradius

Three distribution channels, **one set of artifacts**. A pushed `vX.Y.Z` tag
builds four release tarballs + `SHA256SUMS` and feeds all three:

| Channel | User runs | How it gets the binary |
| --- | --- | --- |
| npm / npx | `npx @amlalabs/blastradius …` | `npm/bin/blastradius.js` downloads the tarball on first **explicit** run, checksum-verifies it, caches it. No install hook. |
| cargo-binstall | `cargo binstall blastradius` | `[package.metadata.binstall]` in `Cargo.toml` points at the same release tarball. |
| Manual | download from the Release page | Same `.tar.gz` + `SHA256SUMS`. |

`cargo install --git …` / source build remain available and need none of this.

## One-time setup

Publishing uses **npm trusted publishing (OIDC)** — no long-lived `NPM_TOKEN`
secret, and every publish gets a **provenance attestation** (signed link from the
tarball back to this repo + workflow run, shown as a badge on npmjs.com).

1. **Git remote** points at `github.com/amlalabs/blastradius` (the shim and the
   binstall metadata both hardcode this path).
2. **npm org.** The `amlalabs` npm org must exist and own the
   `@amlalabs/blastradius` name.
3. **Bootstrap the name (one time).** A Trusted Publisher can only be attached to
   a package that already exists, so claim the name with the *first* publish
   manually — from a laptop logged into npm (`npm whoami`):

   ```sh
   cd npm
   cp ../README.md ../LICENSE .          # files CI normally stages
   npm publish --access public           # uses your interactive npm login (2FA)
   rm -f README.md LICENSE
   ```

   (Equivalently, do this one publish with a short-lived granular token, then
   revoke it.)
4. **Configure the Trusted Publisher.** On npmjs.com →
   `@amlalabs/blastradius` → *Settings* → *Trusted Publisher* → GitHub Actions:
   - Organization/User: `amlalabs`
   - Repository: `blastradius`
   - Workflow filename: `release.yml`

   From then on the `publish-npm` job authenticates via OIDC with no secret.
5. Nothing else — `GITHUB_TOKEN` is provided automatically for the Release.

> If a future npm adds *pending* trusted publishers (configure-before-first-
> publish, like PyPI), step 3 becomes unnecessary — just configure step 4 and let
> the workflow do the first publish.

## Cutting a release

Versions are checked against the tag in CI and the build fails on mismatch, so
keep all three in lockstep:

```sh
V=0.1.0

# 1. Bump both manifests to the SAME version.
#    - Cargo.toml      -> version = "0.1.0"
#    - npm/package.json -> "version": "0.1.0"
#    (then `cargo build` once so Cargo.lock updates, and commit it)

git add Cargo.toml Cargo.lock npm/package.json
git commit -m "release: v$V"
git push origin main

# 2. Tag and push — this is what triggers .github/workflows/release.yml
git tag "v$V"
git push origin "v$V"
```

The workflow then:

1. **validate-tag** — strict `vX.Y.Z` check, asserts the ref is a real tag (not
   a same-named branch), asserts `Cargo.toml` **and** `npm/package.json` both
   match the tag, and runs the shim self-test — all once, before any build.
2. **build** (matrix) — cross-builds `x86_64`/`aarch64` Linux **musl** (static,
   via `cross`) and `x86_64`/`aarch64` macOS (native), packages each as
   `blastradius-<target>.tar.gz`.
3. **release** — assembles a single `SHA256SUMS` and creates/updates the GitHub
   Release with all tarballs.
4. **publish-npm** — `npm publish --access=public --provenance` via OIDC. Runs
   **after** the Release exists, so the package is never live before the assets
   it points at.

This mirrors the conventions of the kalahari mirror's `release.yml` (OIDC +
provenance, Node 24 for npm ≥ 11.5.1, `concurrency` guard, minimal per-job
permissions, idempotent re-runs). The one intentional difference is job order:
kalahari publishes npm before the Release because its binary ships *inside* the
tarball; blastradius's shim downloads the binary *from* the Release, so the
Release must come first.

## Supported targets

The set is defined in two places that must agree: the workflow matrix and the
shim's `detectTarget()` (`npm/bin/blastradius.js`). Currently:

- `x86_64-unknown-linux-musl`  — x86-64 Linux
- `aarch64-unknown-linux-musl` — arm64 Linux
- `aarch64-apple-darwin`       — arm64 macOS (Apple Silicon)

x86-64 macOS and Windows are intentionally not built; those platforms get a
clean "unsupported platform/arch" error from the shim. To add a target, add a
matrix row **and** a `detectTarget()` branch.

## Verifying a release

```sh
# npx path (fresh machine / clean cache)
rm -rf "${XDG_CACHE_HOME:-$HOME/.cache}/blastradius"
npx -y @amlalabs/blastradius@$V --help     # downloads + checksum-verifies, then runs

# checksums
curl -fsSLO https://github.com/amlalabs/blastradius/releases/download/v$V/SHA256SUMS
curl -fsSLO https://github.com/amlalabs/blastradius/releases/download/v$V/blastradius-x86_64-unknown-linux-musl.tar.gz
shasum -a 256 -c SHA256SUMS --ignore-missing
```

## Re-running a failed release

The workflow is idempotent — safe to re-run from the Actions tab, or via
`workflow_dispatch` without re-tagging:

```sh
gh workflow run release.yml --ref "refs/tags/v$V" -f tag="v$V"
```

The Release step `--clobber`s existing assets, and `publish-npm` pre-checks the
registry and skips the publish if that version is already live (npm publishes
are immutable). Dispatch **against the tag** (`--ref refs/tags/…`), not a
branch — the workflow refuses otherwise, because npm provenance signs the ref.
