// @story 04.2
// @implements AC-01, AC-02, AC-03
import fs from "node:fs";
import path from "node:path";
import { guardedRoute } from "../../../../../../../../lib/api-guard";
import {
  CreateScreenFixtureSchema,
  ScreenFixtureResponse,
} from "../../../../../../../../lib/validations/screen-fixture";

const RADIX_BASE = 36;
const SLICE_START = 2;
const SLICE_END = 7;

// In-memory development store with scratch persistence for screen state fixtures
export const fixturesStore: Map<string, ScreenFixtureResponse> = new Map();

const SCRATCH_DIR = path.resolve(
  process.env.IWISH_PROJECT_ROOT || process.cwd(),
  "_iwish-output/adhoc-workspace/scratch"
);
const PERSIST_FILE_PATH = path.join(SCRATCH_DIR, "screen-fixtures.json");

function loadFixturesStore(): void {
  try {
    if (fs.existsSync(PERSIST_FILE_PATH)) {
      const raw = fs.readFileSync(PERSIST_FILE_PATH, "utf-8");
      const list: ScreenFixtureResponse[] = JSON.parse(raw);
      for (const item of list) {
        fixturesStore.set(item.id, item);
      }
    }
  } catch {
    // Ignore read errors
  }
}

loadFixturesStore();

function persistFixturesStore(): void {
  try {
    if (!fs.existsSync(SCRATCH_DIR)) {
      fs.mkdirSync(SCRATCH_DIR, { recursive: true });
    }
    const list = Array.from(fixturesStore.values());
    fs.writeFileSync(PERSIST_FILE_PATH, JSON.stringify(list, null, 2), "utf-8");
  } catch {
    // Ignore write errors
  }
}

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "";
  const nodeId = ctx.params?.nodeId || "";

  const nodeFixtures = Array.from(fixturesStore.values()).filter(
    (f) => f.projectId === projectId && f.nodeId === nodeId
  );

  return new Response(JSON.stringify(nodeFixtures), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export const POST = guardedRoute(
  { body: CreateScreenFixtureSchema },
  async (_req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "default-project";
    const tenantId = ctx.tenantId || "default-tenant";
    const nodeId = ctx.params?.nodeId || "default-node";

    if (!body) {
      return new Response(JSON.stringify({ error: "MISSING_BODY" }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      });
    }

    const nowIso = new Date().toISOString();
    const id = `fix-${Date.now()}-${Math.random().toString(RADIX_BASE).slice(SLICE_START, SLICE_END)}`;

    const newFixture: ScreenFixtureResponse = {
      id,
      tenantId,
      projectId,
      nodeId,
      variant: body.variant,
      payloadJson: body.payloadJson ?? "{}",
      isDefault: body.isDefault ?? false,
      createdAt: nowIso,
      updatedAt: nowIso,
    };

    fixturesStore.set(id, newFixture);
    persistFixturesStore();

    return new Response(JSON.stringify(newFixture), {
      status: 201,
      headers: { "Content-Type": "application/json" },
    });
  }
);
