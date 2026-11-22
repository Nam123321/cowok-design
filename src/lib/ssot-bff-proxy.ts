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

const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
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

export async function readBoundedRequestBytes(
  req: Request,
  maxBytes: number
): Promise<Uint8Array | "EMPTY" | "TOO_LARGE"> {
  if (!req.body) {
    const rawBuf = new Uint8Array(await req.arrayBuffer());
    if (rawBuf.byteLength === 0) return "EMPTY";
    return rawBuf.byteLength > maxBytes ? "TOO_LARGE" : rawBuf;
  }
  const reader = req.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (!value || value.byteLength === 0) continue;
    total += value.byteLength;
    if (total > maxBytes) {
      await reader.cancel().catch(() => undefined);
      return "TOO_LARGE";
    }
    chunks.push(value);
  }
  if (total === 0) return "EMPTY";
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return merged;
}

export async function parseJsonBodyStrict<T>(
  req: Request,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string,
  maxBytes = DEFAULT_MAX_BODY_BYTES
): Promise<{ ok: true; data: T } | { ok: false; response: Response }> {
  const rawBuf = await readBoundedRequestBytes(req, maxBytes);
  if (rawBuf === "EMPTY" || rawBuf === "TOO_LARGE") {
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
  const msg =
    typeof jsonBody === "object" &&
    jsonBody !== null &&
    typeof (jsonBody as { message?: unknown }).message === "string"
      ? (jsonBody as { message: string }).message
      : "Request rejected by daemon guard";
  let code: SsotApiErrorCode = "INVALID_REQUEST";
  if (status === HTTP_UNAUTHORIZED) {
    code = "UNAUTHORIZED";
  } else if (status === HTTP_FORBIDDEN) {
    code = "AUTH_FORBIDDEN";
  }
  return buildSsotErrorResponse({ status, code, message: msg, requestId });
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

  const flatError = !daemonResp.ok
    ? mapFlatDaemonAuthError(daemonResp.status, jsonBody, requestId)
    : null;
  if (flatError) return flatError;

  return buildSsotErrorResponse({
    status: HTTP_SERVICE_UNAVAILABLE,
    code: "INTERNAL_ERROR",
    message: "Daemon SSOT contract violation",
    requestId,
  });
}

function extractPortalExtraHeaders(
  req: Request,
  requestId: string
): { ok: true; extraHeaders?: Record<string, string> } | { ok: false; response: Response } {
  const rawPortalHeader = req.headers.get("x-cowok-portal");
  if (rawPortalHeader === null) return { ok: true };
  const trimmed = rawPortalHeader.trim();
  if (trimmed.length === 0) {
    return {
      ok: false,
      response: buildSsotErrorResponse({
        status: HTTP_FORBIDDEN,
        code: "CROSS_PORTAL_LEAK_BLOCKED",
        message: "Empty x-cowok-portal header rejected",
        requestId,
        field: "portalId",
      }),
    };
  }
  return { ok: true, extraHeaders: { "X-Cowok-Portal": trimmed } };
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
  const portalCheck = extractPortalExtraHeaders(params.req, params.requestId);
  if (!portalCheck.ok) return portalCheck.response;
  const daemonResp = await forwardToDaemonFlow({
    method: params.method,
    daemonPath: params.daemonPath,
    rawQuery: params.rawQuery,
    body: params.body,
    inboundHeaders: params.req.headers,
    ...(portalCheck.extraHeaders ? { extraHeaders: portalCheck.extraHeaders } : {}),
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
