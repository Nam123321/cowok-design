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

const JS_TS_FILE_EXTS = [".tsx", ".jsx", ".ts", ".js"];

function isRunnableSourcePath(lowerPath: string): boolean {
  return lowerPath.includes("src/") && JS_TS_FILE_EXTS.some((ext) => lowerPath.endsWith(ext));
}

function findPackageJson(files: SnapshotFileItem[]): SnapshotFileItem | undefined {
  return files.find((f) => f.path === "package.json" || f.path.endsWith("/package.json"));
}

export function checkExportCompleteness(files: SnapshotFileItem[]): BundleCompletenessCheck {
  const paths = files.map((f) => f.path.toLowerCase());
  const pkgFile = findPackageJson(files);
  const hasPackageJson = Boolean(pkgFile);
  const hasBuild = pkgFile ? hasBuildScript(pkgFile.content) : false;

  const checks: Array<[keyof Omit<BundleCompletenessCheck, "readyToRun" | "missingItems">, boolean, string]> = [
    ["hasTokens", paths.some((p) => p.endsWith("tokens.css")), "tokens.css"],
    ["hasIcons", paths.some((p) => p.endsWith("icon-manifest.json")), "icon-manifest.json"],
    ["hasPackageJson", hasPackageJson, "package.json"],
    ["hasFixtures", paths.some((p) => p.includes("fixture")), "screen fixtures"],
    ["hasSource", paths.some(isRunnableSourcePath), "source code (src/*.tsx)"],
    ["hasHandoffManifest", paths.some((p) => p === "cowok-handoff-manifest.json" || p.endsWith("/cowok-handoff-manifest.json")), "cowok-handoff-manifest.json"],
  ];

  const missingItems = checks.filter(([, ok]) => !ok).map(([, , label]) => label);
  if (hasPackageJson && !hasBuild) missingItems.push("package.json scripts.build");

  const flags = Object.fromEntries(checks.map(([k, ok]) => [k, ok])) as Record<
    "hasTokens" | "hasIcons" | "hasPackageJson" | "hasFixtures" | "hasSource" | "hasHandoffManifest",
    boolean
  >;

  return {
    ...flags,
    readyToRun: missingItems.length === 0,
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
