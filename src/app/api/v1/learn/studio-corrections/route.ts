// @story 08.7
// @implements AC-04 (BFF Proxy for POST /api/v1/learn/studio-corrections — Bridge 1 Zero-PII)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  AiHarnessErrorEnvelopeSchema,
  RecordStudioCorrectionEnvelopeSchema,
  RecordStudioCorrectionRequestSchema,
} from "@/lib/validations/ai-harness";

const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };
const MAX_CORRECTION_BYTES = 8_192;
const HTTP_UNPROCESSABLE_ENTITY = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;

export const POST = guardedRoute(
  async (req, ctx, data) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const validatedBody = data.body;
    if (!validatedBody) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INVALID_REQUEST",
            message: "Missing studio correction request body",
            field: "body",
            requestId,
          },
        }),
        { status: HTTP_UNPROCESSABLE_ENTITY, headers: JSON_HEADERS }
      );
    }

    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/learn/studio-corrections",
      body: validatedBody,
      inboundHeaders: req.headers,
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

    try {
      const body: unknown = JSON.parse(await daemonResp.text());
      const ok = RecordStudioCorrectionEnvelopeSchema.safeParse(body);
      if (daemonResp.ok && ok.success) {
        return new Response(JSON.stringify(ok.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        });
      }
      const err = AiHarnessErrorEnvelopeSchema.safeParse(body);
      if (!daemonResp.ok && err.success) {
        return new Response(JSON.stringify(err.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        });
      }
    } catch {
      // Fail closed below
    }
    return new Response(
      JSON.stringify({
        error: {
          code: "INTERNAL_ERROR",
          message: "Daemon /learn/studio-corrections contract violation",
          requestId,
        },
      }),
      { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
    );
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    requireHumanAction: true,
    maxBodyBytes: MAX_CORRECTION_BYTES,
    body: RecordStudioCorrectionRequestSchema,
  }
);
