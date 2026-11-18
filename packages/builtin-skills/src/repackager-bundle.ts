// @story 08.2
// @implements AC-01, AC-02, AC-06, AC-07
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import {
  DiscoveryStub,
  SkillManifest,
  SkillPluginDto,
  SkillPluginDtoSchema,
  SkillsLockFile,
  SkillsLockSchema,
  StagedBundleReceiptSchema,
} from "./types";
import { COMPILED_RELEASE_BRIDGE_SHA256 } from "./bridge-pin";
import { buildDiscoveryStub, MAX_DISCOVERY_STUB_BYTES } from "./repackager-stub";

export type { DiscoveryStub };
export { buildDiscoveryStub, MAX_DISCOVERY_STUB_BYTES };
export const PKG_HASH_DOMAIN = Buffer.from("cowok-pkg-hash-v1", "utf8");
export const MAX_SINGLE_FILE_BYTES = 1048576;

let sessionBridgePin: { filePath: string; sha256Hex: string } | null = null;

const MACOS_ROOT_SYMLINKS = new Set(["tmp", "var", "etc"]);

let cachedLinuxGroupUsers: Map<number, Set<number>> | null | undefined;

function getLinuxGroupUsers(): Map<number, Set<number>> | null {
  if (cachedLinuxGroupUsers !== undefined) return cachedLinuxGroupUsers;
  if (process.platform !== "linux") {
    cachedLinuxGroupUsers = null;
    return null;
  }
  try {
    const passwdLines = fs.readFileSync("/etc/passwd", "utf8").split("\n");
    const groupLines = fs.readFileSync("/etc/group", "utf8").split("\n");
    const nameToUid = new Map<string, number>();
    const map = new Map<number, Set<number>>();
    for (const raw of passwdLines) {
      if (!raw || raw.startsWith("#")) continue;
      const parts = raw.split(":");
      if (parts.length < 4) continue;
      const uid = Number(parts[2]);
      const gid = Number(parts[3]);
      if (Number.isInteger(uid)) nameToUid.set(parts[0], uid);
      if (Number.isInteger(uid) && Number.isInteger(gid) && uid !== 0) {
        let set = map.get(gid);
        if (!set) {
          set = new Set<number>();
          map.set(gid, set);
        }
        set.add(uid);
      }
    }
    for (const raw of groupLines) {
      if (!raw || raw.startsWith("#")) continue;
      const parts = raw.split(":");
      if (parts.length < 4) continue;
      const gid = Number(parts[2]);
      if (!Number.isInteger(gid)) continue;
      const members = parts[3].split(",").map((m) => m.trim()).filter(Boolean);
      for (const m of members) {
        const u = nameToUid.get(m);
        if (u !== undefined && u !== 0) {
          let set = map.get(gid);
          if (!set) {
            set = new Set<number>();
            map.set(gid, set);
          }
          set.add(u);
        }
      }
    }
    cachedLinuxGroupUsers = map;
    return map;
  } catch {
    cachedLinuxGroupUsers = null;
    return null;
  }
}

function isSingleUserPrivateGroupLinux(gid: number, ownerUid: number): boolean {
  if (process.platform !== "linux" || ownerUid === 0) return false;
  const groups = getLinuxGroupUsers();
  if (!groups) return false;
  const members = groups.get(gid);
  if (!members || members.size === 0) return false;
  return members.size === 1 && members.has(ownerUid);
}

function isSafeNonWritableNode(st: fs.Stats, wantDir: boolean, allowLinuxCargoHardlink = false): boolean {
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  // On Linux, Cargo hardlinks target/{debug,release}/<bin> with target/.../deps/<bin>-<hash> (nlink === 2).
  const maxFileLinks = allowLinuxCargoHardlink && process.platform === "linux" ? 2 : 1;
  const typeOk = wantDir ? st.isDirectory() : st.isFile() && st.nlink >= 1 && st.nlink <= maxFileLinks;
  if (!typeOk || (uid !== null && st.uid !== uid && st.uid !== 0)) return false;
  if ((st.mode & 0o002) !== 0) return false;
  if ((st.mode & 0o020) !== 0) {
    return uid !== null && isSingleUserPrivateGroupLinux(st.gid, st.uid);
  }
  return true;
}

