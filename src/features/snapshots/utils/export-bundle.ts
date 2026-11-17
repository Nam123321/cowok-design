// @story 05.3
import {
  type ExportZipInput,
  type ExportZipResponse,
  type SnapshotFileItem,
} from "../../../lib/validations/version-snapshot";
import { computeTreeSha256 } from "./snapshot-engine";
import { createZipArchive } from "./zip-writer";

export { createZipArchive };

export interface BundleCompletenessCheck {
  hasTokens: boolean;
  hasIcons: boolean;
  hasPackageJson: boolean;
  hasFixtures: boolean;
  hasSource: boolean;
  hasHandoffManifest: boolean;
  readyToRun: boolean;
  missingItems: string[];
}

function hasBuildScript(content: string): boolean {
  try {
    const parsed = JSON.parse(content) as { scripts?: { build?: unknown } };
    return typeof parsed?.scripts?.build === "string" && parsed.scripts.build.trim().length > 0;
  } catch {
    return false;
  }
}

export function checkExportCompleteness(files: SnapshotFileItem[]): BundleCompletenessCheck {
  const paths = files.map((f) => f.path.toLowerCase());
  const hasTokens = paths.some((p) => p.endsWith("tokens.css"));
  const hasIcons = paths.some((p) => p.endsWith("icon-manifest.json"));
  const pkgFile = files.find((f) => f.path === "package.json" || f.path.endsWith("/package.json"));
  const hasPackageJson = Boolean(pkgFile);
  const hasBuild = pkgFile ? hasBuildScript(pkgFile.content) : false;
  const hasFixtures = paths.some((p) => p.includes("fixture"));
  const hasSource = paths.some(
    (p) =>
      p.includes("src/") &&
      (p.endsWith(".tsx") || p.endsWith(".jsx") || p.endsWith(".ts") || p.endsWith(".js"))
  );
  const hasHandoffManifest = paths.some(
    (p) => p === "cowok-handoff-manifest.json" || p.endsWith("/cowok-handoff-manifest.json")
  );

  const missingItems: string[] = [];
  if (!hasTokens) missingItems.push("tokens.css");
  if (!hasIcons) missingItems.push("icon-manifest.json");
  if (!hasPackageJson) missingItems.push("package.json");
  if (hasPackageJson && !hasBuild) missingItems.push("package.json scripts.build");
  if (!hasFixtures) missingItems.push("screen fixtures");
  if (!hasSource) missingItems.push("source code (src/*.tsx)");
  if (!hasHandoffManifest) missingItems.push("cowok-handoff-manifest.json");

  const readyToRun =
    hasTokens &&
    hasIcons &&
    hasPackageJson &&
    hasBuild &&
    hasFixtures &&
    hasSource &&
    hasHandoffManifest;

  return {
    hasTokens,
    hasIcons,
    hasPackageJson,
    hasFixtures,
    hasSource,
    hasHandoffManifest,
    readyToRun,
    missingItems,
  };
}

export function buildExportZipMetadata(
  projectId: string,
  files: SnapshotFileItem[],
  options: ExportZipInput = {
    includeFixtures: true,
    includeTokens: true,
    includeIcons: true,
    format: "zip",
  },
  tenantId: string = "default-tenant"
): ExportZipResponse {
  const filteredFiles = files.filter((f) => {
    const p = f.path.toLowerCase();
    if (!options.includeFixtures && p.includes("fixture")) return false;
    if (!options.includeTokens && p.endsWith("tokens.css")) return false;
    if (!options.includeIcons && p.endsWith("icon-manifest.json")) return false;
    return true;
  });

  const completeness = checkExportCompleteness(filteredFiles);
  const totalBytes = filteredFiles.reduce((acc, f) => acc + (f.content.length || 0), 0);
  const bundleSha256 = computeTreeSha256(filteredFiles);
  const dateStr = new Date().toISOString().slice(0, 10).replace(/-/g, "");

  return {
    fileName: `${projectId}-${tenantId}-bundle-${dateStr}.zip`,
    byteLength: totalBytes,
    sha256: bundleSha256,
    filesIncluded: filteredFiles.map((f) => f.path),
    readyToRun: completeness.readyToRun,
    handoffManifestIncluded: completeness.hasHandoffManifest,
  };
}
