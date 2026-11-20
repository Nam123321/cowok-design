// @story 06.4
// @implements AC-01, AC-04
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { PresenterHandshakeRequestSchema } from "@/lib/validations/presentation-presenter";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const deckId = ctx.params?.deckId || "";
    const resolvedProjectId =
      body?.projectId || req.headers.get("x-cowok-project-id")?.trim() || ctx.projectId || "";
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath: `/api/v1/decks/${encodeURIComponent(deckId)}/presenter/handshake`,
      body,
      inboundHeaders: req.headers,
      extraHeaders: resolvedProjectId ? { "X-Cowok-Project-Id": resolvedProjectId } : undefined,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"], readOnlyPost: true, body: PresenterHandshakeRequestSchema }
);
