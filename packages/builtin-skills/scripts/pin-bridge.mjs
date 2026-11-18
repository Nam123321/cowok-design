#!/usr/bin/env node
// @story 08.2
// @implements AC-02, AC-07
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "node:url";

const pkgDir = path.resolve(__dirname_compat(), "..");
const repoRoot = path.resolve(pkgDir, "../..");
const defaultBin = path.join(repoRoot, "target", "release", "plugin_host_repackager");
const defaultPinTs = path.join(pkgDir, "src", "bridge-pin.ts");

function __dirname_compat() {
  return path.dirname(fileURLToPath(import.meta.url));
}

function isSingleUserPrivateGroupLinux(gid, ownerUid) {
  if (process.platform !== "linux" || ownerUid === 0) return false;
  try {
    const passwdLines = fs.readFileSync("/etc/passwd", "utf8").split("\n");
    const groupLines = fs.readFileSync("/etc/group", "utf8").split("\n");
    const nameToUid = new Map();
    const members = new Set();
    for (const raw of passwdLines) {
      if (!raw || raw.startsWith("#")) continue;
      const parts = raw.split(":");
      if (parts.length < 4) continue;
      const uid = Number(parts[2]);
      const pGid = Number(parts[3]);
      if (Number.isInteger(uid)) nameToUid.set(parts[0], uid);
      if (Number.isInteger(uid) && pGid === gid && uid !== 0) members.add(uid);
    }
    for (const raw of groupLines) {
      if (!raw || raw.startsWith("#")) continue;
      const parts = raw.split(":");
      if (parts.length < 4 || Number(parts[2]) !== gid) continue;
      for (const m of parts[3].split(",").map((v) => v.trim()).filter(Boolean)) {
        const u = nameToUid.get(m);
        if (u !== undefined && u !== 0) members.add(u);
      }
    }
    return members.size === 1 && members.has(ownerUid);
  } catch {
    return false;
  }
}

function isSafeNonWritable(st) {
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  if (uid !== null && st.uid !== uid && st.uid !== 0) return false;
  if ((st.mode & 0o002) !== 0) return false;
  if ((st.mode & 0o020) !== 0) {
    return uid !== null && isSingleUserPrivateGroupLinux(st.gid, st.uid);
  }
  return true;
}

function isSafeDirNode(st) {
  return st.isDirectory() && !st.isSymbolicLink() && isSafeNonWritable(st);
}

function assertAncestorsTrusted(targetPath) {
  const macSyms = new Set(["tmp", "var", "etc"]);
  const parts = path.resolve(path.dirname(targetPath)).split(path.sep).filter(Boolean);
  let cur = path.sep;
  for (let i = 0; i < parts.length; i++) {
    if (process.platform === "darwin" && i === 0 && macSyms.has(parts[0])) {
      const priv = fs.lstatSync("/private");
      if (!isSafeDirNode(priv)) throw new Error("Untrusted /private ancestor directory");
      cur = path.join("/private", parts[0]);
    } else {
      cur = path.join(cur, parts[i]);
    }
    const st = fs.lstatSync(cur);
    if (!isSafeDirNode(st)) {
      throw new Error(`Untrusted ancestor directory or symlink at ${cur}`);
    }
  }
}

export function hashVerifiedExecutable(binPath) {
  assertAncestorsTrusted(binPath);
  const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(binPath, flags);
  try {
    const st = fs.fstatSync(fd);
    const maxBinLinks = process.platform === "linux" ? 2 : 1;
    if (!st.isFile() || st.nlink < 1 || st.nlink > maxBinLinks || (st.mode & 0o111) === 0 || !isSafeNonWritable(st)) {
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
  const absOut = path.resolve(outTsPath);
  const outDir = path.dirname(absOut);
  const dirStat = fs.lstatSync(outDir);
  if (dirStat.isSymbolicLink() || !dirStat.isDirectory()) {
    throw new Error(`Unsafe pin output directory: ${outDir}`);
  }
  const tmpPath = path.join(outDir, `.${path.basename(absOut)}.tmp-${process.pid}-${crypto.randomBytes(4).toString("hex")}`);
  const flags = fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL | (fs.constants.O_NOFOLLOW ?? 0);
  let fd;
  try {
    fd = fs.openSync(tmpPath, flags, 0o644);
    fs.writeFileSync(fd, content, "utf8");
    fs.fsyncSync(fd);
    fs.closeSync(fd);
    fd = undefined;
    fs.renameSync(tmpPath, absOut);
  } catch (err) {
    if (fd !== undefined) {
      try { fs.closeSync(fd); } catch {}
    }
    try { fs.rmSync(tmpPath, { force: true }); } catch {}
    throw err;
  }
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

if (
  process.argv[1] &&
  (() => {
    try {
      return fs.realpathSync(fileURLToPath(import.meta.url)) === fs.realpathSync(path.resolve(process.argv[1]));
    } catch {
      return false;
    }
  })()
) {
  const args = process.argv.slice(2);
  const verifyOnly = args.includes("--verify");
  const binIdx = args.indexOf("--bin");
  const outIdx = args.indexOf("--out");
  const binPath = binIdx >= 0 && args[binIdx + 1] ? path.resolve(args[binIdx + 1]) : defaultBin;
  const pinTsPath = outIdx >= 0 && args[outIdx + 1] ? path.resolve(args[outIdx + 1]) : defaultPinTs;
  const sha = verifyOnly ? verifyBridgePinFile(binPath, pinTsPath) : writeBridgePinFile(binPath, pinTsPath);
  console.log(JSON.stringify({ mode: verifyOnly ? "verify" : "write", binPath, sha256: sha }));
}
