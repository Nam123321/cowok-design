// @story 08.1
// @implements AC-01, AC-07
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import { SkillManifest, StagingRunReceipt } from "./types";

const KV_HASH_PROTOCOL = Buffer.from("cowok-kv-hash-v1", "utf8");

export interface StagedSkillSource {
  manifest: SkillManifest;
  skillMdSha256?: string | null;
}

function u64Be(n: number): Buffer {
  const buf = Buffer.allocUnsafe(8);
  buf.writeBigUInt64BE(BigInt(n), 0);
  return buf;
}

function updateLenPrefixed(hasher: crypto.Hash, data: Buffer | string): void {
  const buf = typeof data === "string" ? Buffer.from(data, "utf8") : data;
  hasher.update(u64Be(buf.length));
  hasher.update(buf);
}

function canonicalizeJson(val: unknown): string {
  if (val === null) return "null";
  if (typeof val !== "object") return JSON.stringify(val);
  if (Array.isArray(val)) return `[${val.map((v) => canonicalizeJson(v)).join(",")}]`;
  const obj = val as Record<string, unknown>;
  const sortedKeys = Object.keys(obj).sort((a, b) => Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8")));
  return `{${sortedKeys.map((k) => `${JSON.stringify(k)}:${canonicalizeJson(obj[k])}`).join(",")}}`;
}

function hashOptionalString(hasher: crypto.Hash, val?: string | null): void {
  if (val === undefined || val === null) {
    hasher.update(Buffer.from([0]));
  } else {
    hasher.update(Buffer.from([1]));
    updateLenPrefixed(hasher, val);
  }
}

function hashSortedUnique(hasher: crypto.Hash, items?: string[]): void {
  const unique = Array.from(new Set(items || [])).sort((a, b) =>
    Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8")),
  );
  hasher.update(u64Be(unique.length));
  for (const item of unique) {
    updateLenPrefixed(hasher, item);
  }
}

export function validateStagedPromptContent(overview: string): void {
  const lower = overview.toLowerCase();
  for (const tag of ["<important", "</important", "<system", "</system", "<!--", "-->", "<img"]) {
    if (lower.includes(tag)) {
      throw new Error(`Disallowed prompt directive or HTML tag '${tag}' in PROMPT.md`);
    }
  }
  if (/[\u200B-\u200F\u202A-\u202E\u2066-\u2069\uFEFF]/.test(overview)) {
    throw new Error("Disallowed zero-width or bidirectional control Unicode character in PROMPT.md");
  }
  if (/!\[[^\]]*\]\([^)]*\)/.test(lower)) {
    throw new Error("Disallowed Markdown image beacon in PROMPT.md");
  }
}

function ensureSafeBaseDir(baseDir: string): string {
  const absBase = path.resolve(baseDir);
  if (fs.existsSync(absBase)) {
    const stat = fs.lstatSync(absBase);
    if (stat.isSymbolicLink() || !stat.isDirectory()) {
      throw new Error("baseStagingDir must be a real non-symlink directory");
    }
    return fs.realpathSync(absBase);
  }

  let cur = absBase;
  const missing: string[] = [];
  while (!fs.existsSync(cur)) {
    const parent = path.dirname(cur);
    if (parent === cur) break;
    missing.push(path.basename(cur));
    cur = parent;
  }

  const anchorStat = fs.lstatSync(cur);
  if (anchorStat.isSymbolicLink() || !anchorStat.isDirectory()) {
    throw new Error(`Symlink component disallowed in staging anchor: ${cur}`);
  }

  let resolved = fs.realpathSync(cur);
  for (const segment of missing.reverse()) {
    resolved = path.join(resolved, segment);
    if (!fs.existsSync(resolved)) {
      fs.mkdirSync(resolved, { recursive: false });
    }
    const segStat = fs.lstatSync(resolved);
    if (segStat.isSymbolicLink() || !segStat.isDirectory()) {
      throw new Error(`Symlink component disallowed in staging path: ${resolved}`);
    }
  }
  return resolved;
}

function restoreWritableRecursive(targetPath: string): void {
  if (!fs.existsSync(targetPath)) return;
  const stat = fs.lstatSync(targetPath);
  if (stat.isSymbolicLink()) {
    throw new Error(`Refusing permission restore across symlink: ${targetPath}`);
  }
  fs.chmodSync(targetPath, stat.isDirectory() ? 0o755 : 0o644);
  if (stat.isDirectory()) {
    for (const child of fs.readdirSync(targetPath)) {
      restoreWritableRecursive(path.join(targetPath, child));
    }
  }
}

export class PerRunStagingManager {
  private baseStagingDir: string;

  constructor(baseStagingDir: string) {
    this.baseStagingDir = baseStagingDir;
  }

