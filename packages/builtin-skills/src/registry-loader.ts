// @story 08.1
// @implements AC-01, AC-02, AC-05, AC-06
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import { SkillManifest, SkillManifestSchema } from "./types";
import { validateSkillFrontmatter, ValidationResult } from "./validator";
import { readVerifiedRegularUtf8, verifyRepackagedBundle } from "./repackager-bundle";
import { assertNoYamlBreakout } from "./learn-sanitizer";
import { scanSkillMarkdownSafety } from "./repackager-assets";

function sha256Hex(content: string): string {
  return crypto.createHash("sha256").update(content, "utf8").digest("hex");
}

function isSafeSkillName(name: string): boolean {
  return Boolean(name) && name !== "." && name !== ".." && /^[a-zA-Z0-9._-]+$/.test(name);
}

export function lstatSafe(p: string): fs.Stats | null {
  try {
    return fs.lstatSync(p);
  } catch {
    return null;
  }
}

function verifyFrontmatterParity(
  fields: NonNullable<ValidationResult["fields"]>,
  manifest: SkillManifest,
): void {
  if (fields.name !== manifest.name) throw new Error(`SKILL.md name '${fields.name}' mismatches plugin.json '${manifest.name}'`);
  if (fields.version !== manifest.version) throw new Error(`SKILL.md version '${fields.version ?? ""}' mismatches plugin.json '${manifest.version}'`);
  if (fields.description !== manifest.description) throw new Error(`SKILL.md description mismatch for '${manifest.name}'`);
  const fmEntry = fields.entry_point ?? "SKILL.md";
  if (fmEntry !== manifest.entry_point) throw new Error(`SKILL.md entry_point '${fmEntry}' mismatches plugin.json '${manifest.entry_point}'`);
  if (fields.risk_tier !== manifest.risk_tier) throw new Error(`SKILL.md risk_tier '${fields.risk_tier ?? ""}' mismatches plugin.json ${manifest.risk_tier}`);
  const fmCaps = JSON.stringify([...(fields.capabilities_required ?? [])].sort());
  const manCaps = JSON.stringify([...(manifest.capabilities_required ?? [])].sort());
  if (fmCaps !== manCaps) throw new Error(`SKILL.md capabilities_required mismatch for '${manifest.name}'`);
}

function validateSkillEntryPoint(folder: string, entryPoint: string): void {
  const ep = entryPoint.trim();
  const segs = ep.split("/");
  if (!ep || ep.startsWith("/") || ep.endsWith("/") || ep.includes("\\") || /[\u0000-\u001F\u007F]/.test(ep) || segs.some((s) => !s || s === "." || s === "..")) {
    throw new Error(`unsafe skill entry_point '${entryPoint}'`);
  }
  if (ep === "SKILL.md") return;
  let cur = folder;
  for (let i = 0; i < segs.length; i++) {
    cur = path.join(cur, segs[i]);
    const st = lstatSafe(cur);
    if (!st || st.isSymbolicLink()) throw new Error(`entry_point '${ep}' missing or symlink rejected`);
    if (i + 1 === segs.length) {
      if (!st.isFile() || st.nlink !== 1) throw new Error(`entry_point '${ep}' must be regular non-hardlink file`);
    } else if (!st.isDirectory()) {
      throw new Error(`entry_point '${ep}' parent is not a directory`);
    }
  }
}

function validateSkillMetadataSafety(manifest: SkillManifest): void {
  assertNoYamlBreakout("manifest.name", manifest.name);
  assertNoYamlBreakout("manifest.description", manifest.description);
  scanSkillMarkdownSafety("manifest.name", manifest.name);
  scanSkillMarkdownSafety("manifest.description", manifest.description);
  if (manifest.prompt_overview) {
    assertNoYamlBreakout("manifest.prompt_overview", manifest.prompt_overview);
    scanSkillMarkdownSafety("manifest.prompt_overview", manifest.prompt_overview);
  }
}

function readSkillMdFields(folder: string): { fields?: ValidationResult["fields"]; skillMdSha256?: string } | null {
  const skillMdPath = path.join(folder, "SKILL.md");
  if (!lstatSafe(skillMdPath)) return {};
  try {
    const raw = readVerifiedRegularUtf8(skillMdPath);
    const validation = validateSkillFrontmatter(raw);
    if (!validation.valid || !validation.fields) return null;
    return { fields: validation.fields, skillMdSha256: sha256Hex(raw) };
  } catch (err) {
    console.warn(`[BuiltinSkillsRegistry] failed reading SKILL.md at ${path.basename(folder)}:`, err instanceof Error ? err.message : String(err));
    return null;
  }
}

export function loadSkillFolderWithHash(folder: string): { manifest: SkillManifest; skillMdSha256?: string } | null {
  const folderStat = lstatSafe(folder);
  if (!folderStat || folderStat.isSymbolicLink() || !folderStat.isDirectory()) return null;

  let verifiedRepackaged = false;
  try {
    if (lstatSafe(path.join(folder, "skills-lock.json")) || lstatSafe(path.join(folder, "discovery-stub.json"))) {
      verifyRepackagedBundle(folder);
      verifiedRepackaged = true;
    }
  } catch (err) {
    console.warn(`[BuiltinSkillsRegistry] repackaged bundle verification failed at ${path.basename(folder)}:`, err instanceof Error ? err.message : String(err));
    return null;
  }

  const mdInfo = readSkillMdFields(folder);
  if (mdInfo === null) return null;
  const { fields, skillMdSha256 } = mdInfo;

  const pluginJsonPath = path.join(folder, "plugin.json");
  if (lstatSafe(pluginJsonPath)) {
    try {
      const parsed = SkillManifestSchema.parse(JSON.parse(readVerifiedRegularUtf8(pluginJsonPath)));
      if (parsed.tags.includes("repackaged") && !verifiedRepackaged) verifyRepackagedBundle(folder);
      if (!isSafeSkillName(parsed.name)) return null;
      validateSkillMetadataSafety(parsed);
      validateSkillEntryPoint(folder, parsed.entry_point);
      if (fields) verifyFrontmatterParity(fields, parsed);
      return { manifest: parsed, skillMdSha256 };
    } catch (err) {
      console.warn(`[BuiltinSkillsRegistry] failed loading plugin.json at ${path.basename(folder)}:`, err instanceof Error ? err.message : String(err));
      return null;
    }
  }

  if (fields?.name && fields.description && isSafeSkillName(fields.name)) {
    const manifest: SkillManifest = {
      name: fields.name,
      version: fields.version || "1.0.0",
      description: fields.description,
      entry_point: fields.entry_point || "SKILL.md",
      capabilities_required: fields.capabilities_required ?? [],
      risk_tier: (fields.risk_tier || 1) as 1 | 2 | 3,
      tags: fields.tags ?? [],
    };
    try {
      validateSkillMetadataSafety(manifest);
      validateSkillEntryPoint(folder, manifest.entry_point);
    } catch (err) {
      console.warn(`[BuiltinSkillsRegistry] rejected SKILL.md metadata/entry_point at ${path.basename(folder)}:`, err instanceof Error ? err.message : String(err));
      return null;
    }
    return { manifest, skillMdSha256 };
  }
  return null;
}
