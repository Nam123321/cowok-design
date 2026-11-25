// @story 11.2
// @implements AC-01, AC-02, AC-03 (Guarded BFF Proxy for POST /api/v1/mcp/rpc — Streamable HTTP JSON-RPC 2.0)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  McpGatewayErrorResponseSchema,
  McpJsonRpcRequestSchema,
  McpJsonRpcResponseSchema,
} from "@/lib/validations/mcp-gateway";

const MAX_BODY_BYTES = 262_144;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildMcpExtraHeaders(req: Request): Record<string, string> {
  const extra: Record<string, string> = {};
  const hostClient = req.headers.get("x-mcp-host-client")?.trim();
  if (hostClient) extra["X-Mcp-Host-Client"] = hostClient;
  const clientKind = req.headers.get("x-cowok-client-kind")?.trim();
  if (clientKind) extra["X-Cowok-Client-Kind"] = clientKind;
  return extra;
}

async function validateRpcResponse(daemonResp: Response, requestId: string): Promise<Response> {
  const contentType = daemonResp.headers.get("content-type") || "";
  if (daemonResp.ok && contentType.includes("text/event-stream")) {
    return new Response(await daemonResp.text(), {
      status: daemonResp.status,
      headers: { "Content-Type": "text/event-stream; charset=utf-8" },
    });
  }
  let body: unknown;
  try {
    body = JSON.parse(await daemonResp.text());
  } catch {
    return new Response(
      JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "Non-JSON daemon RPC response", requestId } }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  if (daemonResp.ok) {
    const parsed = McpJsonRpcResponseSchema.safeParse(body);
    if (!parsed.success) {
      return new Response(
        JSON.stringify({ error: { code: "INTERNAL_ERROR", message: "MCP JSON-RPC contract mismatch", requestId } }),
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
        : { error: { code: "INTERNAL_ERROR", message: "MCP RPC upstream error", requestId } }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      McpJsonRpcRequestSchema,
      requestId,
      MAX_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/mcp/rpc",
      body: parsed.data,
      inboundHeaders: req.headers,
      extraHeaders: buildMcpExtraHeaders(req),
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
    return validateRpcResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    rawBinary: true,
    body: McpJsonRpcRequestSchema,
  }
);