  public static computeStablePromptHashWithSources(sources: StagedSkillSource[]): string {
    const sorted = [...sources].sort((a, b) =>
      Buffer.compare(Buffer.from(a.manifest.name, "utf8"), Buffer.from(b.manifest.name, "utf8")),
    );
    const hasher = crypto.createHash("sha256");
    updateLenPrefixed(hasher, KV_HASH_PROTOCOL);
    hasher.update(u64Be(sorted.length));

    for (const { manifest: skill, skillMdSha256 } of sorted) {
      updateLenPrefixed(hasher, skill.name);
      updateLenPrefixed(hasher, skill.version);
      updateLenPrefixed(hasher, skill.description);
      updateLenPrefixed(hasher, skill.entry_point || "SKILL.md");
      hashSortedUnique(hasher, skill.capabilities_required);
      hasher.update(Buffer.from([skill.risk_tier ?? 1]));
      hashOptionalString(hasher, skill.parameters !== undefined && skill.parameters !== null ? canonicalizeJson(skill.parameters) : null);
      hashSortedUnique(hasher, skill.tags);
      hashOptionalString(hasher, skill.prompt_overview ?? null);
      hashOptionalString(hasher, skillMdSha256 ?? null);
    }
    return hasher.digest("hex");
  }

  public static computeStablePromptHash(skills: SkillManifest[]): string {
    return PerRunStagingManager.computeStablePromptHashWithSources(
      skills.map((manifest) => ({ manifest, skillMdSha256: null })),
    );
  }

  private static assertSafeSegment(kind: string, segment: string): void {
    if (!segment || segment === "." || segment === ".." || segment.includes("/") || segment.includes("\\") || segment.includes("\0") || /[\x00-\x1f\x7f]/.test(segment)) {
      throw new Error(`Invalid ${kind} segment '${segment}': path separators and traversal segments are forbidden`);
    }
  }

  public stageRunWithSources(runId: string, sources: StagedSkillSource[]): StagingRunReceipt {
    PerRunStagingManager.assertSafeSegment("runId", runId);
    const baseResolved = ensureSafeBaseDir(this.baseStagingDir);
    const runDir = path.resolve(baseResolved, runId);
    if (!runDir.startsWith(`${baseResolved}${path.sep}`)) {
      throw new Error(`Resolved runDir escaped baseStagingDir: ${runDir}`);
    }
    if (fs.existsSync(runDir) || ((): boolean => { try { fs.lstatSync(runDir); return true; } catch { return false; } })()) {
      throw new Error(`Staging run directory already exists or is a symlink: ${runId}`);
    }

    fs.mkdirSync(runDir, { recursive: false });
    try {
      const sorted = [...sources].sort((a, b) =>
        Buffer.compare(Buffer.from(a.manifest.name, "utf8"), Buffer.from(b.manifest.name, "utf8")),
      );
      const stagedSkills: string[] = [];
      const skillDirs: string[] = [];

      for (const { manifest: skill } of sorted) {
        PerRunStagingManager.assertSafeSegment("skill.name", skill.name);
        if (skill.prompt_overview) validateStagedPromptContent(skill.prompt_overview);

        const skillDir = path.resolve(runDir, skill.name);
        if (!skillDir.startsWith(`${runDir}${path.sep}`)) {
          throw new Error(`Resolved skillDir escaped runDir: ${skillDir}`);
        }
        fs.mkdirSync(skillDir, { recursive: false });

        const manifestPath = path.join(skillDir, "plugin.json");
        fs.writeFileSync(manifestPath, JSON.stringify(skill, null, 2), "utf8");
        fs.chmodSync(manifestPath, 0o444);

        if (skill.prompt_overview) {
          const promptPath = path.join(skillDir, "PROMPT.md");
          fs.writeFileSync(promptPath, skill.prompt_overview, "utf8");
          fs.chmodSync(promptPath, 0o444);
        }

        skillDirs.push(skillDir);
        stagedSkills.push(skill.name);
      }

      const stablePromptHash = PerRunStagingManager.computeStablePromptHashWithSources(sorted);
      const receipt: StagingRunReceipt = {
        run_id: runId,
        staged_skills: stagedSkills,
        stable_prompt_hash: stablePromptHash,
        staged_dir: runDir,
        timestamp: Date.now(),
      };

      const receiptPath = path.join(runDir, "run-receipt.json");
      fs.writeFileSync(receiptPath, JSON.stringify(receipt, null, 2), "utf8");
      fs.chmodSync(receiptPath, 0o444);

      for (const dir of skillDirs) fs.chmodSync(dir, 0o555);
      fs.chmodSync(runDir, 0o555);
      return receipt;
    } catch (err) {
      try {
        restoreWritableRecursive(runDir);
        fs.rmSync(runDir, { recursive: true, force: true });
      } catch {
        // Ignore secondary rollback failure while propagating primary staging error
      }
      throw err;
    }
  }

  public stageRun(runId: string, skills: SkillManifest[]): StagingRunReceipt {
    return this.stageRunWithSources(
      runId,
      skills.map((manifest) => ({ manifest, skillMdSha256: null })),
    );
  }

  public cleanupRun(runId: string): void {
    PerRunStagingManager.assertSafeSegment("runId", runId);
    const runDir = path.resolve(this.baseStagingDir, runId);
    if (!fs.existsSync(runDir)) return;
    if (fs.lstatSync(runDir).isSymbolicLink()) {
      throw new Error(`Refusing cleanup across symlink: ${runDir}`);
    }
    restoreWritableRecursive(runDir);
    fs.rmSync(runDir, { recursive: true, force: true });
  }
}
