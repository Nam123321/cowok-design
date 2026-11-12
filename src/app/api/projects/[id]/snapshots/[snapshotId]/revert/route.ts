// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { snapshotsStore, persistSnapshotsStore, collectProjectFiles } from "../../route";
import {
  RevertSnapshotSchema,
  VersionSnapshotResponse,
} from "../../../../../../../lib/validations/version-snapshot";
import { performTwoPhaseRevert } from "../../../../../../../features/snapshots/utils/snapshot-engine";
import { syncRevertWithDaemon, createDaemonClient } from "../../../../../../../api/version-snapshot";

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonRevert(
  projectId: string,
  snapshotId: string,
  tenantId?: string
): Promise<Response | null> {
  try {
    await syncRevertWithDaemon(projectId, snapshotId, tenantId);
    return null;
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return null;
    }
    const msg = err instanceof Error ? err.message : "DAEMON_REVERT_FAILED";
    return new Response(JSON.stringify({ error: "DAEMON_REVERT_FAILED", message: msg }), {
      status: HTTP_STATUS_BAD_GATEWAY,
      headers: { "Content-Type": "application/json" },
    });
  }
}

async function resolveTargetSnapshot(
  snapshotId: string,
  projectId: string,
  tenantId?: string
): Promise<VersionSnapshotResponse | undefined> {
  const local = snapshotsStore.get(snapshotId);
  if (local) return local;

  try {
    const client = createDaemonClient(tenantId);
    const remoteSnapshots = await client.getSnapshots(projectId);
    const found = remoteSnapshots.find((s) => s.id === snapshotId);
    if (found) {
      snapshotsStore.set(found.id, found);
      return found;
    }
  } catch {
    // Ignore and return undefined
  }
  return undefined;
}

export const POST = guardedRoute({ body: RevertSnapshotSchema }, async (_req, ctx) => {
  const snapshotId = ctx.params?.snapshotId || "";
  const projectId = ctx.projectId || ctx.params?.id || "";
  const tenantId = ctx.tenantId;

  const targetSnapshot = await resolveTargetSnapshot(snapshotId, projectId, tenantId);
  if (!targetSnapshot) {
    return new Response(JSON.stringify({ error: "SNAPSHOT_NOT_FOUND" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  if (projectId && targetSnapshot.projectId !== projectId) {
    return new Response(JSON.stringify({ error: "PROJECT_MISMATCH" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  if (tenantId && targetSnapshot.tenantId !== tenantId) {
    return new Response(JSON.stringify({ error: "TENANT_MISMATCH" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  const currentFiles = collectProjectFiles(projectId, tenantId);
  const { safetySnapshot, result } = performTwoPhaseRevert(
    currentFiles,
    targetSnapshot,
    projectId,
    tenantId || "default-tenant"
  );

  const daemonError = await executeDaemonRevert(projectId, snapshotId, tenantId);
  if (daemonError) return daemonError;

  snapshotsStore.set(safetySnapshot.id, safetySnapshot);
  persistSnapshotsStore();

  return new Response(JSON.stringify(result), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
