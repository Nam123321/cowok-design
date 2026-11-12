// @story 04.1
// @implements AC-06
import crypto from "node:crypto";
import { z, ZodSchema } from "zod";

export interface LoopbackSessionRecord {
  tenantId: string;
  userId: string;
  role: "viewer" | "editor" | "admin";
  expiresAt: number;
}

const sessionRegistry = new Map<string, LoopbackSessionRecord>();
const SESSION_TTL_MS = 8 * 60 * 60 * 1_000;

export function issueBffLoopbackSession(input?: {
  tenantId?: string;
  userId?: string;
  role?: "viewer" | "editor" | "admin";
}): { sessionToken: string } & LoopbackSessionRecord {
  const token = `cwk_bff_${crypto.randomBytes(24).toString("hex")}`;
  const rec: LoopbackSessionRecord = {
    tenantId: input?.tenantId?.trim() || "local-default",
    userId: input?.userId?.trim() || "local-operator",
    role: input?.role || "editor",
    expiresAt: Date.now() + SESSION_TTL_MS,
  };
  sessionRegistry.set(token, rec);
  return { sessionToken: token, ...rec };
}

export function isAllowedLoopbackHost(rawHost: string): boolean {
  const host = rawHost.trim().toLowerCase();
  if (host === "127.0.0.1" || host === "localhost") return true;
  const m = /^(127\.0\.0\.1|localhost):(\d{1,5})$/.exec(host);
  if (!m) return false;
  const port = Number(m[2]);
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

function jsonResp(status: number, body: Record<string, unknown>): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function verifyStrictLoopback(
  req: Request,
  fallbackUrl: URL
): { ok: true; session: LoopbackSessionRecord } | { ok: false; response: Response } {
  const hostHeader = req.headers.get("host") || fallbackUrl.host;
  if (!isAllowedLoopbackHost(hostHeader)) {
    return { ok: false, response: jsonResp(403, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" }) };
  }
  const sfs = req.headers.get("sec-fetch-site");
  if (sfs && sfs !== "same-origin" && sfs !== "none") {
    return { ok: false, response: jsonResp(403, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" }) };
  }
  const origin = req.headers.get("origin");
  if (origin) {
    let originHost = "";
    try {
      originHost = new URL(origin).host;
    } catch {
      originHost = "";
    }
    if (!originHost || !isAllowedLoopbackHost(originHost)) {
      return { ok: false, response: jsonResp(403, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" }) };
    }
  }
  const token = req.headers.get("x-cowok-session-token")?.trim() || "";
  const session = sessionRegistry.get(token);
  if (!session || session.expiresAt < Date.now()) {
    return { ok: false, response: jsonResp(401, { code: "UNAUTHENTICATED", error: "UNAUTHENTICATED" }) };
  }
  if (session.role === "viewer" && req.method !== "GET" && req.method !== "HEAD") {
    return { ok: false, response: jsonResp(403, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE" }) };
  }
  return { ok: true, session };
}

export interface GuardedContext {
  userId?: string;
  tenantId?: string;
  role?: "viewer" | "editor" | "admin";
  projectId?: string;
  correlationId?: string;
  params?: Record<string, string>;
}

export interface GuardedRouteOptions<B = unknown, Q = unknown> {
  body?: ZodSchema<B>;
  query?: ZodSchema<Q>;
  roles?: string[];
  authMode?: "default" | "strict-loopback";
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
  context: { tenantId?: string }
): { ok: true; tenantId: string; userId: string; role: "viewer" | "editor" | "admin" } | { ok: false; response: Response } {
  if (options.authMode === "strict-loopback") {
    const verified = verifyStrictLoopback(req, url);
    if (!verified.ok) return verified;
    return { ok: true, ...verified.session };
  }
  if (options.roles && options.roles.length > 0) {
    const raw = req.headers.get("x-user-roles") || "";
    const userRoles = raw ? raw.split(",").map((r) => r.trim().toLowerCase()) : ["editor", "admin"];
    const allowed = options.roles.map((r) => r.toLowerCase());
    if (!allowed.some((r) => userRoles.includes(r) || userRoles.includes("admin"))) {
      return { ok: false, response: jsonResp(403, { error: "FORBIDDEN", message: "Insufficient permissions" }) };
    }
  }
  return {
    ok: true,
    tenantId: context.tenantId || req.headers.get("x-tenant-id") || "default-tenant",
    userId: req.headers.get("x-user-id") || "anonymous",
    role: "editor",
  };
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

    const parsedBody =
      options.body && req.method !== "GET" && req.method !== "HEAD"
        ? options.body.parse(await req.json().catch(() => ({})))
        : undefined;
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
    };
    return await handler(req, ctx, { body: parsedBody, query: parsedQuery });
  } catch (err: unknown) {
    if (err instanceof z.ZodError) {
      return jsonResp(400, { code: "VALIDATION_ERROR", error: "VALIDATION_ERROR", details: err.errors });
    }
    const message = err instanceof Error ? err.message : "INTERNAL_SERVER_ERROR";
    return jsonResp(500, { code: "INTERNAL_ERROR", error: message });
  }
}

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
