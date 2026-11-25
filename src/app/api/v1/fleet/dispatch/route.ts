// @story 10.2
// @implements AC-01, AC-03 (Guarded BFF Proxy for POST /api/v1/fleet/dispatch)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  DispatchFleetSwarmRequestSchema,
  DispatchFleetSwarmResponseSchema,
  FleetSwarmErrorResponseSchema,
} from "@/lib/validations/fleet-swarm";

const MAX_BODY_BYTES = 262_144;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

async function validateDispatchResponse(daemonResp: Response, requestId: string): Promise<Response> {
  let body: unknown;
  try {
    body = JSON.parse(await daemonResp.text());
  } catch {
    return new Response(
      JSON.stringify({
        error: { code: "INTERNAL_ERROR", message: "Non-JSON daemon response", requestId },
      }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  if (daemonResp.ok) {
    const parsed = DispatchFleetSwarmResponseSchema.safeParse(body);
    if (!parsed.success) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INTERNAL_ERROR",
            message: "Fleet dispatch response mismatch",
            requestId,
          },
        }),
        { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
      );
    }
    return new Response(JSON.stringify(parsed.data), {
      status: daemonResp.status,
      headers: JSON_HEADERS,
    });
  }
  const errParsed = FleetSwarmErrorResponseSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : {
            error: {
              code: "INTERNAL_ERROR",
              message: "Fleet dispatch upstream error",
              requestId,
            },
          }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      DispatchFleetSwarmRequestSchema,
      requestId,
      MAX_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/fleet/dispatch",
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
    return validateDispatchResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: DispatchFleetSwarmRequestSchema,
  }
);
