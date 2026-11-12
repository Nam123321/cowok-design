// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { ClarificationSelectSchema } from "../../../../../../lib/validations/sdi";
import { intentsStore } from "../intent/route";

export const POST = guardedRoute(
  { body: ClarificationSelectSchema },
  async (_req, ctx, { body }) => {
    if (!body) {
      return new Response(JSON.stringify({ error: "MISSING_BODY" }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      });
    }

    const projectId = ctx.projectId || ctx.params?.id;
    const tenantId = ctx.tenantId;
    const intent = intentsStore.get(body.intentId);
    const isMismatch = !intent || (projectId && intent.projectId !== projectId) || (tenantId && intent.tenantId !== tenantId);

    if (isMismatch) {
      return new Response(JSON.stringify({ error: "INTENT_NOT_FOUND" }), {
        status: 404,
        headers: { "Content-Type": "application/json" },
      });
    }

    const validVariants = intent.clarificationVariants || [];
    const hasVariant = validVariants.some((v) => v.id === body.selectedVariantId);
    if (!hasVariant) {
      return new Response(JSON.stringify({ error: "INVALID_VARIANT_ID" }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      });
    }

    const updated = {
      ...intent,
      status: "ready" as const,
      updatedAt: new Date().toISOString(),
    };
    intentsStore.set(body.intentId, updated);

    return new Response(JSON.stringify(updated), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }
);
