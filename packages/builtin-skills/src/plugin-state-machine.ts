// @story 08.3
// @implements AC-01, AC-03, AC-05
import type { SkillPlugin } from "@cowok-design/shared-types";
import { detectBreakingChange, stableJsonEqual } from "./breaking-diff";
import { nowUnixSeconds } from "./learn-sanitizer";
import { ManifestUpdateOutcome, PluginLifecycleError } from "./lifecycle-errors";
import { ManagedPluginRecord, ManagedPluginRecordSchema, PluginState, PluginStateSchema, StagedPluginBundle, StagedPluginBundleSchema } from "./lifecycle-types";
import {
  assertManifestContract,
  derivePluginSlug,
  ManifestLike,
  normalizeManifest,
  parseLifecycleManifest,
  validatePersistedPlugin,
} from "./plugin-persisted-validator";
import { validateStagedBundleSecurity } from "./staging-security";

const ALLOWED_TRANSITIONS: Record<PluginState, ReadonlySet<PluginState>> = {
  DISCOVERED: new Set(["VALIDATED", "ARCHIVED"]),
  VALIDATED: new Set(["INSTALLED", "ARCHIVED"]),
  INSTALLED: new Set(["ACTIVE", "ARCHIVED"]),
  ACTIVE: new Set(["DEPRECATED", "ARCHIVED"]),
  DEPRECATED: new Set(["ACTIVE", "ARCHIVED"]),
  ARCHIVED: new Set(),
};

/** Deterministic 6-state plugin lifecycle machine with CAS revisioning, quarantine, rehydration, and staging isolation. */
export class PluginStateMachine {
  private plugin: ManagedPluginRecord;

  constructor(id: string, name: string, version: string, manifestOverrides?: Partial<ManifestLike>) {
    const cleanId = id.trim();
    const fallbackName = name.trim();
    const manifest = normalizeManifest(
      parseLifecycleManifest({
        description: fallbackName ? `Managed plugin ${fallbackName}` : "",
        entry_point: "SKILL.md",
        capabilities_required: [],
        risk_tier: 2,
        parameters: null,
        tags: [],
        prompt_overview: null,
        ...manifestOverrides,
        name: (manifestOverrides?.name ?? fallbackName).trim(),
        version,
      })
    );
    assertManifestContract(cleanId, manifest);
    this.plugin = {
      id: cleanId,
      tenant_id: "local-default",
      project_id: "local-default",
      plugin_slug: derivePluginSlug(manifest.name),
      name: manifest.name,
      version: manifest.version,
      state: "DISCOVERED",
      revision: 1,
      breaking_change_isolated: false,
      staging_version: null,
      staging_manifest: null,
      staging_diff_reasons: [],
      manifest,
      quarantined: false,
      quarantine_reason: null,
      updated_at: nowUnixSeconds(),
    };
  }

  public static withManifest(id: string, manifest: ManifestLike): PluginStateMachine {
    const parsed = parseLifecycleManifest(manifest);
    return new PluginStateMachine(id, parsed.name, parsed.version, parsed);
  }

  public static withScope(tenantId: string, projectId: string, id: string, pluginSlug: string, manifest: ManifestLike): PluginStateMachine {
    const sm = PluginStateMachine.withManifest(id, manifest);
    sm.plugin = validatePersistedPlugin({ ...sm.snapshot, tenant_id: tenantId, project_id: projectId, plugin_slug: pluginSlug });
    return sm;
  }

  public static fromPersisted(record: ManagedPluginRecord): PluginStateMachine {
    const validated = validatePersistedPlugin(ManagedPluginRecordSchema.parse(record));
    const sm = PluginStateMachine.withManifest(validated.id, validated.manifest);
    sm.plugin = validated;
    return sm;
  }

  public get snapshot(): ManagedPluginRecord {
    return structuredClone(this.plugin);
  }

  public toDomainPlugin(): SkillPlugin {
    return {
      id: this.plugin.id,
      name: this.plugin.manifest.name,
      version: this.plugin.version,
      entry_point: this.plugin.manifest.entry_point,
      permissions: [...this.plugin.manifest.capabilities_required],
    };
  }

  private assertRevision(expectedRevision: number): void {
    if (!Number.isSafeInteger(expectedRevision) || expectedRevision !== this.plugin.revision) {
      throw new PluginLifecycleError("RevisionConflict", `expected revision ${expectedRevision}, actual ${this.plugin.revision}`);
    }
  }

  private clearStaging(): void {
    this.plugin.breaking_change_isolated = false;
    this.plugin.staging_version = null;
    this.plugin.staging_manifest = null;
    this.plugin.staging_diff_reasons = [];
  }

