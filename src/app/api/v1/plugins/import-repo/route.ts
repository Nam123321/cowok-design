// @story 08.5
// @implements AC-03 (GET/POST /api/v1/plugins/import-repo BFF proxy -> V20 user_skill_registry)
import { z } from "zod";
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type DeRuntimeErrorCode,
  DeRuntimeErrorEnvelopeSchema,
  ImportUserSkillRequestSchema,
  ImportUserSkillResponseSchema,
  ListUserSkillsResponseSchema,
} from "@/lib/validations/de-runtime";

const MAX_IMPORT_BODY_BYTES = 131_072;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function mapFlatAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  let code: DeRuntimeErrorCode = "INVALID_REQUEST";
  if (status === HTTP_UNAUTHORIZED) code = "UNAUTHORIZED";
  else if (status === HTTP_FORBIDDEN) code = "AUTH_FORBIDDEN";
  else if (status === HTTP_PAYLOAD_TOO_LARGE) code = "PAYLOAD_TOO_LARGE";
  return new Response(JSON.stringify({ error: { code, message: msg, requestId } }), {
    status,
    headers: JSON_HEADERS,
  });
}

async function validateImportDaemonResp<T>(
  daemonResp: Response,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string
): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = schema.safeParse(parsed);
    if (daemonResp.ok && okData.success) {
      return new Response(JSON.stringify(okData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const errData = DeRuntimeErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) {
      return new Response(JSON.stringify(errData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const flatErr = !daemonResp.ok ? mapFlatAuthErr(daemonResp.status, parsed, requestId) : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed contract error
  }
  return new Response(
    JSON.stringify({
      error: {
        code: "INTERNAL_ERROR",
        message: "Daemon user-skill import contract violation",
        requestId,
      },
    }),
    { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
  );
}

async function proxyImportRepo<T>(params: {
  method: "GET" | "POST";
  rawQuery?: string;
  body?: unknown;
  req: Request;
  ctx: GuardedContext;
  requestId: string;
  schema: z.ZodType<T, z.ZodTypeDef, unknown>;
}): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: params.method,
    daemonPath: "/api/v1/plugins/import-repo",
    rawQuery: params.rawQuery,
    body: params.body,
    inboundHeaders: params.req.headers,
    principal: {
      tenantId: params.ctx.tenantId || "local-default",
      userId: params.ctx.userId || "local-operator",
      role: params.ctx.role || "viewer",
    },
    correlationId: params.requestId,
    bffSessionToken: params.ctx.bffSessionToken,
    isHumanWorkbench: params.ctx.isHumanWorkbench,
    humanSessionToken: params.ctx.humanSessionToken,
    signal: params.req.signal,
  });
  return validateImportDaemonResp(daemonResp, params.schema, params.requestId);
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const projectId = url.searchParams.get("projectId")?.trim() ?? "";
    const rawQuery = projectId ? new URLSearchParams({ projectId }).toString() : undefined;
    return proxyImportRepo({
      method: "GET",
      rawQuery,
      req,
      ctx,
      requestId,
      schema: ListUserSkillsResponseSchema,
    });
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      ImportUserSkillRequestSchema,
      requestId,
      MAX_IMPORT_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return proxyImportRepo({
      method: "POST",
      body: parsed.data,
      req,
      ctx,
      requestId,
      schema: ImportUserSkillResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: ImportUserSkillRequestSchema,
  }
);
