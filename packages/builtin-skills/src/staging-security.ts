// @story 08.3
// @implements AC-05
import { PluginLifecycleError } from "./lifecycle-errors";
import type { StagedPluginBundle } from "./lifecycle-types";
import { classifyAndValidateSourceAsset, scanExternalScriptSecurity, scanSkillMarkdownSafety } from "./repackager-security";
import { validateSkillFrontmatter } from "./validator";

const RESERVED_ROOT_NAMES = new Set(["skill.md", "plugin.json", "discovery-stub.json", "skills-lock.json"]);
const SCRIPT_EXTENSIONS = new Set(["py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "sh", "bash", "zsh", "rb", "pl"]);
const UNSAFE_PATH_CTRL = /[\u0000-\u001F\u007F-\u009F\u200B-\u200F\u2028-\u202E\u2066-\u2069\uFEFF\u{E0001}-\u{E007F}]/u;

export type ExtendedStagedBundle = Omit<StagedPluginBundle, "assets"> & {
  assets?: Array<{ rel_path: string; bytes: number[] | Uint8Array }>;
};

function validateSafeRelBundlePath(relPath: string, kind: string): string {
  const badSegment = relPath.split("/").some((s) => !s || s === "." || s === "..");
  if (!relPath || relPath.startsWith("/") || relPath.includes("\\") || UNSAFE_PATH_CTRL.test(relPath) || badSegment) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Unsafe staged ${kind} path '${relPath}'`);
  }
  const base = (relPath.split("/").pop() ?? "").toLowerCase();
  if (RESERVED_ROOT_NAMES.has(base)) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Reserved bundle filename disallowed in ${kind}: '${relPath}'`);
  }
  return base;
}

/** Validates staged script relative paths against traversal, controls, bidi, reserved files, and script extensions. */
export function validateSafeRelScriptPath(relPath: string): void {
  const base = validateSafeRelBundlePath(relPath, "script");
  const dot = base.lastIndexOf(".");
  const ext = dot >= 0 ? base.slice(dot + 1) : "";
  if (!SCRIPT_EXTENSIONS.has(ext)) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Unsupported staged script extension on '${relPath}'`);
  }
}

function validateStagedReferences(references: NonNullable<ExtendedStagedBundle["references"]>): void {
  const seen = new Set<string>();
  for (const ref of references) {
    validateSafeRelBundlePath(ref.rel_path, "reference");
    if (seen.has(ref.rel_path)) {
      throw new PluginLifecycleError("StagingPromotionRejected", `Duplicate staged reference path '${ref.rel_path}'`);
    }
    seen.add(ref.rel_path);
    const bucket = classifyAndValidateSourceAsset(ref.rel_path, Buffer.from(ref.content, "utf8"));
    if (bucket !== "references") {
      throw new PluginLifecycleError("StagingPromotionRejected", `Staged reference '${ref.rel_path}' is not a valid reference document`);
    }
    scanSkillMarkdownSafety(ref.rel_path, ref.content);
  }
}

function validateStagedAssets(assets: NonNullable<ExtendedStagedBundle["assets"]>): void {
  const seen = new Set<string>();
  for (const asset of assets) {
    validateSafeRelBundlePath(asset.rel_path, "asset");
    if (seen.has(asset.rel_path)) {
      throw new PluginLifecycleError("StagingPromotionRejected", `Duplicate staged asset path '${asset.rel_path}'`);
    }
    seen.add(asset.rel_path);
    const raw = Buffer.from(asset.bytes);
    const bucket = classifyAndValidateSourceAsset(asset.rel_path, raw);
    if (bucket === "assets_text") {
      scanSkillMarkdownSafety(asset.rel_path, raw.toString("utf8"));
    } else if (bucket !== "assets_image") {
      throw new PluginLifecycleError("StagingPromotionRejected", `Staged asset '${asset.rel_path}' is not a valid asset file`);
    }
  }
}

/** Validates promoted staging SKILL.md frontmatter, IPI safety, and script/reference/asset security (AC-05, EC-E08-10). */
export function validateStagedBundleSecurity(bundle: ExtendedStagedBundle): void {
  const fm = validateSkillFrontmatter(bundle.skill_md);
  if (!fm.valid || !fm.fields) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Invalid SKILL.md: ${fm.error ?? "unknown error"}`);
  }
  const fmCaps = JSON.stringify([...(fm.fields.capabilities_required ?? [])].sort());
  const manCaps = JSON.stringify([...(bundle.manifest.capabilities_required ?? [])].sort());
  const fmEntry = fm.fields.entry_point ?? "SKILL.md";
  const matchesIdentity =
    fm.fields.name === bundle.manifest.name &&
    fm.fields.version === bundle.manifest.version &&
    fm.fields.description === bundle.manifest.description &&
    fmEntry === bundle.manifest.entry_point &&
    fm.fields.risk_tier === bundle.manifest.risk_tier &&
    fmCaps === manCaps;
  if (!matchesIdentity) {
    throw new PluginLifecycleError("StagingPromotionRejected", "SKILL.md frontmatter diverges from staged manifest (identity, entry_point, risk_tier, or capabilities)");
  }
  try {
    scanSkillMarkdownSafety("SKILL.md", bundle.skill_md);
    const seen = new Set<string>();
    for (const script of bundle.scripts) {
      validateSafeRelScriptPath(script.rel_path);
      if (seen.has(script.rel_path)) {
        throw new PluginLifecycleError("StagingPromotionRejected", `Duplicate staged script path '${script.rel_path}'`);
      }
      seen.add(script.rel_path);
      const res = scanExternalScriptSecurity(script.rel_path, script.content);
      if (!res.valid) {
        throw new PluginLifecycleError("StagingPromotionRejected", `Script '${script.rel_path}' rejected: ${res.error}`);
      }
    }
    if (bundle.manifest.entry_point !== "SKILL.md" && !seen.has(bundle.manifest.entry_point)) {
      throw new PluginLifecycleError("StagingPromotionRejected", `Manifest entry_point '${bundle.manifest.entry_point}' missing from verified staged scripts`);
    }
    if (bundle.references) validateStagedReferences(bundle.references);
    if (bundle.assets) validateStagedAssets(bundle.assets);
  } catch (err) {
    if (err instanceof PluginLifecycleError) throw err;
    throw new PluginLifecycleError("StagingPromotionRejected", err instanceof Error ? err.message : String(err));
  }
}