function readNoFollowRegularBytes(filePath: string, requireExec: boolean): Buffer {
  const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(filePath, flags);
  try {
    const st = fs.fstatSync(fd);
    const execOk = !requireExec || (st.mode & 0o111) !== 0;
    if (!execOk || !isSafeNonWritableNode(st, false, requireExec)) {
      throw new Error(`UnsafeSkillFileTypeOrHardlink: ${path.basename(filePath)} (mode=0${(st.mode & 0o777).toString(8)}, nlink=${st.nlink}, uid=${st.uid}, gid=${st.gid})`);
    }
    return fs.readFileSync(fd);
  } finally {
    fs.closeSync(fd);
  }
}

function assertAncestorsTrusted(parentDir: string): void {
  const parts = path.resolve(parentDir).split(path.sep).filter(Boolean);
  let cur: string = path.sep;
  for (let i = 0; i < parts.length; i++) {
    if (process.platform === "darwin" && i === 0 && MACOS_ROOT_SYMLINKS.has(parts[0])) {
      const priv = fs.lstatSync("/private");
      if (priv.isSymbolicLink() || !isSafeNonWritableNode(priv, true)) {
        throw new Error(`Untrusted ancestor /private (mode=0${(priv.mode & 0o777).toString(8)}, uid=${priv.uid}, gid=${priv.gid})`);
      }
      cur = path.join("/private", parts[0]);
    } else {
      cur = path.join(cur, parts[i]);
    }
    const st = fs.lstatSync(cur);
    if (st.isSymbolicLink() || !isSafeNonWritableNode(st, true)) {
      throw new Error(`Untrusted ancestor ${cur} (sym=${st.isSymbolicLink()}, mode=0${(st.mode & 0o777).toString(8)}, uid=${st.uid}, gid=${st.gid})`);
    }
  }
}

function tryVerifyBridgeExecutable(candidate: string): { digest: Buffer | null; reason?: string } {
  try {
    assertAncestorsTrusted(path.dirname(candidate));
    return { digest: crypto.createHash("sha256").update(readNoFollowRegularBytes(candidate, true)).digest() };
  } catch (err) {
    return { digest: null, reason: err instanceof Error ? err.message : String(err) };
  }
}

function isTrustedBridgeExecutable(candidate: string): Buffer | null {
  return tryVerifyBridgeExecutable(candidate).digest;
}

function timingSafeHexMatch(actualDigest: Buffer, expectedHex: string): boolean {
  if (!/^[a-f0-9]{64}$/.test(expectedHex)) return false;
  return crypto.timingSafeEqual(actualDigest, Buffer.from(expectedHex, "hex"));
}

/** Verifies candidate executable integrity and timing-safe SHA-256 equality against an immutable expected digest. */
export function verifyPinnedBridgeBinary(candidate: string, expectedSha256Hex: string): string {
  const normalizedPin = expectedSha256Hex.trim().toLowerCase();
  if (!/^[a-f0-9]{64}$/.test(normalizedPin)) {
    throw new Error("PluginHostBridgeUnavailable: immutable bridge digest pin missing or malformed");
  }
  const digest = isTrustedBridgeExecutable(candidate);
  if (!digest || !timingSafeHexMatch(digest, normalizedPin)) {
    throw new Error("PluginHostBridgeUnavailable: canonical bridge binary digest mismatch against immutable pin");
  }
  return candidate;
}

function resolveBridgeBinary(): string {
  if (process.platform === "win32") {
    throw new Error("UnsupportedPlatformSecurityCheck: requires Unix O_NOFOLLOW");
  }
  const exeName = "plugin_host_repackager";
  const repoRoot = path.resolve(__dirname, "../../..");
  const resourcesPath = (process as { resourcesPath?: string }).resourcesPath;
  const cargoToml = path.join(repoRoot, "Cargo.toml");
  const isProd = process.env.NODE_ENV === "production" || !fs.existsSync(cargoToml);

  // Production / packaged mode: fail closed unless compile-time release digest matches packaged canonical binary (zero env-var trust root).
  if (isProd) {
    if (!COMPILED_RELEASE_BRIDGE_SHA256 || !/^[a-f0-9]{64}$/.test(COMPILED_RELEASE_BRIDGE_SHA256)) {
      throw new Error("PluginHostBridgeUnavailable: immutable compile-time bridge trust root unpinned in production mode");
    }
    const prodAllowlist = [
      ...(resourcesPath ? [path.join(resourcesPath, "bin", exeName)] : []),
      path.join(repoRoot, "target", "release", exeName),
    ];
    for (const cand of prodAllowlist) {
      const digest = isTrustedBridgeExecutable(cand);
      if (digest && timingSafeHexMatch(digest, COMPILED_RELEASE_BRIDGE_SHA256)) return cand;
    }
    throw new Error("PluginHostBridgeUnavailable: no packaged canonical binary matches immutable compile-time digest pin");
  }

  // Dev workspace mode: bind session trust root via local Cargo build once, then re-verify exact binary digest before every execution.
  const debugBin = path.join(repoRoot, "target", "debug", exeName);
  if (!sessionBridgePin) {
    execFileSync("cargo", ["build", "--quiet", "-p", "plugin-host", "--bin", "plugin_host_repackager"], {
      cwd: repoRoot,
      stdio: "pipe",
      timeout: 180_000,
    });
    const check = tryVerifyBridgeExecutable(debugBin);
    if (!check.digest) {
      throw new Error(`PluginHostBridgeUnavailable: failed building trusted plugin_host_repackager binary (${check.reason || "unknown"})`);
    }
    sessionBridgePin = { filePath: debugBin, sha256Hex: check.digest.toString("hex") };
  }
  return verifyPinnedBridgeBinary(sessionBridgePin.filePath, sessionBridgePin.sha256Hex);
}

