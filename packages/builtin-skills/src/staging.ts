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

function lstatExists(targetPath: string): fs.Stats | null {
  try {
    return fs.lstatSync(targetPath);
  } catch {
    return null;
  }
}

function writeReadOnlyNoFollow(filePath: string, content: string, mode = 0o444): void {
  const flags = fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(filePath, flags, 0o600);
  try {
    const st = fs.fstatSync(fd);
    if (!st.isFile() || st.nlink !== 1) {
      throw new Error(`UnsafeSkillFileTypeOrHardlink: ${path.basename(filePath)}`);
    }
    fs.writeFileSync(fd, content, "utf8");
    fs.fchmodSync(fd, mode);
  } finally {
    fs.closeSync(fd);
  }
}

export function validateStagedPromptContent(overview: string): void {
  const folded = overview.normalize("NFKC");
  const lower = folded.toLowerCase();
  for (const tag of [
    "<important", "</important", "<system", "</system", "<!--", "-->", "<img",
    "<learned_cards_data", "</learned_cards_data", "<skill_catalog_data", "</skill_catalog_data",
    "<learn_context", "</learn_context", "<assistant", "</assistant", "<tool_call", "</tool_call",
  ]) {
    if (lower.includes(tag) || overview.toLowerCase().includes(tag)) {
      throw new Error(`Disallowed prompt directive or HTML tag '${tag}' in PROMPT.md`);
    }
  }
  if (/[\u00AD\u034F\u061C\u180E\u200B-\u200F\u2028-\u202E\u2060-\u2069\uFE00-\uFE0F\uFEFF]|\uDB40[\uDC01-\uDC7F]/.test(overview)) {
    throw new Error("Disallowed zero-width or bidirectional control Unicode character in PROMPT.md");
  }
  if (/!\[[^\]]*\]\([^)]*\)|!\[[^\]]*\]\[[^\]]*\]/.test(lower)) {
    throw new Error("Disallowed Markdown image beacon in PROMPT.md");
  }
}

function ensureSafeBaseDir(baseDir: string): string {
  const absBase = path.resolve(baseDir);
  const baseStat = lstatExists(absBase);
  if (baseStat) {
    if (baseStat.isSymbolicLink() || !baseStat.isDirectory()) {
      throw new Error("baseStagingDir must be a real non-symlink directory");
    }
    return fs.realpathSync(absBase);
  }

  let cur = absBase;
  const missing: string[] = [];
  while (!lstatExists(cur)) {
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
    if (!lstatExists(resolved)) {
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
  const stat = lstatExists(targetPath);
  if (!stat) return;
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
    if (!segment || segment === "." || segment === ".." || segment.includes("/") || segment.includes("\\") || segment.includes("\0") || /[\x00-\x1f\x7f-\x9f]/.test(segment)) {
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
        writeReadOnlyNoFollow(manifestPath, JSON.stringify(skill, null, 2), 0o444);

        if (skill.prompt_overview) {
          const promptPath = path.join(skillDir, "PROMPT.md");
          writeReadOnlyNoFollow(promptPath, skill.prompt_overview, 0o444);
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
      writeReadOnlyNoFollow(receiptPath, JSON.stringify(receipt, null, 2), 0o444);

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
    const absBase = path.resolve(this.baseStagingDir);
    const baseStat = lstatExists(absBase);
    if (!baseStat) return;
    if (baseStat.isSymbolicLink() || !baseStat.isDirectory()) {
      throw new Error(`Refusing cleanup across symlink or non-directory baseStagingDir: ${absBase}`);
    }
    const baseResolved = fs.realpathSync(absBase);
    const runDir = path.resolve(baseResolved, runId);
    if (!runDir.startsWith(`${baseResolved}${path.sep}`)) {
      throw new Error(`Resolved runDir escaped baseStagingDir: ${runDir}`);
    }
    const runStat = lstatExists(runDir);
    if (!runStat) return;
    if (runStat.isSymbolicLink()) {
      throw new Error(`Refusing cleanup across symlink: ${runDir}`);
    }
    restoreWritableRecursive(runDir);
    fs.rmSync(runDir, { recursive: true, force: true });
  }
}
