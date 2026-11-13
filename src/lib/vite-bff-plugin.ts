// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import crypto from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";
import { Readable } from "node:stream";
import type { ReadableStream as WebReadableStream } from "node:stream/web";
import type { Connect, Plugin, ViteDevServer } from "vite";
import { isAllowedLoopbackHost, issueBffLoopbackSession } from "./api-guard";
import { LocalSessionBootstrapInputSchema } from "./validations/harness-daemon";

const MAX_BODY_BYTES = 1_048_576;
const HTTP_OK = 200;
const HTTP_BAD_REQUEST = 400;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;

type RouteFn = (req: Request, ctx: { params?: Record<string, string> }) => Promise<Response>;
type RouteMatcher = (
  method: string,
  pathname: string
) => { handler: RouteFn; params: Record<string, string> } | null;

let cachedMatcher: RouteMatcher | null = null;
let previewViteServerPromise: Promise<ViteDevServer> | null = null;

async function resolveRouteMatcher(server?: ViteDevServer): Promise<RouteMatcher> {
  if (cachedMatcher) return cachedMatcher;
  if (server) {
    const mod = await server.ssrLoadModule("/src/lib/vite-bff-routes.ts");
    cachedMatcher = mod.matchBffRoute as RouteMatcher;
    return cachedMatcher;
  }
  try {
    const routeModuleSpecifier = ["./vite-bff", "routes"].join("-");
    const mod = (await import(/* @vite-ignore */ routeModuleSpecifier)) as {
      matchBffRoute: RouteMatcher;
    };
    cachedMatcher = mod.matchBffRoute;
    return cachedMatcher;
  } catch {
    // In `vite preview`, there is no outer ViteDevServer; boot an internal headless SSR module loader once.
    if (!previewViteServerPromise) {
      previewViteServerPromise = import("vite").then((v) =>
        v.createServer({
          server: { middlewareMode: true },
          appType: "custom",
          optimizeDeps: { noDiscovery: true },
        })
      );
    }
    const ssrServer = await previewViteServerPromise;
    const mod = await ssrServer.ssrLoadModule("/src/lib/vite-bff-routes.ts");
    cachedMatcher = mod.matchBffRoute as RouteMatcher;
    return cachedMatcher;
  }
}

