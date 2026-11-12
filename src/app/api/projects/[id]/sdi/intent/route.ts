// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import {
  CompileIntentSchema,
  DesignIntentResponse,
  toRustDesignIntentV1,
} from "../../../../../../lib/validations/sdi";
import {
  computeAmbiguityAndRisk,
  generateClarificationVariants,
} from "../../../../../../features/sdi/utils/ambiguity-calculator";

export const intentsStore: Map<string, DesignIntentResponse> = new Map();

export const GET = guardedRoute({}, async (_req, ctx) => {
  const projectId = ctx.projectId || ctx.params?.id || "";
  const tenantId = ctx.tenantId;
  const list = Array.from(intentsStore.values()).filter(
    (i) => i.projectId === projectId && (!tenantId || i.tenantId === tenantId)
  );
  return new Response(JSON.stringify(list), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export const POST = guardedRoute({ body: CompileIntentSchema }, async (_req, ctx, { body }) => {
  const projectId = ctx.projectId || ctx.params?.id || "default-project";
  const tenantId = ctx.tenantId || "default-tenant";

  if (!body) {
    return new Response(JSON.stringify({ error: "MISSING_BODY" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  const { ambiguityScore, riskScore, isAmbiguous } = computeAmbiguityAndRisk(
    body.rawPrompt,
    body.targetNodeId
  );
  const now = new Date().toISOString();
  const id = crypto.randomUUID();

  const newIntent: DesignIntentResponse = {
    id,
    tenantId,
    projectId,
    rawPrompt: body.rawPrompt,
    intentType: "create_screen",
    targetScope: {
      nodeIds: body.targetNodeId ? [body.targetNodeId] : [],
      screenKeys: [],
      components: [],
    },
    ambiguityScore,
    riskScore,
    clarificationVariants: isAmbiguous ? generateClarificationVariants(body.rawPrompt) : undefined,
    status: isAmbiguous ? "clarifying" : "ready",
    createdAt: now,
    updatedAt: now,
  };

  // Convert to Rust Core domain contract (DesignIntentV1)
  const rustPayload = toRustDesignIntentV1(newIntent);
  (newIntent as unknown as { rustDomainIntent: typeof rustPayload }).rustDomainIntent = rustPayload;

  intentsStore.set(id, newIntent);
  return new Response(JSON.stringify(newIntent), {
    status: 201,
    headers: { "Content-Type": "application/json" },
  });
});
