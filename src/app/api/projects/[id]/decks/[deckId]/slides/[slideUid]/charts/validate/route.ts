// @story 06.3
// @implements AC-01, AC-03
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { ValidateChartRequestSchema } from "@/lib/validations/presentation-chart-specs";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const deckId = ctx.params?.deckId || "";
    const slideUid = ctx.params?.slideUid || "";
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/${encodeURIComponent(slideUid)}/charts/validate`,
      body,
      inboundHeaders: req.headers,
      extraHeaders: { "X-Cowok-Project-Id": projectId },
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"], readOnlyPost: true, body: ValidateChartRequestSchema }
);
