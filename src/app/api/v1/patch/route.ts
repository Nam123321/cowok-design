// @story 05.4
// @implements AC-02, AC-03, AC-04, AC-05
import { guardedRoute } from "@/lib/api-guard";
import {
  buildSsotErrorResponse,
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  ScreenPatchHistoryResponseSchema,
  SurgicalPatchRequestSchema,
  SurgicalPatchResponseSchema,
} from "@/lib/validations/sketch-polish";

const HTTP_UNPROCESSABLE = 422;
const MAX_PATCH_BODY_BYTES = 524_288;

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const projectId = url.searchParams.get("projectId")?.trim() ?? "";
    const screenId = url.searchParams.get("screenId")?.trim() ?? "";
    if (!projectId || !screenId) {
      return buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "projectId and screenId query parameters are required",
        requestId,
        field: projectId ? "screenId" : "projectId",
      });
    }
    const qs = new URLSearchParams({ projectId, screenId }).toString();
    return proxyAndValidateSsotDaemon({
      method: "GET",
      daemonPath: "/api/v1/patch",
      rawQuery: qs,
      req,
      ctx,
      requestId,
      successSchema: ScreenPatchHistoryResponseSchema,
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
    const parsed = await parseJsonBodyStrict(
      req,
      SurgicalPatchRequestSchema,
      requestId,
      MAX_PATCH_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/patch",
      body: parsed.data,
      req,
      ctx,
      requestId,
      successSchema: SurgicalPatchResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: SurgicalPatchRequestSchema,
  }
);
