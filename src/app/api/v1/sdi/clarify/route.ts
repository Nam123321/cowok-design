// @story 05.4
// @implements AC-01
import { guardedRoute } from "@/lib/api-guard";
import {
  buildSsotErrorResponse,
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  ClarifyDirectionRequestSchema,
  ClarifyDirectionResponseSchema,
  GetClarifySessionResponseSchema,
} from "@/lib/validations/sketch-polish";

const HTTP_UNPROCESSABLE = 422;

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const projectId = url.searchParams.get("projectId")?.trim() ?? "";
    if (!projectId) {
      return buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "projectId query parameter is required",
        requestId,
        field: "projectId",
      });
    }
    const qs = new URLSearchParams({ projectId }).toString();
    return proxyAndValidateSsotDaemon({
      method: "GET",
      daemonPath: "/api/v1/sdi/clarify",
      rawQuery: qs,
      req,
      ctx,
      requestId,
      successSchema: GetClarifySessionResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, ClarifyDirectionRequestSchema, requestId);
    if (!parsed.ok) return parsed.response;

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/sdi/clarify",
      body: parsed.data,
      req,
      ctx,
      requestId,
      successSchema: ClarifyDirectionResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: ClarifyDirectionRequestSchema,
  }
);
