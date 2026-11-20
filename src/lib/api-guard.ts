// @story 04.1
// @implements AC-06
import crypto from "node:crypto";
import { z, ZodSchema } from "zod";
import { jsonResp, verifyStrictLoopback } from "./api-guard-session.ts";

export {
  getBffSessionRegistrySize,
  getBoundBffPort,
  isAllowedLoopbackHost,
  issueBffLoopbackSession,
  pruneExpiredBffSessions,
  setBffSessionHumanToken,
  setBoundBffPort,
  type LoopbackSessionRecord,
} from "./api-guard-session.ts";

const HTTP_BAD_REQUEST = 400;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_INTERNAL_ERROR = 500;

export interface GuardedContext {
  userId?: string;
  tenantId?: string;
  role?: "viewer" | "editor" | "admin";
  projectId?: string;
  correlationId?: string;
  params?: Record<string, string>;
  bffSessionToken?: string;
  isHumanWorkbench?: boolean;
  humanSessionToken?: string;
}

export interface GuardedRouteOptions<B = unknown, Q = unknown> {
  body?: ZodSchema<B>;
  rawJsonPassthrough?: boolean;
  query?: ZodSchema<Q>;
  roles?: string[];
  authMode?: "default" | "strict-loopback";
  requireHumanAction?: boolean;
  readOnlyPost?: boolean;
  maxBodyBytes?: number;
  maxRawHtmlBytes?: number;
}

type RouteHandler<B, Q> = (
  req: Request,
  ctx: GuardedContext,
  data: { body?: B; query?: Q }
) => Promise<Response> | Response;

function resolveAuthIdentity<B, Q>(
  req: Request,
  url: URL,
  options: GuardedRouteOptions<B, Q>,
  _context: { tenantId?: string }
): {
  ok: true;
  tenantId: string;
  userId: string;
  role: "viewer" | "editor" | "admin";
  sessionToken: string;
  isHumanWorkbench?: boolean;
  humanSessionToken?: string;
} | { ok: false; response: Response } {
  const verified = verifyStrictLoopback(req, url, { allowViewerReadOnlyPost: Boolean(options.readOnlyPost) });
  if (!verified.ok) return verified;
  if (options.roles && options.roles.length > 0) {
    const allowed = options.roles.map((r) => r.toLowerCase());
    if (!allowed.includes(verified.session.role) && verified.session.role !== "admin") {
      return { ok: false, response: jsonResp(HTTP_FORBIDDEN, { error: "FORBIDDEN", message: "Insufficient permissions" }) };
    }
  }
  if (options.requireHumanAction && !verified.session.isHumanWorkbench) {
    return {
      ok: false,
      response: jsonResp(HTTP_FORBIDDEN, {
        code: "HUMAN_APPROVAL_REQUIRED",
        error: "HUMAN_APPROVAL_REQUIRED",
        message: "Verified human workbench session required",
      }),
    };
  }
  return { ok: true, ...verified.session, sessionToken: verified.sessionToken };
}

