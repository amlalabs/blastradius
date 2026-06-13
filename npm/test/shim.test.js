const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const zlib = require("zlib");

const {
  cachedBinaryIsValid,
  checksumFor,
  extractBinaryFromTarGz,
  isSafeTarPath,
  normalizeHttpsUrl,
  readChecksumSidecar,
  sha256,
} = require("../bin/blastradius.js");

function writeAscii(buf, value, offset, length) {
  buf.write(value, offset, Math.min(length, Buffer.byteLength(value)), "ascii");
}

function writeOctal(buf, value, offset, length) {
  const text = value.toString(8).padStart(length - 1, "0") + "\0";
  writeAscii(buf, text, offset, length);
}

function tarEntry(name, body = "", type = "0") {
  const content = Buffer.from(body);
  const header = Buffer.alloc(512, 0);
  writeAscii(header, name, 0, 100);
  writeOctal(header, 0o755, 100, 8);
  writeOctal(header, 0, 108, 8);
  writeOctal(header, 0, 116, 8);
  writeOctal(header, content.length, 124, 12);
  writeOctal(header, 0, 136, 12);
  header.fill(0x20, 148, 156);
  writeAscii(header, type, 156, 1);
  writeAscii(header, "ustar", 257, 6);
  writeAscii(header, "00", 263, 2);

  let sum = 0;
  for (const byte of header) sum += byte;
  writeAscii(header, sum.toString(8).padStart(6, "0"), 148, 6);
  header[154] = 0;
  header[155] = 0x20;

  const padding = Buffer.alloc((512 - (content.length % 512)) % 512, 0);
  return Buffer.concat([header, content, padding]);
}

function archive(entries) {
  return zlib.gzipSync(Buffer.concat([...entries, Buffer.alloc(1024, 0)]));
}

function withTemp(fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "blastradius-shim-"));
  try {
    return fn(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

assert.strictEqual(isSafeTarPath("blastradius"), true);
assert.strictEqual(isSafeTarPath("./bin/blastradius"), true);
assert.strictEqual(isSafeTarPath("../blastradius"), false);
assert.strictEqual(isSafeTarPath("/tmp/blastradius"), false);
assert.strictEqual(isSafeTarPath("bin\\blastradius"), false);

assert.strictEqual(
  normalizeHttpsUrl(
    "/download/blastradius.tar.gz",
    "https://github.com/amlalabs/blastradius/releases"
  ),
  "https://github.com/download/blastradius.tar.gz"
);
assert.throws(
  () => normalizeHttpsUrl("http://example.com/blastradius.tar.gz"),
  /refusing non-HTTPS/
);
assert.throws(
  () => normalizeHttpsUrl("file:///tmp/blastradius.tar.gz"),
  /refusing non-HTTPS/
);

withTemp((dir) => {
  const out = extractBinaryFromTarGz(
    archive([tarEntry("./blastradius", "binary")]),
    dir
  );
  assert.strictEqual(path.basename(out), "blastradius");
  assert.strictEqual(fs.readFileSync(out, "utf8"), "binary");
});

withTemp((dir) => {
  assert.throws(
    () => extractBinaryFromTarGz(archive([tarEntry("../blastradius", "x")]), dir),
    /unsafe path/
  );
});

withTemp((dir) => {
  assert.throws(
    () => extractBinaryFromTarGz(archive([tarEntry("blastradius", "", "2")]), dir),
    /unsupported entry type/
  );
});

withTemp((dir) => {
  assert.throws(
    () =>
      extractBinaryFromTarGz(
        archive([
          tarEntry("one/blastradius", "x"),
          tarEntry("two/blastradius", "y"),
        ]),
        dir
      ),
    /expected exactly one/
  );
});

assert.strictEqual(
  checksumFor(
    Buffer.from("abc123  *blastradius-x86_64-unknown-linux-musl.tar.gz\n"),
    "blastradius-x86_64-unknown-linux-musl.tar.gz"
  ),
  "abc123"
);

withTemp((dir) => {
  const bin = path.join(dir, "blastradius");
  const sum = path.join(dir, "blastradius.sha256");
  fs.writeFileSync(bin, "binary");
  fs.writeFileSync(sum, `${sha256(Buffer.from("binary"))}\n`);
  assert.strictEqual(cachedBinaryIsValid(bin, sum), true);

  fs.writeFileSync(bin, "tampered");
  assert.strictEqual(cachedBinaryIsValid(bin, sum), false);

  fs.writeFileSync(bin, "binary");
  fs.unlinkSync(sum);
  assert.strictEqual(cachedBinaryIsValid(bin, sum), false);

  fs.writeFileSync(sum, `${sha256(Buffer.from("binary"))}\n`.repeat(8));
  assert.strictEqual(readChecksumSidecar(sum), null);
  assert.strictEqual(cachedBinaryIsValid(bin, sum), false);
});

withTemp((dir) => {
  const target = path.join(dir, "target");
  const bin = path.join(dir, "blastradius");
  const sum = path.join(dir, "blastradius.sha256");
  fs.writeFileSync(target, "binary");
  try {
    fs.symlinkSync(target, bin);
    assert.throws(() => cachedBinaryIsValid(bin, sum), /not a regular file/);
  } catch (e) {
    if (e.code !== "EPERM") throw e;
  }
});

withTemp((dir) => {
  const target = path.join(dir, "target.sha256");
  const sum = path.join(dir, "blastradius.sha256");
  fs.writeFileSync(target, `${sha256(Buffer.from("binary"))}\n`);
  try {
    fs.symlinkSync(target, sum);
    assert.strictEqual(readChecksumSidecar(sum), null);
  } catch (e) {
    if (e.code !== "EPERM") throw e;
  }
});

withTemp((dir) => {
  const tooLarge = zlib.gzipSync(Buffer.alloc(129 * 1024 * 1024));
  assert.throws(
    () => extractBinaryFromTarGz(tooLarge, dir),
    /Cannot create a Buffer larger/
  );
});

console.log("shim tests passed");
