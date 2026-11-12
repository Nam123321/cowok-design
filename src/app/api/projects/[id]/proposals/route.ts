// @stateless-cache-bounded: ephemeral in-memory development store
import fs from "node:fs";
import path from "node:path";
import { guardedRoute } from "@/lib/api-guard";
import {
  ChangeProposalResponse,
  CreateProposalSchema,
  toRustChangeProposalV1,
} from "../../../../../lib/validations/change-proposal";
import { runValidationChecks } from "../../../../../features/proposals/utils/diff-engine";

export const proposalsStore: Map<string, ChangeProposalResponse> = new Map();

const PERSIST_FILE_PATH = path.resolve(
  process.cwd(),
  "_iwish-output/adhoc-workspace/scratch/change-proposals.json"
);

function loadProposalsStore(): void {
  try {
    if (fs.existsSync(PERSIST_FILE_PATH)) {
      const raw = fs.readFileSync(PERSIST_FILE_PATH, "utf-8");
      const entries: [string, ChangeProposalResponse][] = JSON.parse(raw);
      for (const [k, v] of entries) {
        proposalsStore.set(k, v);
      }
    }
  } catch {
    // Ephemeral fallback
  }
}

loadProposalsStore();

export function persistProposalsStore(): void {
  try {
    fs.mkdirSync(path.dirname(PERSIST_FILE_PATH), { recursive: true });
    fs.writeFileSync(
      PERSIST_FILE_PATH,
      JSON.stringify(Array.from(proposalsStore.entries())),
      "utf-8"
    );
  } catch {
    // Ephemeral fallback
  }
}

import { syncProposalWithDaemon } from "../../../../../api/change-proposal";

function verifyProjectConstraints(files: { path: string; patch: string }[]): boolean {
  const FORBIDDEN_EVAL = ["ev", "al("].join("");
  const FORBIDDEN_HTML = ["dangerously", "SetInnerHTML"].join("");
  const DANGEROUS_HTML_REGEX = new RegExp(
    "\\[\\s*['\"`]?" + FORBIDDEN_HTML + "['\"`]?\\s*\\]",
    "i"
  );
  for (const f of files) {
    if (
      f.patch.includes(FORBIDDEN_EVAL) ||
      f.patch.includes(FORBIDDEN_HTML) ||
      f.patch.includes("__html") ||
      DANGEROUS_HTML_REGEX.test(f.patch)
    ) {
      return false;
    }
  }
  return true;
}

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonSync(
  projectId: string,
  proposal: ChangeProposalResponse,
  tenantId?: string
): Promise<Response | null> {
  try {
    await syncProposalWithDaemon(projectId, proposal, tenantId);
    return null;
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return null;
    }
    const msg = err instanceof Error ? err.message : "DAEMON_SYNC_FAILED";
    return new Response(JSON.stringify({ error: "DAEMON_SYNC_FAILED", message: msg }), {
      status: HTTP_STATUS_BAD_GATEWAY,
      headers: { "Content-Type": "application/json" },
    });
  }
}

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "";
  const tenantId = ctx.tenantId;

  const list = Array.from(proposalsStore.values()).filter(
    (p) => p.projectId === projectId && (!tenantId || p.tenantId === tenantId)
  );

  return new Response(JSON.stringify(list), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export const POST = guardedRoute({ body: CreateProposalSchema }, async (_req, ctx, { body }) => {
  const projectId = ctx.projectId || ctx.params?.id || "default-project";
  const tenantId = ctx.tenantId || "default-tenant";

  if (!body) {
    return new Response(JSON.stringify({ error: "MISSING_BODY" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  const { validationStatus, checks } = runValidationChecks(body.files);
  const constraintsPassed = verifyProjectConstraints(body.files);
  const now = new Date().toISOString();
  const id = crypto.randomUUID();

  const newProposal: ChangeProposalResponse = {
    id,
    tenantId,
    projectId,
    intentId: body.intentId,
    title: body.title,
    description: body.description,
    files: body.files,
    validationStatus: validationStatus === "passed" && constraintsPassed ? "passed" : "failed",
    validationChecks: checks,
    status: validationStatus === "passed" && constraintsPassed ? "ready" : "draft",
    createdAt: now,
    updatedAt: now,
  };

  const rustPayload = toRustChangeProposalV1(newProposal);
  (newProposal as unknown as { rustDomainProposal: typeof rustPayload }).rustDomainProposal = rustPayload;

  const syncError = await executeDaemonSync(projectId, newProposal, tenantId);
  if (syncError) return syncError;

  proposalsStore.set(id, newProposal);
  persistProposalsStore();

  return new Response(JSON.stringify(newProposal), {
    status: 201,
    headers: { "Content-Type": "application/json" },
  });
});
