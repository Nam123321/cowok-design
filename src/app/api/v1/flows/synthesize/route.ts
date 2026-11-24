// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (BFF Proxy for POST /api/v1/flows/synthesize & GET /api/v1/flows/substates)
import type { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  firstSsotZodField,
  parseJsonBodyStrict,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  FlowSubstatesListResponseSchema,
  FlowSubstatesQuerySchema,
  FlowSynthesizeErrorCodeSchema,
  FlowSynthesizeErrorResponseSchema,
  FlowSynthesizeRequestSchema,
  SynthesizeFlowResponseSchema,
} from "@/lib/validations/flow-synthesize";

const MAX_SYNTHESIZE_BODY_BYTES = 262_144;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_UNPROCESSABLE = 422;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildBadGatewayError(message: string, requestId: string): Response {
  return new Response(
    JSON.stringify({ error: { code: "INTERNAL_ERROR", message, requestId } }),
    { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
  );
}

function mapFlatDaemonAuthError(
  status: number,
  jsonBody: unknown,
  requestId: string
): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  if (typeof jsonBody !== "object" || jsonBody === null) return null;
  const rec = jsonBody as { code?: unknown; message?: unknown };
  if (typeof rec.code !== "string" || typeof rec.message !== "string") return null;
  const parsedCode = FlowSynthesizeErrorCodeSchema.safeParse(rec.code);
  let fallbackCode: "UNAUTHORIZED" | "FORBIDDEN" | "PAYLOAD_TOO_LARGE" = "PAYLOAD_TOO_LARGE";
  if (status === HTTP_UNAUTHORIZED) {
    fallbackCode = "UNAUTHORIZED";
  } else if (status === HTTP_FORBIDDEN) {
    fallbackCode = "FORBIDDEN";
  }
  const code = parsedCode.success ? parsedCode.data : fallbackCode;
  return new Response(
    JSON.stringify({ error: { code, message: rec.message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

async function mapDaemonFlowResponse<T>(
  daemonResp: Response,
  requestId: string,
  successSchema: z.ZodType<T, z.ZodTypeDef, unknown>,
  contractErrMsg: string
): Promise<Response> {
  let jsonBody: unknown;
  try {
    jsonBody = JSON.parse(await daemonResp.text());
  } catch {
    return buildBadGatewayError("Daemon returned non-JSON flow payload", requestId);
  }
  if (daemonResp.ok) {
    const okCheck = successSchema.safeParse(jsonBody);
    return okCheck.success
      ? new Response(JSON.stringify(okCheck.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        })
      : buildBadGatewayError(contractErrMsg, requestId);
  }
  const errCheck = FlowSynthesizeErrorResponseSchema.safeParse(jsonBody);
  if (errCheck.success) {
    const headers = new Headers(JSON_HEADERS);
    const retryAfter = daemonResp.headers.get("retry-after");
    if (errCheck.data.error.code === "STORAGE_BUSY" && retryAfter) {
      headers.set("Retry-After", retryAfter);
    }
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers,
    });
  }
  const flatAuthErr = mapFlatDaemonAuthError(daemonResp.status, jsonBody, requestId);
  if (flatAuthErr) return flatAuthErr;
  return buildBadGatewayError("Daemon flow error contract violation", requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      FlowSynthesizeRequestSchema,
      requestId,
      MAX_SYNTHESIZE_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;

    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/flows/synthesize",
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
    return mapDaemonFlowResponse(
      daemonResp,
      requestId,
      SynthesizeFlowResponseSchema,
      "VerdictShield flow synthesis response contract violation"
    );
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: FlowSynthesizeRequestSchema,
  }
);

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsedQuery = FlowSubstatesQuerySchema.safeParse(
      Object.fromEntries(url.searchParams.entries())
    );
    if (!parsedQuery.success) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INVALID_REQUEST",
            message: parsedQuery.error.issues[0]?.message ?? "Invalid substates query parameters",
            field: firstSsotZodField(parsedQuery.error) ?? "query",
            requestId,
          },
        }),
        {
          status: HTTP_UNPROCESSABLE,
          headers: { "Content-Type": "application/json", "X-Correlation-Id": requestId },
        }
      );
    }

    const canonical = new URLSearchParams({ projectId: parsedQuery.data.projectId });
    if (parsedQuery.data.flowTier) {
      canonical.set("flowTier", parsedQuery.data.flowTier);
    }
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/flows/substates",
      rawQuery: canonical.toString(),
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
    return mapDaemonFlowResponse(
      daemonResp,
      requestId,
      FlowSubstatesListResponseSchema,
      "Flow substates response contract violation"
    );
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
