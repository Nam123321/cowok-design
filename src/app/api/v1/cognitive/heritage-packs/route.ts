// @story 05.6
// @implements AC-05 (Guarded BFF Proxy for GET/POST /api/v1/cognitive/heritage-packs)
import type { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CognitiveStudioErrorResponseSchema,
  CreateHeritagePackRequestSchema,
  CreateHeritagePackResponseSchema,
  HeritagePacksListResponseSchema,
} from "@/lib/validations/cognitive-studio";

const MAX_BODY_BYTES = 262_144;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

async function validateHeritageResp<T>(
  daemonResp: Response,
  requestId: string,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>
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
    const parsed = schema.safeParse(body);
    if (!parsed.success) {
      return new Response(
        JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Heritage pack contract mismatch", requestId } }),
        { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
      );
    }
    return new Response(JSON.stringify(parsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
  }
  const errParsed = CognitiveStudioErrorResponseSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "INTERNAL_ERROR", message: "Heritage pack upstream error", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/cognitive/heritage-packs",
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
    return validateHeritageResp(daemonResp, requestId, HeritagePacksListResponseSchema);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      CreateHeritagePackRequestSchema,
      requestId,
      MAX_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/cognitive/heritage-packs",
      body: parsed.data,
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
    return validateHeritageResp(daemonResp, requestId, CreateHeritagePackResponseSchema);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: CreateHeritagePackRequestSchema,
  }
);
