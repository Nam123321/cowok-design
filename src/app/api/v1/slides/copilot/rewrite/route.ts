// @story 06.5
// @implements AC-01, AC-05 (BFF Proxy for POST /api/v1/slides/copilot/rewrite)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  parseJsonBodyStrict,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  CopilotRewriteSlideRequestSchema,
  CopilotRewriteSlideResponseSchema,
  DeckGenerationErrorResponseSchema,
} from "@/lib/validations/deck-generation";

const MAX_REWRITE_BODY_BYTES = 262_144;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function badGateway(message: string): Response {
  return new Response(
    JSON.stringify({ error: { code: "INTERNAL_ERROR", message, retryable: true } }),
    { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
  );
}

async function mapDaemonResponse(daemonResp: Response): Promise<Response> {
  let jsonBody: unknown;
  try {
    jsonBody = JSON.parse(await daemonResp.text());
  } catch {
    return badGateway("Daemon returned non-JSON slide rewrite payload");
  }
  if (daemonResp.ok) {
    const okCheck = CopilotRewriteSlideResponseSchema.safeParse(jsonBody);
    return okCheck.success
      ? new Response(JSON.stringify(okCheck.data), { status: daemonResp.status, headers: JSON_HEADERS })
      : badGateway("Slide rewrite response contract violation");
  }
  const errCheck = DeckGenerationErrorResponseSchema.safeParse(jsonBody);
  if (errCheck.success) {
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers: JSON_HEADERS,
    });
  }
  return new Response(JSON.stringify(jsonBody), {
    status: daemonResp.status,
    headers: JSON_HEADERS,
  });
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      CopilotRewriteSlideRequestSchema,
      requestId,
      MAX_REWRITE_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;

    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/slides/copilot/rewrite",
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
    return mapDaemonResponse(daemonResp);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
  }
);
