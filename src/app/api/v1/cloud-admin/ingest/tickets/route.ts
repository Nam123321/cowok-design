// @story 14.2
// @implements AC-01, AC-05 (Guarded BFF Proxy for POST /api/v1/cloud-admin/ingest/tickets)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CloudAdminErrorEnvelopeSchema,
  IngestTicketRequestSchema,
  IngestTicketResponseSchema,
} from "@/lib/validations/cloud-admin";

const MAX_BODY_BYTES = 8_388_608;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function extractCloudHeaders(headers: Headers): Record<string, string> {
  const extra: Record<string, string> = {};
  const auth = headers.get("authorization");
  if (auth) extra.Authorization = auth;
  const adminSess = headers.get("x-admin-session");
  if (adminSess) extra["X-Admin-Session"] = adminSess;
  const clientIp = headers.get("x-client-ip") || headers.get("x-real-ip");
  if (clientIp) extra["X-Client-Ip"] = clientIp;
  return extra;
}

async function relayIngestTicketResponse(daemonResp: Response, requestId: string): Promise<Response> {
  const headers: Record<string, string> = { ...JSON_HEADERS };
  const retryAfter = daemonResp.headers.get("retry-after");
  if (retryAfter) headers["Retry-After"] = retryAfter;
  let body: unknown;
  try {
    body = JSON.parse(await daemonResp.text());
  } catch {
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Non-JSON daemon response", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers }
    );
  }
  if (daemonResp.ok) {
    const okParsed = IngestTicketResponseSchema.safeParse(body);
    if (okParsed.success) {
      return new Response(JSON.stringify(okParsed.data), { status: daemonResp.status, headers });
    }
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Ingest ticket response contract mismatch", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers }
    );
  }
  const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "UPSTREAM_ERROR", message: "Cloud ticket ingest failed", requestId } }
    ),
    { status: daemonResp.status, headers }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, IngestTicketRequestSchema, requestId, MAX_BODY_BYTES);
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/v1/ingest/tickets",
      body: parsed.data,
      inboundHeaders: req.headers,
      extraHeaders: extractCloudHeaders(req.headers),
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
    return relayIngestTicketResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    rawBinary: true,
    body: IngestTicketRequestSchema,
  }
);
