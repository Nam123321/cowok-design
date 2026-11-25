// @story 14.2
// @implements AC-02, AC-03 (Guarded BFF Proxy for GET & POST /api/v1/cloud-admin/clusters)
import { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CloudAdminErrorEnvelopeSchema,
  CreateClusterRequestSchema,
  CreateClusterResponseSchema,
  ListClustersResponseSchema,
} from "@/lib/validations/cloud-admin";

const MAX_BODY_BYTES = 65_536;
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

async function relayClusterResponse<T>(
  daemonResp: Response,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string
): Promise<Response> {
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
    const okParsed = schema.safeParse(body);
    if (okParsed.success) {
      return new Response(JSON.stringify(okParsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
    }
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Clusters response mismatch", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  const errParsed = CloudAdminErrorEnvelopeSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "UPSTREAM_ERROR", message: "Cluster request failed", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/v1/admin/clusters",
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
    return relayClusterResponse(daemonResp, ListClustersResponseSchema, requestId);
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, CreateClusterRequestSchema, requestId, MAX_BODY_BYTES);
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/v1/admin/clusters",
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
    return relayClusterResponse(daemonResp, CreateClusterResponseSchema, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    rawBinary: true,
    body: CreateClusterRequestSchema,
  }
);
