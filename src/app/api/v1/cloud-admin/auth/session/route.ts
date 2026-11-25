// @story 14.2
// @implements AC-02 (Guarded BFF Proxy for POST /api/v1/cloud-admin/auth/session — OIDC+MFA Token Exchange)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  AdminAuthSessionRequestSchema,
  AdminAuthSessionResponseSchema,
  CloudAdminErrorEnvelopeSchema,
} from "@/lib/validations/cloud-admin";

const MAX_BODY_BYTES = 16_384;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function extractCloudHeaders(headers: Headers): Record<string, string> {
  const extra: Record<string, string> = {};
  const auth = headers.get("authorization");
  if (auth) extra.Authorization = auth;
  const adminSess = headers.get("x-admin-session");
  if (adminSess) extra["X-Admin-Session"] = adminSess;
  const oidcSecret = headers.get("x-cloud-admin-oidc-secret");
  if (oidcSecret) extra["X-Cloud-Admin-Oidc-Secret"] = oidcSecret;
  return extra;
}

async function relayAuthSessionResponse(daemonResp: Response, requestId: string): Promise<Response> {
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
    const okParsed = AdminAuthSessionResponseSchema.safeParse(body);
    if (okParsed.success) {
      return new Response(JSON.stringify(okParsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
    }
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Admin auth response mismatch", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "ADMIN_AUTH_REQUIRED", message: "Admin session exchange failed", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, AdminAuthSessionRequestSchema, requestId, MAX_BODY_BYTES);
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/v1/admin/auth/session",
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
    return relayAuthSessionResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    rawBinary: true,
    body: AdminAuthSessionRequestSchema,
  }
);
