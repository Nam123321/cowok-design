// @story 06.1
// @implements AC-01, AC-02, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { DuplicateSlideRequestSchema } from "@/lib/validations/presentation";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const deckId = ctx.params?.deckId || "";
    const slideUid = ctx.params?.slideUid || "";
    const idemKey = req.headers.get("idempotency-key")?.trim();
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/${encodeURIComponent(slideUid)}/duplicate`,
      body,
      inboundHeaders: req.headers,
      extraHeaders: idemKey ? { "Idempotency-Key": idemKey } : undefined,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", roles: ["editor", "admin"], body: DuplicateSlideRequestSchema }
);
