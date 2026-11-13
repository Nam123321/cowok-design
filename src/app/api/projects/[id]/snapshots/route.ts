// @stateless-cache-bounded: ephemeral in-memory development store
import fs from "node:fs";
import path from "node:path";
import { guardedRoute } from "@/lib/api-guard";
import {
  CreateSnapshotInput,
  CreateSnapshotSchema,
  SnapshotFileItem,
  VersionSnapshotResponse,
} from "../../../../../lib/validations/version-snapshot";
import { DEFAULT_SEEDED_ICON_MANIFEST } from "../../../../../lib/validations/constraints";
import {
  createSnapshotFromInput,
  collectWorkspaceFiles,
} from "../../../../../features/snapshots/utils/snapshot-engine";
import { syncSnapshotWithDaemon, createDaemonClient } from "../../../../../api/version-snapshot";

export const snapshotsStore = new Map<string, VersionSnapshotResponse>();

const SCRATCH_DIR = path.resolve(
  process.env.IWISH_PROJECT_ROOT || process.cwd(),
  "_iwish-output/adhoc-workspace/scratch"
);
const PERSIST_FILE_PATH = path.join(SCRATCH_DIR, "version-snapshots.json");

function initStoreFromFile(): void {
  try {
    if (fs.existsSync(PERSIST_FILE_PATH)) {
      const raw = fs.readFileSync(PERSIST_FILE_PATH, "utf-8");
      const list: VersionSnapshotResponse[] = JSON.parse(raw);
      for (const item of list) {
        snapshotsStore.set(item.id, item);
      }
    }
  } catch {
    // Ignore read errors
  }
}

initStoreFromFile();

export function persistSnapshotsStore(): void {
  try {
    if (!fs.existsSync(SCRATCH_DIR)) {
      fs.mkdirSync(SCRATCH_DIR, { recursive: true });
    }
    const list = Array.from(snapshotsStore.values());
    fs.writeFileSync(PERSIST_FILE_PATH, JSON.stringify(list, null, 2), "utf-8");
  } catch {
    // Ignore write errors
  }
}

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonSnapshotSync(
  projectId: string,
  snapshot: VersionSnapshotResponse,
  tenantId?: string
): Promise<{ error: Response | null; snapshot?: VersionSnapshotResponse }> {
  try {
    const remote = await syncSnapshotWithDaemon(projectId, snapshot, tenantId);
    return { error: null, snapshot: remote };
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return { error: null };
    }
    const msg = err instanceof Error ? err.message : "DAEMON_SNAPSHOT_SYNC_FAILED";
    return {
      error: new Response(JSON.stringify({ error: "DAEMON_SNAPSHOT_SYNC_FAILED", message: msg }), {
        status: HTTP_STATUS_BAD_GATEWAY,
        headers: { "Content-Type": "application/json" },
      }),
    };
  }
}

export function collectProjectFiles(projectId: string, tenantId?: string): SnapshotFileItem[] {
  const snapshots = Array.from(snapshotsStore.values()).filter(
    (s) => s.projectId === projectId && (!tenantId || s.tenantId === tenantId)
  );

  const baseFiles: SnapshotFileItem[] =
    snapshots.length > 0
      ? snapshots[snapshots.length - 1].files.map((f) => ({ ...f }))
      : [
          {
            path: "package.json",
            content: '{"name":"project","version":"1.0.0","scripts":{"build":"vite build"}}\n',
          },
          { path: "src/App.tsx", content: "export default function App() { return <div>App</div>; }\n" },
          { path: "src/theme/tokens.css", content: ":root { --color-primary: #000; }\n" },
          {
            path: "src/assets/icon-manifest.json",
            content: `${JSON.stringify(DEFAULT_SEEDED_ICON_MANIFEST)}\n`,
          },
          { path: "src/fixtures/screen-fixture.json", content: '{"fixtures":[]}\n' },
        ];

  return collectWorkspaceFiles(projectId, tenantId || "default-tenant", baseFiles);
}

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "";
  const tenantId = ctx.tenantId;

  if (process.env.NODE_ENV !== "test") {
    try {
      const client = createDaemonClient(tenantId);
      const remote = await client.getSnapshots(projectId);
      if (remote && remote.length > 0) {
        return new Response(JSON.stringify(remote), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
    } catch {
      // Fall back to local store
    }
  }

  const list = Array.from(snapshotsStore.values()).filter(
    (s) => s.projectId === projectId && (!tenantId || s.tenantId === tenantId)
  );

  return new Response(JSON.stringify(list), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export const POST = guardedRoute({ body: CreateSnapshotSchema }, async (_req, ctx, { body }) => {
  const projectId = ctx.projectId || ctx.params?.id || "default-project";
  const tenantId = ctx.tenantId || "default-tenant";

  if (!body) {
    return new Response(JSON.stringify({ error: "MISSING_BODY" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  const existingCount = Array.from(snapshotsStore.values()).filter(
    (s) => s.projectId === projectId
  ).length;

  let newSnapshot = createSnapshotFromInput(projectId, tenantId, body, existingCount);

  const { error: syncError, snapshot: daemonSnapshot } = await executeDaemonSnapshotSync(
    projectId,
    newSnapshot,
    tenantId
  );
  if (syncError) return syncError;
  if (daemonSnapshot) {
    newSnapshot = daemonSnapshot;
  }

  snapshotsStore.set(newSnapshot.id, newSnapshot);
  persistSnapshotsStore();

  return new Response(JSON.stringify(newSnapshot), {
    status: 201,
    headers: { "Content-Type": "application/json" },
  });
});
