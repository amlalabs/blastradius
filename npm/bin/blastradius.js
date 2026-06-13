#!/usr/bin/env node
// Tiny shim: on EXPLICIT invocation only, detect OS/arch, download-or-cache the
// release binary, checksum-verify it, and exec it. Contains no scanning logic.
//
// There is intentionally NO `postinstall`/`install` lifecycle hook. Shipping a
// fetch-and-run-on-install package is the exact supply-chain pattern blastradius
// exists to surface — so we don't do it ourselves.

const fs = require("fs");
const os = require("os");
const path = require("path");
const https = require("https");
const crypto = require("crypto");
const zlib = require("zlib");
const { spawnSync } = require("child_process");

const REPO = "amlalabs/blastradius";
const VERSION = require("../package.json").version;
const RELEASE_TAG = `v${VERSION}`;
const BASE = `https://github.com/${REPO}/releases/download/${RELEASE_TAG}`;
const DOWNLOAD_TIMEOUT_MS = 30_000;
const MAX_REDIRECTS = 5;
const MAX_ARCHIVE_BYTES = 64 * 1024 * 1024;
const MAX_TAR_BYTES = 128 * 1024 * 1024;
const MAX_SUMS_BYTES = 1024 * 1024;
const MAX_CACHE_CHECKSUM_BYTES = 128;

function detectTarget() {
  const platform = os.platform();
  const arch = os.arch();
  if (platform === "darwin") {
    if (arch === "arm64") return "aarch64-apple-darwin";
    if (arch === "x64") return "x86_64-apple-darwin";
  } else if (platform === "linux") {
    if (arch === "arm64") return "aarch64-unknown-linux-musl";
    if (arch === "x64") return "x86_64-unknown-linux-musl";
  }
  throw new Error(`unsupported platform/arch: ${platform}/${arch}`);
}

function cacheDir() {
  const base =
    process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(base, "blastradius", VERSION);
}

function normalizeHttpsUrl(input, baseUrl) {
  const parsed = new URL(input, baseUrl);
  if (parsed.protocol !== "https:") {
    throw new Error(`refusing non-HTTPS download URL: ${parsed.toString()}`);
  }
  return parsed.toString();
}

function get(url, maxBytes, redirects = 0) {
  const safeUrl = normalizeHttpsUrl(url);
  return new Promise((resolve, reject) => {
    let settled = false;
    const fail = (err) => {
      if (!settled) {
        settled = true;
        reject(err);
      }
    };
    const req = https
      .get(
        safeUrl,
        { headers: { "User-Agent": `blastradius-npm/${VERSION}` } },
        (res) => {
          if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
            res.resume();
            if (redirects >= MAX_REDIRECTS) {
              return fail(new Error(`too many redirects while fetching ${safeUrl}`));
            }
            let next;
            try {
              next = normalizeHttpsUrl(res.headers.location, safeUrl);
            } catch (e) {
              return fail(e);
            }
            return get(next, maxBytes, redirects + 1).then(resolve, fail);
          }
          if (res.statusCode !== 200) {
            res.resume();
            return fail(new Error(`GET ${safeUrl} -> ${res.statusCode}`));
          }
          const len = Number(res.headers["content-length"]);
          if (Number.isFinite(len) && len > maxBytes) {
            res.resume();
            return fail(new Error(`download too large from ${safeUrl}`));
          }
          const chunks = [];
          let total = 0;
          res.on("data", (c) => {
            total += c.length;
            if (total > maxBytes) {
              req.destroy(new Error(`download too large from ${safeUrl}`));
              return;
            }
            chunks.push(c);
          });
          res.on("end", () => {
            if (!settled) {
              settled = true;
              resolve(Buffer.concat(chunks, total));
            }
          });
          res.on("error", fail);
        }
      )
      .on("error", fail);
    req.setTimeout(DOWNLOAD_TIMEOUT_MS, () => {
      req.destroy(new Error(`GET ${safeUrl} timed out`));
    });
  });
}

function sha256(buf) {
  return crypto.createHash("sha256").update(buf).digest("hex");
}

function tarString(block, start, len) {
  const slice = block.subarray(start, start + len);
  const nul = slice.indexOf(0);
  return slice.subarray(0, nul === -1 ? slice.length : nul).toString("utf8");
}

function tarOctal(block, start, len) {
  const raw = tarString(block, start, len).trim();
  if (!raw) return 0;
  const n = Number.parseInt(raw.replace(/\0/g, ""), 8);
  if (!Number.isFinite(n) || n < 0) {
    throw new Error("invalid tar entry size");
  }
  return n;
}

function isSafeTarPath(name) {
  if (!name || name.includes("\0") || name.includes("\\")) return false;
  if (path.posix.isAbsolute(name) || /^[A-Za-z]:/.test(name)) return false;
  const parts = name.split("/");
  if (parts.some((p) => p === "..")) return false;
  const normalized = path.posix.normalize(name);
  return normalized !== "." && !normalized.startsWith("../");
}