export function invokePluginHostBridge<T>(mode: "repackage" | "stage-bundle" | "verify-bundle" | "commit-bundle" | "video-intake", payload: unknown): T {
  const binPath = resolveBridgeBinary();
  try {
    const out = execFileSync(binPath, [mode], {
      input: JSON.stringify(payload),
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024,
      timeout: 30_000,
    });
    return JSON.parse(out) as T;
  } catch (err: unknown) {
    const stderr = (err as { stderr?: Buffer | string })?.stderr?.toString().trim();
    throw new Error(stderr || (err instanceof Error ? err.message : String(err)));
  }
}

export function manifestToSkillPlugin(manifest: SkillManifest): SkillPluginDto {
  return SkillPluginDtoSchema.parse({
    id: manifest.name,
    name: manifest.name,
    version: manifest.version,
    entry_point: manifest.entry_point,
    permissions: manifest.capabilities_required,
  });
}

function u64Be(n: number): Buffer {
  const b = Buffer.allocUnsafe(8);
  b.writeBigUInt64BE(BigInt(n), 0);
  return b;
}

export function sha256Hex(buf: Buffer | string): string {
  return crypto.createHash("sha256").update(buf).digest("hex");
}

/** Reads a non-symlink regular file (`nlink === 1`) via kernel `O_NOFOLLOW` descriptor. */
export function readVerifiedRegularUtf8(filePath: string): string {
  return readNoFollowRegularBytes(filePath, false).toString("utf8");
}

export function computePackageSha256(entries: Array<{ relPath: string; raw: Buffer }>): string {
  const sorted = [...entries].sort((a, b) => Buffer.compare(Buffer.from(a.relPath, "utf8"), Buffer.from(b.relPath, "utf8")));
  const h = crypto.createHash("sha256");
  h.update(PKG_HASH_DOMAIN);
  for (const item of sorted) {
    const p = Buffer.from(item.relPath, "utf8");
    h.update(u64Be(p.length));
    h.update(p);
    h.update(u64Be(item.raw.length));
    h.update(item.raw);
  }
  return h.digest("hex");
}

export interface StagedBundleReceipt {
  tmp_dir: string;
  skill_name: string;
  package_sha256: string;
  staging_token: string;
}

export function stageRepackagedBundle(params: {
  sourceDir: string;
  destDir: string;
  skillName: string;
  description: string;
  capabilities: string[];
  upstreamUrl?: string;
}): StagedBundleReceipt {
  const raw = invokePluginHostBridge<unknown>("stage-bundle", {
    source_dir: params.sourceDir,
    dest_dir: params.destDir,
    skill_name: params.skillName,
    description: params.description,
    capabilities: params.capabilities,
    upstream_url: params.upstreamUrl ?? null,
  });
  return StagedBundleReceiptSchema.parse(raw);
}

export function commitRepackagedDir(tmpDir: string, dstDir: string, stagingToken: string): void {
  invokePluginHostBridge<Record<string, never>>("commit-bundle", {
    tmp_dir: tmpDir,
    dst_dir: dstDir,
    staging_token: stagingToken,
  });
}

export function verifyRepackagedBundle(bundleDir: string): SkillsLockFile {
  const raw = invokePluginHostBridge<unknown>("verify-bundle", { bundle_dir: bundleDir });
  return SkillsLockSchema.parse(raw);
}
