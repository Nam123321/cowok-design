// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import type { GuardedContext } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";
import {
  type SsotApiError,
  type SsotApiErrorCode,
  SsotApiErrorSchema,
} from "./validations/ssot-registry";

const HTTP_UNPROCESSABLE = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;
const DEFAULT_MAX_BODY_BYTES = 65_536;
const DEFAULT_RETRY_AFTER_SECS = 1;

export function buildSsotErrorResponse(params: {
  status: number;
  code: SsotApiErrorCode;
  message: string;
  requestId: string;
  field?: string;
  details?: Record<string, unknown>;
  retryAfterSeconds?: number;
}): Response {
  const payload: SsotApiError = {
    error: {
      code: params.code,
      message: params.message,
      requestId: params.requestId,
      ...(params.field ? { field: params.field } : {}),
      ...(params.details ? { details: params.details } : {}),
    },
  };
  const headers: Record<string, string> = {
    "Content-Type": "application/json; charset=utf-8",
  };
  if (params.code === "STORAGE_BUSY") {
    headers["Retry-After"] = String(params.retryAfterSeconds ?? DEFAULT_RETRY_AFTER_SECS);
  }
  return new Response(JSON.stringify(payload), { status: params.status, headers });
}

export function firstSsotZodField(err: z.ZodError): string | undefined {
  const issue = err.issues[0];
  if (!issue) return undefined;
  if (issue.code === "unrecognized_keys" && issue.keys.length > 0) {
    return issue.keys[0];
  }
  return issue.path.length > 0 ? String(issue.path[0]) : undefined;
}

export function resolveRequestCorrelationId(ctx: GuardedContext): string {
  const cid = ctx.correlationId?.trim();
  return cid && cid.length > 0 ? cid : globalThis.crypto.randomUUID();
}

export async function parseJsonBodyStrict<T>(
  req: Request,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string,
  maxBytes = DEFAULT_MAX_BODY_BYTES
): Promise<{ ok: true; data: T } | { ok: false; response: Response }> {
  const rawBuf = new Uint8Array(await req.arrayBuffer());
  if (rawBuf.byteLength === 0 || rawBuf.byteLength > maxBytes) {
    return {
      ok: false,
      response: buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "Request body is empty or exceeds size limit",
        requestId,
        field: "body",
      }),
    };
  }
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(rawBuf));
  } catch {
    return {
      ok: false,
      response: buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "Malformed UTF-8 JSON body",
        requestId,
        field: "body",
      }),
    };
  }
  const checked = schema.safeParse(parsedJson);
  if (!checked.success) {
    return {
      ok: false,
      response: buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: checked.error.issues[0]?.message || "Request validation failed",
        requestId,
        field: firstSsotZodField(checked.error),
      }),
    };
  }
  return { ok: true, data: checked.data };
}

function buildValidatedDaemonResponse<T>(
  daemonResp: Response,
  jsonBody: unknown,
  successSchema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string
): Response {
  const okCheck = successSchema.safeParse(jsonBody);
  if (daemonResp.ok && okCheck.success) {
    return new Response(JSON.stringify(okCheck.data), {
      status: daemonResp.status,
      headers: { "Content-Type": "application/json; charset=utf-8" },
    });
  }

  const errCheck = SsotApiErrorSchema.safeParse(jsonBody);
  if (!daemonResp.ok && errCheck.success) {
    const outHeaders = new Headers({ "Content-Type": "application/json; charset=utf-8" });
    const retryAfter = daemonResp.headers.get("retry-after");
    if (errCheck.data.error.code === "STORAGE_BUSY" && retryAfter) {
      outHeaders.set("Retry-After", retryAfter);
    }
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers: outHeaders,
    });
  }

  return buildSsotErrorResponse({
    status: HTTP_SERVICE_UNAVAILABLE,
    code: "INTERNAL_ERROR",
    message: "Daemon SSOT contract violation",
    requestId,
  });
}

export async function proxyAndValidateSsotDaemon<T>(params: {
  method: "GET" | "POST";
  daemonPath: string;
  rawQuery?: string;
  body?: unknown;
  req: Request;
  ctx: GuardedContext;
  requestId: string;
  successSchema: z.ZodType<T, z.ZodTypeDef, unknown>;
}): Promise<Response> {
  const rawPortalHeader = params.req.headers.get("x-cowok-portal");
  const daemonResp = await forwardToDaemonFlow({
    method: params.method,
    daemonPath: params.daemonPath,
    rawQuery: params.rawQuery,
    body: params.body,
    inboundHeaders: params.req.headers,
    ...(rawPortalHeader !== null ? { extraHeaders: { "X-Cowok-Portal": rawPortalHeader.trim() } } : {}),
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

  const rawText = await daemonResp.text();
  try {
    const parsed: unknown = JSON.parse(rawText);
    return buildValidatedDaemonResponse(daemonResp, parsed, params.successSchema, params.requestId);
  } catch {
    return buildSsotErrorResponse({
      status: HTTP_SERVICE_UNAVAILABLE,
      code: "INTERNAL_ERROR",
      message: "Daemon returned non-JSON response",
      requestId: params.requestId,
    });
  }
}