function readRawBody(req: IncomingMessage): Promise<Buffer | "TOO_LARGE"> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let total = 0;
    let overflow = false;
    req.on("data", (chunk: Buffer) => {
      total += chunk.length;
      if (total > MAX_BODY_BYTES) overflow = true;
      else chunks.push(chunk);
    });
    req.on("end", () => resolve(overflow ? "TOO_LARGE" : Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

function writeJson(res: ServerResponse, statusCode: number, payload: unknown): void {
  res.statusCode = statusCode;
  res.setHeader("Content-Type", "application/json");
  res.end(JSON.stringify(payload));
}

function sendWebResponse(res: ServerResponse, webRes: Response): Promise<void> {
  res.statusCode = webRes.status;
  webRes.headers.forEach((value, key) => res.setHeader(key, value));

  const contentType = webRes.headers.get("content-type") || "";
  const isStream =
    Boolean(webRes.body) &&
    (contentType.includes("text/event-stream") ||
      webRes.headers.get("transfer-encoding") === "chunked");

  if (isStream && webRes.body) {
    if (typeof res.flushHeaders === "function") {
      res.flushHeaders();
    }
    const nodeStream = Readable.fromWeb(webRes.body as unknown as WebReadableStream<Uint8Array>);
    return new Promise((resolve, reject) => {
      nodeStream.on("error", reject);
      res.on("error", reject);
      res.on("close", resolve);
      res.on("finish", resolve);
      nodeStream.pipe(res);
    });
  }

  return webRes.arrayBuffer().then((buf) => {
    res.end(Buffer.from(buf));
  });
}

function verifyBootstrapOrigin(req: IncomingMessage): boolean {
  const host = req.headers.host;
  if (typeof host !== "string" || !host.trim()) return false;
  if (!isAllowedLoopbackHost(host)) return false;
  const sfs = String(req.headers["sec-fetch-site"] || "");
  if (sfs && sfs !== "same-origin" && sfs !== "none") return false;
  const origin = req.headers.origin;
  if (!origin) return true;
  try {
    return isAllowedLoopbackHost(new URL(String(origin)).host);
  } catch {
    return false;
  }
}

function isAuthorizedAdminBearer(authHeader: string | undefined): boolean {
  const expected = process.env.COWOK_ADMIN_TOKEN?.trim() || "";
  if (!expected || !authHeader) return false;
  const supplied = authHeader.startsWith("Bearer ") ? authHeader.slice("Bearer ".length).trim() : "";
  if (!supplied || supplied.length !== expected.length) return false;
  return crypto.timingSafeEqual(Buffer.from(supplied, "utf8"), Buffer.from(expected, "utf8"));
}

export async function handleBootstrapSession(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (!verifyBootstrapOrigin(req)) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" });
  }
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    return writeJson(res, HTTP_PAYLOAD_TOO_LARGE, { code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" });
  }
  let rawJson: unknown = {};
  if (rawBuf.length > 0) {
    try {
      rawJson = JSON.parse(rawBuf.toString("utf8"));
    } catch {
      return writeJson(res, HTTP_BAD_REQUEST, {
        code: "INVALID_JSON",
        error: "INVALID_JSON",
        message: "Malformed JSON request body",
      });
    }
  }
  const parsed = LocalSessionBootstrapInputSchema.safeParse(rawJson);
  if (!parsed.success) {
    return writeJson(res, HTTP_BAD_REQUEST, {
      code: "VALIDATION_ERROR",
      error: "VALIDATION_ERROR",
      details: parsed.error.issues,
    });
  }
  if (parsed.data.role === "admin" && !isAuthorizedAdminBearer(req.headers.authorization)) {
    return writeJson(res, HTTP_FORBIDDEN, {
      code: "INSUFFICIENT_ROLE",
      error: "INSUFFICIENT_ROLE",
      message: "Unauthenticated local-session bootstrap cannot issue admin role",
    });
  }
  const requestedTenant = parsed.data.tenantId?.trim() || "local-default";
  if (requestedTenant !== "local-default" && !isAuthorizedAdminBearer(req.headers.authorization)) {
    return writeJson(res, HTTP_FORBIDDEN, {
      code: "TENANT_ISOLATION_BREACH",
      error: "TENANT_ISOLATION_BREACH",
      message: "Unauthenticated local-session bootstrap cannot switch away from local-default tenant",
    });
  }
  const issued = issueBffLoopbackSession({
    tenantId: requestedTenant,
    userId: parsed.data.userId,
    role: parsed.data.role,
  });
  return writeJson(res, HTTP_OK, issued);
}

export async function dispatchMiddlewareRequest(
  req: IncomingMessage,
  res: ServerResponse,
  next: Connect.NextFunction,
  server?: ViteDevServer
): Promise<void> {
  const host = String(req.headers.host || "127.0.0.1");
  const parsedUrl = new URL(req.url || "/", `http://${host}`);
  const method = (req.method || "GET").toUpperCase();
  if (parsedUrl.pathname === "/api/auth/local-session" && method === "POST") {
    return handleBootstrapSession(req, res);
  }
  const matchFn = await resolveRouteMatcher(server);
  const matched = matchFn(method, parsedUrl.pathname);
  if (!matched) {
    next();
    return;
  }
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    return writeJson(res, HTTP_PAYLOAD_TOO_LARGE, { code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" });
  }
  const headers = new Headers();
  for (const [k, v] of Object.entries(req.headers)) {
    if (Array.isArray(v)) headers.set(k, v.join(", "));
    else if (typeof v === "string") headers.set(k, v);
  }
  const webReq = new Request(parsedUrl.toString(), {
    method,
    headers,
    body: method === "GET" || method === "HEAD" || rawBuf.length === 0 ? undefined : new Uint8Array(rawBuf),
  });
  await sendWebResponse(res, await matched.handler(webReq, { params: matched.params }));
}

export function cowokLoopbackBffPlugin(): Plugin {
  return {
    name: "cowok-loopback-bff",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        dispatchMiddlewareRequest(req, res, next, server).catch(next);
      });
    },
    configurePreviewServer(server) {
      server.middlewares.use((req, res, next) => {
        dispatchMiddlewareRequest(req, res, next).catch(next);
      });
    },
  };
}
