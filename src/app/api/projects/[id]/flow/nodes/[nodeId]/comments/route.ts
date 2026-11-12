// @story 04.2
// @implements AC-01, AC-02, AC-03
import fs from "node:fs";
import path from "node:path";
import { guardedRoute } from "../../../../../../../../lib/api-guard";
import {
  CreateElementCommentSchema,
  ElementCommentResponse,
} from "../../../../../../../../lib/validations/element-comment";

const RADIX_BASE = 36;
const SLICE_START = 2;
const SLICE_END = 7;

// In-memory development store with scratch persistence for element-anchored comments
export const commentsStore: Map<string, ElementCommentResponse> = new Map();

const SCRATCH_DIR = path.resolve(
  process.env.IWISH_PROJECT_ROOT || process.cwd(),
  "_iwish-output/adhoc-workspace/scratch"
);
const PERSIST_FILE_PATH = path.join(SCRATCH_DIR, "element-comments.json");

function loadCommentsStore(): void {
  try {
    if (fs.existsSync(PERSIST_FILE_PATH)) {
      const raw = fs.readFileSync(PERSIST_FILE_PATH, "utf-8");
      const list: ElementCommentResponse[] = JSON.parse(raw);
      for (const item of list) {
        commentsStore.set(item.id, item);
      }
    }
  } catch {
    // Ignore read errors
  }
}

loadCommentsStore();

function persistCommentsStore(): void {
  try {
    if (!fs.existsSync(SCRATCH_DIR)) {
      fs.mkdirSync(SCRATCH_DIR, { recursive: true });
    }
    const list = Array.from(commentsStore.values());
    fs.writeFileSync(PERSIST_FILE_PATH, JSON.stringify(list, null, 2), "utf-8");
  } catch {
    // Ignore write errors
  }
}

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "";
  const nodeId = ctx.params?.nodeId || "";

  const nodeComments = Array.from(commentsStore.values()).filter(
    (c) => c.projectId === projectId && c.nodeId === nodeId
  );

  return new Response(JSON.stringify(nodeComments), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export const POST = guardedRoute(
  { body: CreateElementCommentSchema },
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
    const id = `cmt-${Date.now()}-${Math.random().toString(RADIX_BASE).slice(SLICE_START, SLICE_END)}`;

    const newComment: ElementCommentResponse = {
      id,
      tenantId,
      projectId,
      nodeId,
      elementFfId: body.elementFfId,
      author: body.author,
      content: body.content,
      domHash: body.domHash,
      anchorStatus: "active",
      createdAt: nowIso,
      updatedAt: nowIso,
    };

    commentsStore.set(id, newComment);
    persistCommentsStore();

    return new Response(JSON.stringify(newComment), {
      status: 201,
      headers: { "Content-Type": "application/json" },
    });
  }
);
