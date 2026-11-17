// @story 08.3
// @implements AC-05
import { PluginLifecycleError } from "./lifecycle-errors";
import type { StagedPluginBundle } from "./lifecycle-types";
import { scanExternalScriptSecurity, scanSkillMarkdownSafety } from "./repackager-security";
import { validateSkillFrontmatter } from "./validator";

const RESERVED_ROOT_NAMES = new Set(["skill.md", "plugin.json", "discovery-stub.json", "skills-lock.json"]);
const SCRIPT_EXTENSIONS = new Set(["py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "sh", "bash", "zsh", "rb", "pl"]);
const UNSAFE_PATH_CTRL = /[\u0000-\u001F\u007F-\u009F\u200B-\u200F\u2028-\u202E\u2066-\u2069\uFEFF\u{E0001}-\u{E007F}]/u;

/** Validates staged script relative paths against traversal, controls, bidi, reserved files, and script extensions. */
export function validateSafeRelScriptPath(relPath: string): void {
  const badSegment = relPath.split("/").some((s) => !s || s === "." || s === "..");
  if (!relPath || relPath.startsWith("/") || relPath.includes("\\") || UNSAFE_PATH_CTRL.test(relPath) || badSegment) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Unsafe staged script path '${relPath}'`);
  }
  const base = (relPath.split("/").pop() ?? "").toLowerCase();
  if (RESERVED_ROOT_NAMES.has(base)) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Reserved bundle filename disallowed in scripts: '${relPath}'`);
  }
  const dot = base.lastIndexOf(".");
  const ext = dot >= 0 ? base.slice(dot + 1) : "";
  if (!SCRIPT_EXTENSIONS.has(ext)) {
    throw new PluginLifecycleError("StagingPromotionRejected", `Unsupported staged script extension on '${relPath}'`);
  }
}

/** Validates promoted staging SKILL.md frontmatter, IPI safety, and script security (AC-05, EC-E08-10). */
export function validateStagedBundleSecurity(bundle: StagedPluginBundle): void {
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
  } catch (err) {
    throw new PluginLifecycleError("StagingPromotionRejected", `SKILL.md safety failed: ${err instanceof Error ? err.message : String(err)}`);
  }
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
}
