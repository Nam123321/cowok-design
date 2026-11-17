// @story 08.2
// @implements AC-01, AC-02, AC-06, AC-07
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import {
  DiscoveryStub,
  DiscoveryStubSchema,
  SkillManifest,
  SkillPluginDto,
  SkillPluginDtoSchema,
  SkillsLockFile,
  SkillsLockSchema,
  StagedBundleReceiptSchema,
} from "./types";
import { COMPILED_RELEASE_BRIDGE_SHA256 } from "./bridge-pin";
import { scanSkillMarkdownSafety } from "./repackager-assets";

export const PKG_HASH_DOMAIN = Buffer.from("cowok-pkg-hash-v1", "utf8");
export const MAX_DISCOVERY_STUB_BYTES = 1536;
export const MAX_SINGLE_FILE_BYTES = 1048576;

let sessionBridgePin: { filePath: string; sha256Hex: string } | null = null;

const MACOS_ROOT_SYMLINKS = new Set(["tmp", "var", "etc"]);

function isSafeNonWritableNode(st: fs.Stats, wantDir: boolean): boolean {
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  const typeOk = wantDir ? st.isDirectory() : st.isFile() && st.nlink === 1;
  return typeOk && (st.mode & 0o022) === 0 && (uid === null || st.uid === uid || st.uid === 0);
}

function readNoFollowRegularBytes(filePath: string, requireExec: boolean): Buffer {
  const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(filePath, flags);
  try {
    const st = fs.fstatSync(fd);
    const execOk = !requireExec || (st.mode & 0o111) !== 0;
    if (!execOk || !isSafeNonWritableNode(st, false)) {
      throw new Error(`UnsafeSkillFileTypeOrHardlink: ${path.basename(filePath)}`);
    }
    return fs.readFileSync(fd);
  } finally {
    fs.closeSync(fd);
  }
}

function areAncestorsTrusted(parentDir: string): boolean {
  const parts = path.resolve(parentDir).split(path.sep).filter(Boolean);
  let cur: string = path.sep;
  for (let i = 0; i < parts.length; i++) {
    cur = path.join(cur, parts[i]);
    if (process.platform === "darwin" && i === 0 && MACOS_ROOT_SYMLINKS.has(parts[0])) {
      cur = path.join("/private", parts[0]);
      continue;
    }
    const st = fs.lstatSync(cur);
    if (st.isSymbolicLink() || !isSafeNonWritableNode(st, true)) return false;
  }
  return true;
}

function isTrustedBridgeExecutable(candidate: string): Buffer | null {
  try {
    if (!areAncestorsTrusted(path.dirname(candidate))) return null;
    return crypto.createHash("sha256").update(readNoFollowRegularBytes(candidate, true)).digest();
  } catch {
    return null;
  }
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
      timeout: 60_000,
    });
    const builtDigest = isTrustedBridgeExecutable(debugBin);
    if (!builtDigest) {
      throw new Error("PluginHostBridgeUnavailable: failed building trusted plugin_host_repackager binary");
    }
    sessionBridgePin = { filePath: debugBin, sha256Hex: builtDigest.toString("hex") };
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

function validateSafeSkillSegment(segment: string): void {
  if (!segment || segment === "." || segment === ".." || Buffer.byteLength(segment, "utf8") > 64 || !/^[A-Za-z0-9._-]+$/.test(segment)) {
    throw new Error(`Invalid skill_name '${segment}': unsafe characters, traversal segment, or length > 64B`);
  }
}

function validateUpstreamUrl(url: string): void {
  if (Buffer.byteLength(url, "utf8") > 256 || !/^https?:\/\/[^/\s@]+/i.test(url) || url.includes("@") || /[\s\x00-\x1f\x7f-\x9f]/.test(url)) {
    throw new Error("Invalid or oversized upstream_url for DiscoveryStub");
  }
}

export function buildDiscoveryStub(params: {
  skillName: string;
  description: string;
  capabilities: string[];
  upstreamUrl?: string;
  packageSha256: string;
}): { stub: DiscoveryStub; stubJson: string } {
  const { skillName, description, capabilities, upstreamUrl, packageSha256 } = params;
  validateSafeSkillSegment(skillName);
  scanSkillMarkdownSafety("description", description);
  if (description.replace(/\r\n?/g, "\n").split("\n").some((l) => l.trim() === "---")) {
    throw new Error("YAML delimiter '---' line forbidden inside frontmatter field");
  }
  if (Buffer.byteLength(description, "utf8") > 320) throw new Error("DiscoveryStub overflow: description > 320B");
  if (capabilities.length > 16 || capabilities.some((c) => Buffer.byteLength(c, "utf8") > 48)) {
    throw new Error("DiscoveryStub overflow: capabilities limit exceeded");
  }
  for (const cap of capabilities) scanSkillMarkdownSafety("capability", cap);
  if (upstreamUrl !== undefined) validateUpstreamUrl(upstreamUrl);
  const stub: DiscoveryStub = {
    name: skillName,
    version: "1.0.0",
    description,
    ...(upstreamUrl !== undefined ? { upstream_url: upstreamUrl } : {}),
    capabilities,
    risk_tier: 2,
    package_sha256: packageSha256,
    stub_size_bytes: 0,
  };
  for (let i = 0; i < 8; i++) {
    const finalJson = JSON.stringify(stub, null, 2);
    const bytes = Buffer.byteLength(finalJson, "utf8");
    if (bytes === stub.stub_size_bytes) {
      if (bytes > MAX_DISCOVERY_STUB_BYTES) {
        throw new Error(`DiscoveryStubFailClosed: ${bytes}B > ${MAX_DISCOVERY_STUB_BYTES}B`);
      }
      return { stub: DiscoveryStubSchema.parse(stub), stubJson: finalJson };
    }
    stub.stub_size_bytes = bytes;
  }
  throw new Error("DiscoveryStubFailClosed: stub_size_bytes failed to converge to serialized byte length");
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
