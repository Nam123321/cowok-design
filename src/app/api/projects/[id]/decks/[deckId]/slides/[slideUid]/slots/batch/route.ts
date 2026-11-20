// @story 06.2
// @implements AC-02, AC-04, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { MAX_SLIDE_HTML_BYTES, PatchSlotsBatchRequestSchema } from "@/lib/validations/presentation";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const deckId = ctx.params?.deckId || "";
    const slideUid = ctx.params?.slideUid || "";
    const idemKey = req.headers.get("idempotency-key")?.trim();
    const extraHeaders: Record<string, string> = { "X-Cowok-Project-Id": projectId };
    if (idemKey) {
      extraHeaders["Idempotency-Key"] = idemKey;
    }
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/${encodeURIComponent(slideUid)}/slots/batch`,
      body,
      inboundHeaders: req.headers,
      extraHeaders,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", roles: ["editor", "admin"], body: PatchSlotsBatchRequestSchema, maxBodyBytes: MAX_SLIDE_HTML_BYTES }
);
