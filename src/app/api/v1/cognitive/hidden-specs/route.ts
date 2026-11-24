// @story 05.6
// @implements AC-04, AC-06 (Guarded BFF Proxy for GET/PUT /api/v1/cognitive/hidden-specs)
import type { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  firstSsotZodField,
  parseJsonBodyStrict,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  CognitiveStudioErrorResponseSchema,
  HiddenSpecsListResponseSchema,
  HiddenSpecsQuerySchema,
  PutHiddenSpecRequestSchema,
  PutHiddenSpecResponseSchema,
} from "@/lib/validations/cognitive-studio";

const MAX_BODY_BYTES = 262_144;
const HTTP_UNPROCESSABLE = 422;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

async function validateHiddenSpecResp<T>(
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
        JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Hidden spec contract mismatch", requestId } }),
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
        : { error: { code: "INTERNAL_ERROR", message: "Hidden spec upstream error", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsedQuery = HiddenSpecsQuerySchema.safeParse(
      Object.fromEntries(url.searchParams.entries())
    );
    if (!parsedQuery.success) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INVALID_REQUEST",
            message: parsedQuery.error.issues[0]?.message ?? "Invalid projectId",
            field: firstSsotZodField(parsedQuery.error) ?? "projectId",
            requestId,
          },
        }),
        { status: HTTP_UNPROCESSABLE, headers: JSON_HEADERS }
      );
    }
    const qs = new URLSearchParams({ projectId: parsedQuery.data.projectId }).toString();
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/cognitive/hidden-specs",
      rawQuery: qs,
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
    return validateHiddenSpecResp(daemonResp, requestId, HiddenSpecsListResponseSchema);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);

export const PUT = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      PutHiddenSpecRequestSchema,
      requestId,
      MAX_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "PUT",
      daemonPath: "/api/v1/cognitive/hidden-specs",
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
    return validateHiddenSpecResp(daemonResp, requestId, PutHiddenSpecResponseSchema);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: PutHiddenSpecRequestSchema,
  }
);
