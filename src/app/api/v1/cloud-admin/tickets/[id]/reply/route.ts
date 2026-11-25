// @story 14.2
// @implements AC-02, AC-03 (Guarded BFF Proxy for POST /api/v1/cloud-admin/tickets/:id/reply)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CloudAdminErrorEnvelopeSchema,
  ReplyAdminTicketRequestSchema,
  ReplyAdminTicketResponseSchema,
} from "@/lib/validations/cloud-admin";

const MAX_BODY_BYTES = 65_536;
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

async function relayReplyResponse(daemonResp: Response, requestId: string): Promise<Response> {
  let body: unknown;
  try {
    body = JSON.parse(await daemonResp.text());
  } catch {
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Non-JSON daemon response", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  if (daemonResp.ok) {
    const okParsed = ReplyAdminTicketResponseSchema.safeParse(body);
    if (okParsed.success) {
      return new Response(JSON.stringify(okParsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
    }
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Reply response mismatch", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "UPSTREAM_ERROR", message: "Ticket reply failed", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const ticketId = ctx.params?.id?.trim() ?? "";
    if (!SAFE_TICKET_ID_RE.test(ticketId)) {
      return new Response(
        JSON.stringify({ error: { code: "INVALID_TICKET_ID", message: "Invalid ticket ID", requestId } }),
        { status: HTTP_BAD_REQUEST, headers: JSON_HEADERS }
      );
    }
    const parsed = await parseJsonBodyStrict(req, ReplyAdminTicketRequestSchema, requestId, MAX_BODY_BYTES);
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: `/v1/admin/tickets/${encodeURIComponent(ticketId)}/reply`,
      body: parsed.data,
      inboundHeaders: req.headers,
      extraHeaders: extractAdminAuthHeaders(req.headers),
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: requestId,
      bffSessionToken: ctx.bffSessionToken,
      isHumanWorkbench: ctx.isHumanWorkbench,
      humanSessionToken: ctx.humanSessionToken,
      signal: req.signal,
    });
    return relayReplyResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    rawBinary: true,
    body: ReplyAdminTicketRequestSchema,
  }
);
