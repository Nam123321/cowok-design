// @story 14.2
// @implements AC-01, AC-03, AC-05 (Guarded BFF Proxy for GET /api/v1/cloud-admin/tickets/:id/attachment)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import { CloudAdminErrorEnvelopeSchema } from "@/lib/validations/cloud-admin";

const HTTP_BAD_REQUEST = 400;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };
const SAFE_TICKET_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;

function extractAdminAuthHeaders(headers: Headers): Record<string, string> {
  const extra: Record<string, string> = {};
  const auth = headers.get("authorization");
  if (auth) extra.Authorization = auth;
  const adminSess = headers.get("x-admin-session");
  if (adminSess) extra["X-Admin-Session"] = adminSess;
  return extra;
}

async function relayAttachmentResponse(daemonResp: Response, requestId: string): Promise<Response> {
  const contentType = daemonResp.headers.get("content-type") || "";
  if (daemonResp.ok && contentType.startsWith("image/")) {
    return new Response(await daemonResp.arrayBuffer(), {
      status: daemonResp.status,
      headers: {
        "Content-Type": contentType,
        "X-Content-Type-Options": "nosniff",
        "Cache-Control": "no-store",
        "X-Correlation-Id": requestId,
      },
    });
  }
  if (daemonResp.ok) {
    return new Response(
      JSON.stringify({
        error: { code: "INTERNAL_ERROR", message: "Unexpected non-image attachment response", requestId },
      }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  try {
    const body: unknown = JSON.parse(await daemonResp.text());
    const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
    return new Response(
      JSON.stringify(
        errParsed.success
          ? errParsed.data
          : { error: { code: "UPSTREAM_ERROR", message: "Failed to fetch ticket attachment", requestId } }
      ),
      { status: daemonResp.status, headers: JSON_HEADERS }
    );
  } catch {
    return new Response(
      JSON.stringify({ error: { code: "UPSTREAM_ERROR", message: "Failed to fetch ticket attachment", requestId } }),
      { status: daemonResp.status, headers: JSON_HEADERS }
    );
  }
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const ticketId = ctx.params?.id?.trim() ?? "";
    if (!SAFE_TICKET_ID_RE.test(ticketId)) {
      return new Response(
        JSON.stringify({ error: { code: "INVALID_TICKET_ID", message: "Invalid ticket ID", requestId } }),
        { status: HTTP_BAD_REQUEST, headers: JSON_HEADERS }
      );
    }
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: `/v1/admin/tickets/${encodeURIComponent(ticketId)}/attachment`,
      inboundHeaders: req.headers,
      extraHeaders: extractAdminAuthHeaders(req.headers),
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
    return relayAttachmentResponse(daemonResp, requestId);
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
