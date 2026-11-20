// @story 05.1
// @implements AC-03, AC-04 (4-Path Launchpad Creation & Sample Data Studio Persistence)
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";
import type { SampleDataPreset } from "../components/SampleDataStudioModal";

const ENTRY_PATH_BY_LAUNCHPAD = {
  prd: "prompt_prd",
  templates: "starter_template",
  design: "design_import",
  git: "git_codebase",
} as const;

const MAX_LAUNCHPAD_SLUG_LEN = 48;
const MAX_LAUNCHPAD_NAME_LEN = 80;
const MAX_LAUNCHPAD_BRIEF_LEN = 2000;
const SLUG_TIMESTAMP_RADIX = 36;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const VARIANT_BY_PRESET: Record<SampleDataPreset, FixtureVariant> = {
  HAPPY: "DEFAULT",
  EMPTY: "EMPTY",
  LONG_TEXT: "DEFAULT",
  VALIDATION_ERROR: "ERROR",
};

function firstNonEmptyString(values: unknown[], fallback: string): string {
  for (const v of values) {
    if (typeof v === "string" && v.trim().length > 0) return v.trim();
  }
  return fallback;
}

function optionalStringField(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function buildLaunchpadRequest(
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>
) {
  const rawTitle = firstNonEmptyString(
    [data.prdFileName, data.templateId, data.designUrl, data.gitRepoUrl],
    `cowok-${pathType}`
  );
  const cleanSlug = rawTitle
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, MAX_LAUNCHPAD_SLUG_LEN);
  const slugBase = cleanSlug || `cowok-${pathType}`;
  const brief = firstNonEmptyString(
    [data.prdText, data.clipboardSnippet, data.designUrl, data.gitRepoUrl],
    `Initialized from ${pathType}`
  );

  return {
    slug: `${slugBase}-${Date.now().toString(SLUG_TIMESTAMP_RADIX)}`,
    name: rawTitle.slice(0, MAX_LAUNCHPAD_NAME_LEN) || "New Launchpad Workspace",
    product_brief: brief.slice(0, MAX_LAUNCHPAD_BRIEF_LEN),
    provenance: "scratch" as const,
    entryPath: ENTRY_PATH_BY_LAUNCHPAD[pathType],
    templateId: typeof data.templateId === "string" ? data.templateId : undefined,
    designSourceUrl: optionalStringField(data.designUrl),
    gitRepoUrl: optionalStringField(data.gitRepoUrl),
    gitBranch: optionalStringField(data.gitBranch),
  };
}

function summarizeLaunchpadMaterial(
  entryKind: "fe-dag" | "presentation",
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>
): string {
  const snippet = firstNonEmptyString(
    [data.prdText, data.clipboardSnippet, data.designUrl, data.gitRepoUrl, data.templateId],
    ""
  );
  const label = ENTRY_PATH_BY_LAUNCHPAD[pathType];
  return snippet
    ? `[${entryKind}:${label}] ${snippet.slice(0, MAX_LAUNCHPAD_BRIEF_LEN)}`
    : `[${entryKind}:${label}] Initialized workspace material`;
}

export async function applyLaunchpadMaterialToProject(
  projectId: string,
  entryKind: "fe-dag" | "presentation",
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>
): Promise<void> {
  const resolvedKind: "fe-dag" | "presentation" =
    pathType === "templates" && data.templateId === "tpl-slide-deck" ? "presentation" : entryKind;
  const briefSummary = summarizeLaunchpadMaterial(resolvedKind, pathType, data);
  try {
    if (UUID_RE.test(projectId)) {
      await apiHarnessDaemon.updateProject(projectId, { product_brief: briefSummary });
    }
    if (pathType === "git" && typeof data.gitRepoUrl === "string" && data.gitRepoUrl.trim() && UUID_RE.test(projectId)) {
      await apiVersionSnapshot.bindGitRemote(projectId, data.gitRepoUrl.trim());
    }
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("cowok:chat-receipt", {
        detail: { projectId, text: briefSummary },
      }));
    }
  } catch (err) {
    if (typeof window !== "undefined") {
      const message = err instanceof Error ? err.message : "";
      window.dispatchEvent(new CustomEvent("cowok:sample-fixture-error", { detail: { message } }));
    }
  }
}

export async function submitLaunchpadProject(
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>
): Promise<void> {
  try {
    const created = await apiHarnessDaemon.createProjectFromLaunchpad(
      buildLaunchpadRequest(pathType, data)
    );
    const entryKind = pathType === "templates" && data.templateId === "tpl-slide-deck" ? "presentation" : "fe-dag";
    window.dispatchEvent(new CustomEvent("cowok:project-created", { detail: { ...created, entryKind } }));
  } catch (err) {
    const message = err instanceof Error ? err.message : "";
    window.dispatchEvent(new CustomEvent("cowok:project-create-error", { detail: { message } }));
  }
}

export async function applyStudioSampleFixture(
  projectId: string | undefined,
  nodeId: string | null | undefined,
  preset: SampleDataPreset,
  json: string
): Promise<void> {
  const variant = VARIANT_BY_PRESET[preset] || "DEFAULT";
  window.dispatchEvent(new CustomEvent("cowok:sample-fixture-applied", {
    detail: { projectId, nodeId, preset, variant, fixtureJson: json },
  }));
  if (!projectId || !nodeId || !UUID_RE.test(projectId)) return;
  try {
    await apiScreenFixture.saveFixture(projectId, nodeId, {
      variant,
      payloadJson: json,
      presetKey: preset,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "";
    window.dispatchEvent(new CustomEvent("cowok:sample-fixture-error", { detail: { message } }));
  }
}