function extractBinaryFromTarGz(archive, destDir) {
  const tar = zlib.gunzipSync(archive, { maxOutputLength: MAX_TAR_BYTES });
  const candidates = [];

  for (let offset = 0; offset + 512 <= tar.length; offset += 512) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;

    const name = tarString(header, 0, 100);
    const prefix = tarString(header, 345, 155);
    const entryName = prefix ? `${prefix}/${name}` : name;
    const type = tarString(header, 156, 1) || "0";
    const size = tarOctal(header, 124, 12);

    offset += 512;
    if (!isSafeTarPath(entryName)) {
      throw new Error(`unsafe path in release archive: ${entryName}`);
    }

    if (type === "x" || type === "g") {
      // PAX metadata; safe to skip. The release binary path is short and does
      // not rely on PAX long-path records.
    } else if (type === "5") {
      fs.mkdirSync(path.join(destDir, entryName), { recursive: true });
    } else if (type === "0") {
      const outPath = path.join(destDir, entryName);
      fs.mkdirSync(path.dirname(outPath), { recursive: true });
      fs.writeFileSync(outPath, tar.subarray(offset, offset + size), { mode: 0o755 });
      if (path.basename(entryName) === "blastradius") {
        candidates.push(outPath);
      }
    } else {
      throw new Error(`unsupported entry type in release archive: ${type || "unknown"}`);
    }

    offset += Math.ceil(size / 512) * 512 - 512;
  }

  if (candidates.length !== 1) {
    throw new Error(`expected exactly one blastradius binary, found ${candidates.length}`);
  }
  const st = fs.lstatSync(candidates[0]);
  if (!st.isFile() || st.isSymbolicLink()) {
    throw new Error("release archive binary is not a regular file");
  }
  return candidates[0];
}

function checksumFor(sums, tarball) {
  return sums
    .toString("utf8")
    .split("\n")
    .map((l) => l.trim().split(/\s+/))
    .find((parts) => parts[1] && parts[1].replace(/^\*/, "") === tarball)?.[0];
}

function sha256File(filePath) {
  return sha256(fs.readFileSync(filePath));
}

function readChecksumSidecar(checksumPath) {
  if (!fs.existsSync(checksumPath)) return null;
  const st = fs.lstatSync(checksumPath);
  if (!st.isFile() || st.isSymbolicLink() || st.size > MAX_CACHE_CHECKSUM_BYTES) {
    return null;
  }
  const expected = fs.readFileSync(checksumPath, "utf8").trim();
  return /^[a-f0-9]{64}$/.test(expected) ? expected : null;
}

function cachedBinaryIsValid(binPath, checksumPath) {
  if (!fs.existsSync(binPath)) return false;
  const st = fs.lstatSync(binPath);
  if (!st.isFile() || st.isSymbolicLink()) {
    throw new Error(`cached binary is not a regular file: ${binPath}`);
  }
  const expected = readChecksumSidecar(checksumPath);
  return expected !== null && sha256File(binPath) === expected;
}

async function ensureBinary() {
  const target = detectTarget();
  const dir = cacheDir();
  const binPath = path.join(dir, "blastradius");
  const checksumPath = path.join(dir, "blastradius.sha256");
  if (cachedBinaryIsValid(binPath, checksumPath)) {
    return binPath;
  }

  const tarball = `blastradius-${target}.tar.gz`;
  process.stderr.write(`blastradius: downloading ${tarball}\n`);
  const [archive, sums] = await Promise.all([
    get(`${BASE}/${tarball}`, MAX_ARCHIVE_BYTES),
    get(`${BASE}/SHA256SUMS`, MAX_SUMS_BYTES),
  ]);

  // Verify checksum against SHA256SUMS.
  const want = checksumFor(sums, tarball);
  if (!want) throw new Error(`no checksum for ${tarball} in SHA256SUMS`);
  const got = sha256(archive);
  if (got !== want) {
    throw new Error(`checksum mismatch for ${tarball}: ${got} != ${want}`);
  }

  fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
  const extractDir = fs.mkdtempSync(path.join(dir, ".extract-"));
  const tmpBin = path.join(dir, `.blastradius-${process.pid}-${Date.now()}`);
  const tmpChecksum = path.join(dir, `.blastradius.sha256-${process.pid}-${Date.now()}`);
  try {
    const extracted = extractBinaryFromTarGz(archive, extractDir);
    fs.copyFileSync(extracted, tmpBin);
    fs.chmodSync(tmpBin, 0o755);
    fs.writeFileSync(tmpChecksum, `${sha256File(tmpBin)}\n`, { mode: 0o600 });
    fs.renameSync(tmpBin, binPath);
    fs.renameSync(tmpChecksum, checksumPath);
  } finally {
    try {
      if (fs.existsSync(tmpBin)) fs.unlinkSync(tmpBin);
    } catch (_) {}
    try {
      if (fs.existsSync(tmpChecksum)) fs.unlinkSync(tmpChecksum);
    } catch (_) {}
    fs.rmSync(extractDir, { recursive: true, force: true });
  }
  return binPath;
}

async function main() {
  try {
    const bin = await ensureBinary();
    const res = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
    process.exit(res.status === null ? 1 : res.status);
  } catch (e) {
    process.stderr.write(`blastradius: ${e.message}\n`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  cachedBinaryIsValid,
  checksumFor,
  extractBinaryFromTarGz,
  isSafeTarPath,
  normalizeHttpsUrl,
  readChecksumSidecar,
  sha256,
  sha256File,
};
