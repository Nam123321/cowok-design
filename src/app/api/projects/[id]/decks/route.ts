// @story 06.1
// @implements AC-01, AC-02, AC-04, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext, forwardToDaemonFlow } from "@/lib/daemon-client";
import { CreateDeckRequestSchema } from "@/lib/validations/presentation";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/decks`
    );
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const idemKey = req.headers.get("idempotency-key")?.trim();
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/decks`,
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
  { authMode: "strict-loopback", roles: ["editor", "admin"], body: CreateDeckRequestSchema }
);
