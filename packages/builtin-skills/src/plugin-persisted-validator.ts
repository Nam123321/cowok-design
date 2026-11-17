// @story 08.3
// @implements AC-01, AC-03, AC-05
import { parseSemVerStrict } from "./breaking-diff";
import { PluginLifecycleError } from "./lifecycle-errors";
import { ManagedPluginRecord } from "./lifecycle-types";
import { assertNoYamlBreakout } from "./learn-sanitizer";
import { scanSkillMarkdownSafety } from "./repackager-security";
import { SkillManifest, SkillManifestSchema } from "./types";

export type LifecycleManifest = ManagedPluginRecord["manifest"];

export const derivePluginSlug = (name: string): string =>
  name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "unnamed-plugin";

export function normalizeManifest(manifest: LifecycleManifest): LifecycleManifest {
  return { ...manifest, name: manifest.name.trim(), version: manifest.version.trim(), description: manifest.description.trim(), entry_point: manifest.entry_point.trim() };
}

export function parseLifecycleManifest(raw: unknown): LifecycleManifest {
  const base = SkillManifestSchema.parse(raw);
  const p = base.parameters ?? null;
  if (p !== null && typeof p !== "boolean" && (typeof p !== "object" || Array.isArray(p))) {
    throw new PluginLifecycleError("InvalidManifestField", "Lifecycle manifest parameters must be a JSON object, boolean, or null");
  }
  return { ...base, parameters: p as Record<string, unknown> | boolean | null, prompt_overview: base.prompt_overview ?? null };
}

export function assertManifestContract(id: string, manifest: LifecycleManifest): void {
  if (!id.trim() || !manifest.name.trim() || !manifest.description.trim() || !manifest.entry_point.trim()) {
    throw new PluginLifecycleError("InvalidManifestField", "id, name, description, and entry_point must not be empty");
  }
  try {
    assertNoYamlBreakout("manifest.name", manifest.name);
    assertNoYamlBreakout("manifest.description", manifest.description);
    scanSkillMarkdownSafety("manifest.name", manifest.name);
    scanSkillMarkdownSafety("manifest.description", manifest.description);
    if (manifest.prompt_overview) {
      assertNoYamlBreakout("manifest.prompt_overview", manifest.prompt_overview);
      scanSkillMarkdownSafety("manifest.prompt_overview", manifest.prompt_overview);
    }
  } catch (err) {
    throw new PluginLifecycleError("InvalidManifestField", err instanceof Error ? err.message : String(err));
  }
  parseSemVerStrict(manifest.version);
}

export function validatePersistedPlugin(rec: ManagedPluginRecord): ManagedPluginRecord {
  const manifest = normalizeManifest(parseLifecycleManifest(rec.manifest));
  assertManifestContract(rec.id, manifest);
  if (!rec.tenant_id.trim() || !rec.project_id.trim() || rec.version.trim() !== manifest.version || rec.name.trim() !== manifest.name || rec.revision < 1) {
    throw new PluginLifecycleError("InvalidPersistedState", "Persisted scope, version, name, or revision violates invariant");
  }
  if (rec.quarantined && (rec.state === "ACTIVE" || !rec.quarantine_reason?.trim() || rec.breaking_change_isolated || rec.staging_manifest !== null)) {
    throw new PluginLifecycleError("InvalidPersistedState", "Quarantined plugin violates non-ACTIVE, reason, or clean-staging invariant");
  }
  const cleanStaging = rec.staging_manifest ? normalizeManifest(parseLifecycleManifest(rec.staging_manifest)) : null;
  if (cleanStaging) {
    assertManifestContract(rec.id, cleanStaging);
    if (!rec.breaking_change_isolated || rec.state !== "ACTIVE" || rec.staging_version !== cleanStaging.version) {
      throw new PluginLifecycleError("InvalidPersistedState", "Staged manifest requires ACTIVE state and matching staging_version");
    }
  } else if (rec.breaking_change_isolated || rec.staging_version !== null) {
    throw new PluginLifecycleError("InvalidPersistedState", "Isolated staging flags set without staging_manifest");
  }
  return {
    ...rec,
    id: rec.id.trim(),
    tenant_id: rec.tenant_id.trim(),
    project_id: rec.project_id.trim(),
    plugin_slug: rec.plugin_slug.trim() || derivePluginSlug(manifest.name),
    name: manifest.name,
    version: manifest.version,
    manifest,
    staging_manifest: cleanStaging,
    quarantine_reason: rec.quarantine_reason?.trim() ?? null,
  };
}

export type ManifestLike = LifecycleManifest | SkillManifest;
