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
  const folderBaseName =
    typeof data.localFolderPath === "string" && data.localFolderPath.trim()
      ? data.localFolderPath.trim().replace(/[\\/]+$/, "").split(/[\\/]/).pop() || ""
      : "";
  const rawTitle = firstNonEmptyString(
    [folderBaseName, data.prdFileName, data.templateId, data.designUrl, data.gitRepoUrl],
    `cowok-${pathType}`
  );
  const cleanSlug = rawTitle
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, MAX_LAUNCHPAD_SLUG_LEN);
  const slugBase = cleanSlug || `cowok-${pathType}`;
  const brief = firstNonEmptyString(
    [data.prdText, data.clipboardSnippet, data.designUrl, data.gitRepoUrl, data.localFolderPath],
    `Initialized from ${pathType}`
  );

  return {
    slug: `${slugBase}-${Date.now().toString(SLUG_TIMESTAMP_RADIX)}`,
    name: rawTitle.slice(0, MAX_LAUNCHPAD_NAME_LEN) || "New Launchpad Workspace",
    product_brief: brief.slice(0, MAX_LAUNCHPAD_BRIEF_LEN),
    workspace_root: optionalStringField(data.localFolderPath),
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

interface CatalogAppPayload {
  slug?: string;
  name?: string;
  repoUrl?: string;
  category?: string;
  stars?: number;
  licenseSpdx?: string;
  firewallVerdict?: string;
  description?: string;
  layoutArchetype?: string;
}

function formatCatalogAppSpecMarkdown(app: CatalogAppPayload): string {
  const archetype = app.layoutArchetype ?? "workspace-dashboard";
  return [
    `# UI/UX Reference Contract — ${app.name}`,
    `- **Upstream Awesome-RE Repo:** ${app.repoUrl} (★ ${app.stars ?? 0})`,
    `- **Category:** \`${app.category ?? "App"}\` · **Layout Archetype:** \`${archetype}\``,
    `- **License:** \`${app.licenseSpdx ?? "MIT"}\` · **Firewall Verdict:** \`${app.firewallVerdict ?? "REFERENCE_ONLY"}\``,
    `\n## Architectural Reference Summary\n${app.description ?? ""}`,
    `\n## Keep (Layout & Interaction Grammar)\n- Follow the \`${archetype}\` spatial hierarchy and interaction ergonomics`,
    `\n## Change (Project Brand & Domain)\n- Replace visual palette, typography, and copy with user-defined brand tokens and custom themeModes`,
    `\n## Do Not Copy (IP & Brand Boundary)\n- Never copy proprietary trademarks, logos, icons, or verbatim product copy`,
  ].join("\n");
}

function buildReferenceSpecMarkdown(data: Record<string, unknown>): string | null {
  const app = data.catalogApp as CatalogAppPayload | null | undefined;
  if (app && typeof app.name === "string" && typeof app.repoUrl === "string") {
    return formatCatalogAppSpecMarkdown(app);
  }
  const ingested = data.reaIngestOutcome as {
    record?: { slug?: string; sourceUrl?: string; licenseSpdx?: string; tier1aSummary?: string };
    firewall?: { verdict?: string };
  } | null | undefined;
  if (ingested?.record?.slug) {
    return [
      `# UI/UX Reference Contract — ${ingested.record.slug}`,
      `- **Source URL:** \`${ingested.record.sourceUrl ?? "design-tool"}\` · **License:** \`${ingested.record.licenseSpdx ?? "MIT"}\` · **Firewall Verdict:** \`${ingested.firewall?.verdict ?? "REFERENCE_ONLY"}\``,
      `\n## Tier-1A Design DNA Summary\n${ingested.record.tier1aSummary ?? ""}`,
    ].join("\n");
  }
  const reaRef = data.reaReference as { slug?: string; sourceUrl?: string; firewallVerdict?: string; tier1aSummary?: string } | null | undefined;
  if (reaRef && typeof reaRef.slug === "string") {
    return [
      `# UI/UX Reference Contract — ${reaRef.slug}`,
      `- **Source:** \`${reaRef.sourceUrl ?? "local-rea-cache"}\` · **Firewall Verdict:** \`${reaRef.firewallVerdict ?? "REFERENCE_ONLY"}\``,
      `\n## Tier-1A Design DNA Summary\n${reaRef.tier1aSummary ?? ""}`,
    ].join("\n");
  }
  return null;
}

async function seedReferenceUiUxSpec(projectId: string, data: Record<string, unknown>): Promise<void> {
  if (!UUID_RE.test(projectId)) return;
  const markdown = buildReferenceSpecMarkdown(data);
  if (!markdown) return;
  try {
    await apiHarnessDaemon.putSpec(projectId, "03-ui-ux-spec.md", {
      markdown_content: markdown,
      change_summary: "Seeded UI/UX Reference Contract from Launchpad awesome-re / rea library",
    });
  } catch {
    // Non-blocking: project creation still succeeds and Chat Agent receives product_brief fallback
  }
}

export async function applyLaunchpadMaterialToProject(
  projectId: string,
  entryKind: "fe-dag" | "presentation",
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>
): Promise<void> {
  const isDeck = pathType === "templates" && (data.templateId === "tpl-slide-deck" || data.templateId === "awesome-re:executive-keynote-16-9");
  const resolvedKind: "fe-dag" | "presentation" = isDeck ? "presentation" : entryKind;
  const briefSummary = summarizeLaunchpadMaterial(resolvedKind, pathType, data);
  try {
    if (UUID_RE.test(projectId)) {
      await apiHarnessDaemon.updateProject(projectId, { product_brief: briefSummary });
      await seedReferenceUiUxSpec(projectId, data);
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
    await seedReferenceUiUxSpec(created.id, data);
    const isDeck = pathType === "templates" && (data.templateId === "tpl-slide-deck" || data.templateId === "awesome-re:executive-keynote-16-9");
    const entryKind = isDeck ? "presentation" : "fe-dag";
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