  public transitionToChecked(nextRaw: string, expectedRevision: number): void {
    const parsed = PluginStateSchema.safeParse(nextRaw);
    if (!parsed.success) throw new PluginLifecycleError("UnknownPluginState", `Unknown plugin state '${nextRaw}'`);
    this.assertRevision(expectedRevision);
    const next = parsed.data;
    if (this.plugin.quarantined && next !== "ARCHIVED") {
      throw new PluginLifecycleError("QuarantinedReactivationForbidden", this.plugin.quarantine_reason ?? "quarantined");
    }
    if (!ALLOWED_TRANSITIONS[this.plugin.state].has(next)) {
      throw new PluginLifecycleError("InvalidTransition", `Invalid state transition from ${this.plugin.state} to ${next}`, { from: this.plugin.state, to: next });
    }
    if (this.plugin.state === "ACTIVE" && next !== "ACTIVE") this.clearStaging();
    this.plugin.state = next;
    this.plugin.revision += 1;
    this.plugin.updated_at = nowUnixSeconds();
  }

  public quarantine(reason: string, expectedRevision: number): void {
    this.assertRevision(expectedRevision);
    const trimmed = reason.trim();
    if (!trimmed) throw new PluginLifecycleError("InvalidQuarantineReason", "quarantine reason must not be empty");
    this.plugin.quarantined = true;
    this.plugin.quarantine_reason = trimmed;
    this.clearStaging();
    if (this.plugin.state === "ACTIVE") this.plugin.state = "DEPRECATED";
    this.plugin.revision += 1;
    this.plugin.updated_at = nowUnixSeconds();
  }

  public applyManifestUpdate(candidateRaw: ManifestLike, expectedRevision: number, forceBreakingHint = false): ManifestUpdateOutcome {
    this.assertRevision(expectedRevision);
    if (this.plugin.quarantined) throw new PluginLifecycleError("QuarantinedReactivationForbidden", this.plugin.quarantine_reason ?? "quarantined");
    if (this.plugin.state === "ARCHIVED") throw new PluginLifecycleError("InvalidTransition", "Invalid state transition from ARCHIVED to ARCHIVED");
    const candidate = normalizeManifest(parseLifecycleManifest(candidateRaw));
    assertManifestContract(this.plugin.id, candidate);

    if (this.plugin.breaking_change_isolated) {
      if (this.plugin.staging_manifest && stableJsonEqual(this.plugin.staging_manifest, candidate)) {
        return { outcome: "IDEMPOTENT_STAGED", active_version: this.plugin.version, candidate_version: candidate.version, reasons: [...this.plugin.staging_diff_reasons] };
      }
      throw new PluginLifecycleError("PendingStagingMustBeResolved", "Pending staging candidate must be promoted or discarded before submitting another update");
    }

    const report = detectBreakingChange(this.plugin.manifest, candidate, forceBreakingHint);
    if (report.is_breaking) {
      if (this.plugin.state !== "ACTIVE") {
        throw new PluginLifecycleError("NonActiveBreakingUpdateRejected", `State ${this.plugin.state} cannot apply breaking update or downgrade (${report.reasons.join("; ")})`);
      }
      this.plugin.breaking_change_isolated = true;
      this.plugin.staging_version = candidate.version;
      this.plugin.staging_manifest = candidate;
      this.plugin.staging_diff_reasons = [...report.reasons];
      this.plugin.revision += 1;
      this.plugin.updated_at = nowUnixSeconds();
      return { outcome: "STAGED_FOR_REVIEW", active_version: this.plugin.version, candidate_version: candidate.version, reasons: report.reasons, report };
    }

    this.plugin.name = candidate.name;
    this.plugin.version = candidate.version;
    this.plugin.manifest = candidate;
    this.plugin.revision += 1;
    this.plugin.updated_at = nowUnixSeconds();
    return { outcome: "APPLIED", report };
  }

  public promoteStaging(bundleRaw: StagedPluginBundle, expectedRevision: number): void {
    this.assertRevision(expectedRevision);
    if (this.plugin.quarantined) throw new PluginLifecycleError("QuarantinedReactivationForbidden", this.plugin.quarantine_reason ?? "quarantined");
    if (!this.plugin.breaking_change_isolated || this.plugin.state !== "ACTIVE" || !this.plugin.staging_manifest) {
      throw new PluginLifecycleError("NoStagedCandidate", "no staged breaking-change candidate available");
    }
    const normalizedManifest = normalizeManifest(parseLifecycleManifest((bundleRaw as unknown as Record<string, unknown>).manifest));
    const bundle = StagedPluginBundleSchema.parse({ ...bundleRaw, manifest: normalizedManifest });
    if (!stableJsonEqual(this.plugin.staging_manifest, bundle.manifest)) {
      throw new PluginLifecycleError("StagingPromotionRejected", "Staged bundle manifest does not match isolated candidate");
    }
    validateStagedBundleSecurity(bundle);
    this.plugin.name = bundle.manifest.name;
    this.plugin.version = bundle.manifest.version;
    this.plugin.manifest = bundle.manifest;
    this.clearStaging();
    this.plugin.revision += 1;
    this.plugin.updated_at = nowUnixSeconds();
  }

  public discardStaging(expectedRevision: number): void {
    this.assertRevision(expectedRevision);
    if (!this.plugin.breaking_change_isolated) throw new PluginLifecycleError("NoStagedCandidate", "no staged candidate available");
    this.clearStaging();
    this.plugin.revision += 1;
    this.plugin.updated_at = nowUnixSeconds();
  }
}
