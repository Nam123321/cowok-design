// @story 14.2
// @implements AC-02, AC-03 (Guarded BFF Proxy for GET /api/v1/cloud-admin/tickets)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CloudAdminErrorEnvelopeSchema,
  ListAdminTicketsResponseSchema,
} from "@/lib/validations/cloud-admin";

const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function extractAdminAuthHeaders(headers: Headers): Record<string, string> {
  const extra: Record<string, string> = {};
  const auth = headers.get("authorization");
  if (auth) extra.Authorization = auth;
  const adminSess = headers.get("x-admin-session");
  if (adminSess) extra["X-Admin-Session"] = adminSess;
  return extra;
}

async function relayListTicketsResponse(daemonResp: Response, requestId: string): Promise<Response> {
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
    const okParsed = ListAdminTicketsResponseSchema.safeParse(body);
    if (okParsed.success) {
      return new Response(JSON.stringify(okParsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
    }
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Admin tickets response mismatch", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "UPSTREAM_ERROR", message: "Failed to load admin tickets", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/v1/admin/tickets",
      rawQuery: url.search.replace(/^\?/, ""),
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
    return relayListTicketsResponse(daemonResp, requestId);
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