async function parseGuardedBody<B, Q>(
  req: Request,
  options: GuardedRouteOptions<B, Q>
): Promise<{ ok: true; body: B | undefined } | { ok: false; response: Response }> {
  if (req.method === "GET" || req.method === "HEAD" || (!options.body && !options.rawJsonPassthrough)) {
    return { ok: true, body: undefined };
  }
  let rawText: string;
  try {
    rawText = await req.text();
  } catch {
    return {
      ok: false,
      response: jsonResp(HTTP_BAD_REQUEST, {
        code: "INVALID_JSON",
        error: "INVALID_JSON",
        message: "Failed to read request body",
      }),
    };
  }
  if (typeof options.maxBodyBytes === "number" && new TextEncoder().encode(rawText).length > options.maxBodyBytes) {
    return {
      ok: false,
      response: jsonResp(HTTP_PAYLOAD_TOO_LARGE, {
        error: { code: "E_PAYLOAD_TOO_LARGE", message: `Payload exceeds ${options.maxBodyBytes} bytes`, retryable: false },
      }),
    };
  }
  if (rawText.trim() === "") {
    return { ok: true, body: options.body ? options.body.parse({}) : undefined };
  }
  let jsonVal: unknown;
  try {
    jsonVal = JSON.parse(rawText);
  } catch {
    return {
      ok: false,
      response: jsonResp(HTTP_BAD_REQUEST, {
        code: "INVALID_JSON",
        error: "INVALID_JSON",
        message: "Malformed JSON request body",
      }),
    };
  }
  if (
    typeof options.maxRawHtmlBytes === "number" &&
    jsonVal &&
    typeof jsonVal === "object" &&
    typeof (jsonVal as { rawHtml?: unknown }).rawHtml === "string" &&
    new TextEncoder().encode((jsonVal as { rawHtml: string }).rawHtml).length > options.maxRawHtmlBytes
  ) {
    return {
      ok: false,
      response: jsonResp(HTTP_PAYLOAD_TOO_LARGE, {
        error: { code: "E_PAYLOAD_TOO_LARGE", message: `rawHtml exceeds ${options.maxRawHtmlBytes} UTF-8 bytes`, retryable: false },
      }),
    };
  }
  return { ok: true, body: options.body ? options.body.parse(jsonVal) : (jsonVal as B) };
}

async function executeGuardedRequest<B, Q>(
  req: Request,
  context: { params?: Record<string, string>; projectId?: string; tenantId?: string },
  options: GuardedRouteOptions<B, Q>,
  handler: RouteHandler<B, Q>
): Promise<Response> {
  try {
    const url = new URL(req.url, "http://127.0.0.1");
    const auth = resolveAuthIdentity(req, url, options, context);
    if (!auth.ok) return auth.response;

    const bodyOutcome = await parseGuardedBody(req, options);
    if (!bodyOutcome.ok) return bodyOutcome.response;

    const parsedQuery = options.query
      ? options.query.parse(Object.fromEntries(url.searchParams.entries()))
      : undefined;

    const ctx: GuardedContext = {
      userId: auth.userId,
      tenantId: auth.tenantId,
      role: auth.role,
      projectId: context.projectId || context.params?.id || "",
      correlationId: req.headers.get("x-correlation-id") || crypto.randomUUID(),
      params: context.params || {},
      bffSessionToken: auth.sessionToken,
      isHumanWorkbench: auth.isHumanWorkbench,
      humanSessionToken: auth.humanSessionToken,
    };
    return await handler(req, ctx, { body: bodyOutcome.body, query: parsedQuery });
  } catch (err: unknown) {
    if (err instanceof z.ZodError) {
      return jsonResp(HTTP_BAD_REQUEST, { code: "VALIDATION_ERROR", error: "VALIDATION_ERROR", details: err.errors });
    }
    const message = err instanceof Error ? err.message : "INTERNAL_SERVER_ERROR";
    return jsonResp(HTTP_INTERNAL_ERROR, { code: "INTERNAL_ERROR", error: "INTERNAL_ERROR", message });
  }
}

/** Wraps an HTTP route handler with strict-loopback auth, Zod body/query validation, and context extraction. */
export function guardedRoute<B = unknown, Q = unknown>(
  handlerOrOptions: GuardedRouteOptions<B, Q> | RouteHandler<B, Q>,
  optionsOrHandler?: GuardedRouteOptions<B, Q> | RouteHandler<B, Q>
) {
  const isFirstFn = typeof handlerOrOptions === "function";
  const handler = (isFirstFn ? handlerOrOptions : optionsOrHandler) as RouteHandler<B, Q>;
  const options = ((isFirstFn ? optionsOrHandler : handlerOrOptions) || {}) as GuardedRouteOptions<B, Q>;
  return (req: Request, context: { params?: Record<string, string>; projectId?: string; tenantId?: string } = {}) =>
    executeGuardedRequest(req, context, options, handler);
}
