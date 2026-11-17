#!/usr/bin/env node
// @story 08.2
// @implements AC-02, AC-07
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";

const pkgDir = path.resolve(__dirname_compat(), "..");
const repoRoot = path.resolve(pkgDir, "../..");
const defaultBin = path.join(repoRoot, "target", "release", "plugin_host_repackager");
const defaultPinTs = path.join(pkgDir, "src", "bridge-pin.ts");

function __dirname_compat() {
  return path.dirname(new URL(import.meta.url).pathname);
}

export function hashVerifiedExecutable(binPath) {
  const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(binPath, flags);
  try {
    const st = fs.fstatSync(fd);
    if (!st.isFile() || st.nlink !== 1 || (st.mode & 0o111) === 0 || (st.mode & 0o022) !== 0) {
      throw new Error(`Untrusted binary mode/nlink at ${binPath}`);
    }
    const bytes = fs.readFileSync(fd);
    return crypto.createHash("sha256").update(bytes).digest("hex");
  } finally {
    fs.closeSync(fd);
  }
}

export function writeBridgePinFile(binPath = defaultBin, outTsPath = defaultPinTs) {
  const sha = hashVerifiedExecutable(binPath);
  const content = [
    "// @story 08.2",
    "// @implements AC-02, AC-07",
    `export const COMPILED_RELEASE_BRIDGE_SHA256: string | null = "${sha}";`,
    "",
  ].join("\n");
  fs.writeFileSync(outTsPath, content, "utf8");
  return sha;
}

export function verifyBridgePinFile(binPath = defaultBin, pinTsPath = defaultPinTs) {
  const content = fs.readFileSync(pinTsPath, "utf8");
  const match = /COMPILED_RELEASE_BRIDGE_SHA256:\s*string\s*\|\s*null\s*=\s*"([a-f0-9]{64})"/.exec(content);
  if (!match) {
    throw new Error("Release bridge pin is unpopulated or malformed");
  }
  const actual = hashVerifiedExecutable(binPath);
  if (!crypto.timingSafeEqual(Buffer.from(actual, "hex"), Buffer.from(match[1], "hex"))) {
    throw new Error(`Release bridge pin mismatch: expected ${match[1]}, got ${actual}`);
  }
  return actual;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const verifyOnly = args.includes("--verify");
  const binIdx = args.indexOf("--bin");
  const outIdx = args.indexOf("--out");
  const binPath = binIdx >= 0 && args[binIdx + 1] ? path.resolve(args[binIdx + 1]) : defaultBin;
  const pinTsPath = outIdx >= 0 && args[outIdx + 1] ? path.resolve(args[outIdx + 1]) : defaultPinTs;
  const sha = verifyOnly ? verifyBridgePinFile(binPath, pinTsPath) : writeBridgePinFile(binPath, pinTsPath);
  console.log(JSON.stringify({ mode: verifyOnly ? "verify" : "write", binPath, sha256: sha }));
}
