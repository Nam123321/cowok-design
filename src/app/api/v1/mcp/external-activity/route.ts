// @story 11.2
// @implements AC-04 (Guarded BFF Proxy for GET /api/v1/mcp/external-activity)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  McpExternalActivityQuerySchema,
  McpExternalActivityResponseSchema,
  McpGatewayErrorResponseSchema,
} from "@/lib/validations/mcp-gateway";

const HTTP_BAD_REQUEST = 400;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildValidationError(message: string, requestId: string, field?: string): Response {
  return new Response(
    JSON.stringify({
      error: {
        code: "VALIDATION_ERROR",
        message,
        ...(field ? { field } : {}),
        requestId,
      },
    }),
    { status: HTTP_BAD_REQUEST, headers: JSON_HEADERS }
  );
}

function parseValidatedActivityQuery(
  url: URL,
  requestId: string
): { rawQuery: string } | { errorResponse: Response } {
  const rawParams: Record<string, string> = {};
  for (const [key, value] of url.searchParams.entries()) {
    if (Object.prototype.hasOwnProperty.call(rawParams, key)) {
      return {
        errorResponse: buildValidationError(`Duplicate query parameter '${key}'`, requestId, key),
      };
    }
    rawParams[key] = value;
  }
  const parsed = McpExternalActivityQuerySchema.safeParse(rawParams);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    const field = issue?.path?.length ? issue.path.join(".") : undefined;
    return {
      errorResponse: buildValidationError(
        issue?.message ?? "Invalid MCP external activity query parameters",
        requestId,
        field
      ),
    };
  }
  const clean = new URLSearchParams();
  clean.set("limit", String(parsed.data.limit));
  if (parsed.data.hostClient !== undefined) clean.set("hostClient", parsed.data.hostClient);
  if (parsed.data.status !== undefined) clean.set("status", parsed.data.status);
  return { rawQuery: clean.toString() };
}

async function validateActivityResponse(daemonResp: Response, requestId: string): Promise<Response> {
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
    const parsed = McpExternalActivityResponseSchema.safeParse(body);
    if (!parsed.success) {
      return new Response(
        JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "MCP external activity contract mismatch", requestId } }),
        { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
      );
    }
    return new Response(JSON.stringify(parsed.data), { status: daemonResp.status, headers: JSON_HEADERS });
  }
  const errParsed = McpGatewayErrorResponseSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : { error: { code: "INTERNAL_ERROR", message: "MCP external activity upstream error", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const validated = parseValidatedActivityQuery(url, requestId);
    if ("errorResponse" in validated) {
      return validated.errorResponse;
    }
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/mcp/external-activity",
      rawQuery: validated.rawQuery,
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
    return validateActivityResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
