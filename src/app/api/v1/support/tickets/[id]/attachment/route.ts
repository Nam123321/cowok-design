// @story 14.1
// @implements AC-01, AC-02 (Authenticated BFF Proxy for GET /api/v1/support/tickets/:id/attachment)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";

const HTTP_BAD_REQUEST = 400;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };
const SAFE_TICKET_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;

export const GET = guardedRoute<{ id: string }>(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const ticketId = ctx.params?.id?.trim() ?? "";
    if (!SAFE_TICKET_ID_RE.test(ticketId)) {
      return new Response(
        JSON.stringify({ error: { code: "INVALID_TICKET_ID", message: "Invalid ticket ID", requestId } }),
        { status: HTTP_BAD_REQUEST, headers: JSON_HEADERS }
      );
    }
    try {
      return await forwardToDaemonFlow({
        method: "GET",
        daemonPath: `/api/v1/support/tickets/${encodeURIComponent(ticketId)}/attachment`,
        inboundHeaders: req.headers,
        principal: {
          tenantId: ctx.tenantId || "local-default",
          userId: ctx.userId || "local-operator",
          role: ctx.role || "viewer",
        },
        correlationId: requestId,
        bffSessionToken: ctx.bffSessionToken,
        isHumanWorkbench: ctx.isHumanWorkbench,
        humanSessionToken: ctx.humanSessionToken,
        signal: req.signal,
      });
    } catch {
      return new Response(
        JSON.stringify({ error: { code: "DAEMON_UNAVAILABLE", message: "Daemon unavailable", requestId } }),
        { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
      );
    }
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
