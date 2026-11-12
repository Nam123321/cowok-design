// @stateless-cache-bounded: ephemeral in-memory development store
import fs from "node:fs";
import path from "node:path";
import { guardedRoute } from "@/lib/api-guard";
import { collectProjectFiles } from "../../route";
import { createZipArchive } from "../../../../../../../features/snapshots/utils/export-bundle";

const SCRATCH_SUBDIR = "_iwish-output/adhoc-workspace/scratch";

function findExistingZip(scratchDir: string, projectId: string, tenantId: string): string | null {
  if (!fs.existsSync(scratchDir)) return null;
  const files = fs.readdirSync(scratchDir);
  const prefix = `${projectId}-${tenantId}-bundle-`;
  const match = files.find((f) => f.startsWith(prefix) && f.endsWith(".zip"));
  return match ? path.join(scratchDir, match) : null;
}

function generateZipFile(
  scratchDir: string,
  projectId: string,
  tenantId: string
): { path: string; fileName: string } {
  if (!fs.existsSync(scratchDir)) {
    fs.mkdirSync(scratchDir, { recursive: true });
  }
  const dateStr = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  const fileName = `${projectId}-${tenantId}-bundle-${dateStr}.zip`;
  const files = collectProjectFiles(projectId, tenantId);
  const zipBytes = createZipArchive(files);
  const artifactPath = path.join(scratchDir, fileName);
  fs.writeFileSync(artifactPath, zipBytes);
  return { path: artifactPath, fileName };
}

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "default-project";
  const tenantId = ctx.tenantId || "default-tenant";
  const scratchDir = path.resolve(process.cwd(), SCRATCH_SUBDIR);

  let zipPath = findExistingZip(scratchDir, projectId, tenantId);
  let fileName = zipPath ? path.basename(zipPath) : "";

  if (!zipPath || !fs.existsSync(zipPath)) {
    const generated = generateZipFile(scratchDir, projectId, tenantId);
    zipPath = generated.path;
    fileName = generated.fileName;
  }

  const bytes = fs.readFileSync(zipPath);

  return new Response(bytes, {
    status: 200,
    headers: {
      "Content-Type": "application/zip",
      "Content-Disposition": `attachment; filename="${fileName}"`,
      "Content-Length": String(bytes.byteLength),
    },
  });
});
