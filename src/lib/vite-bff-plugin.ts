// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import fs from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { Readable } from "node:stream";
import type { ReadableStream as WebReadableStream } from "node:stream/web";
import { pathToFileURL } from "node:url";
import type { Connect, Plugin, PreviewServer, ViteDevServer } from "vite";
import { setBoundBffPort } from "./api-guard.ts";
import {
  HTTP_FORBIDDEN,
  HTTP_OK,
  HTTP_PAYLOAD_TOO_LARGE,
  MAX_BODY_BYTES,
  MAX_CAS_UPLOAD_BODY_BYTES,
  buildWorkbenchProofCookieHeader,
  handleBootstrapSession,
  injectBootCapMeta,
  issueBootCapToken,
  readRawBody,
  revokeBootCapToken,
  writeJson,
} from "./vite-bff-bootstrap.ts";

export { handleBootstrapSession, issueBootCapToken, revokeBootCapToken } from "./vite-bff-bootstrap.ts";

type RouteFn = (req: Request, ctx: { params?: Record<string, string> }) => Promise<Response>;
type RouteMatcher = (
  method: string,
  pathname: string
) => { handler: RouteFn; params: Record<string, string> } | null;

let cachedMatcher: RouteMatcher | null = null;
let previewViteServerPromise: Promise<ViteDevServer> | null = null;

/** Closes any headless ViteDevServer created for `vite preview` SSR route loading. */
async function closePreviewBffServer(): Promise<void> {
  cachedMatcher = null;
  const pending = previewViteServerPromise;
  previewViteServerPromise = null;
  if (!pending) return;
  try {
    const ssrServer = await pending;
    await ssrServer.close();
  } catch {
    // ignore teardown failures during preview shutdown
  }
}

async function tryImportPrecompiledRouteMatcher(): Promise<RouteMatcher | null> {
  const candidates = [
    path.resolve(process.cwd(), "dist-bff/vite-bff-routes.mjs"),
    path.resolve(process.cwd(), "dist-bff/vite-bff-routes.js"),
  ];
  for (const candidate of candidates) {
    if (!fs.existsSync(candidate)) continue;
    try {
      const mod = (await import(/* @vite-ignore */ pathToFileURL(candidate).href)) as { matchBffRoute?: RouteMatcher };
      if (typeof mod.matchBffRoute === "function") return mod.matchBffRoute;
    } catch {
      // fall through to next candidate or SSR loader
    }
  }
  try {
    const mod = (await import(/* @vite-ignore */ ["./vite-bff", "routes"].join("-"))) as { matchBffRoute?: RouteMatcher };
    if (typeof mod.matchBffRoute === "function") return mod.matchBffRoute;
  } catch {
    // not precompiled / Node ESM cannot directly execute raw TS
  }
  return null;
}

async function resolveRouteMatcher(server?: ViteDevServer): Promise<RouteMatcher> {
  if (cachedMatcher) return cachedMatcher;
  if (server) {
    const mod = await server.ssrLoadModule("/src/lib/vite-bff-routes.ts");
    cachedMatcher = mod.matchBffRoute as RouteMatcher;
    return cachedMatcher;
  }
  const precompiled = await tryImportPrecompiledRouteMatcher();
  if (precompiled) {
    cachedMatcher = precompiled;
    return cachedMatcher;
  }
  if (!previewViteServerPromise) {
    previewViteServerPromise = import("vite").then((v) =>
      v.createServer({ server: { middlewareMode: true }, appType: "custom", optimizeDeps: { noDiscovery: true } })
    );
  }
  const ssrServer = await previewViteServerPromise;
  const mod = await ssrServer.ssrLoadModule("/src/lib/vite-bff-routes.ts");
  cachedMatcher = mod.matchBffRoute as RouteMatcher;
  return cachedMatcher;
}

function sendWebResponse(res: ServerResponse, webRes: Response): Promise<void> {
  res.statusCode = webRes.status;
  webRes.headers.forEach((value, key) => res.setHeader(key, value));

  const contentType = webRes.headers.get("content-type") || "";
  const isStream =
    Boolean(webRes.body) &&
    (contentType.includes("text/event-stream") || webRes.headers.get("transfer-encoding") === "chunked");

  if (isStream && webRes.body) {
    if (typeof res.flushHeaders === "function") res.flushHeaders();
    const webBody = webRes.body;
    const nodeStream = Readable.fromWeb(webBody as unknown as WebReadableStream<Uint8Array>);
    return new Promise((resolve, reject) => {
      const teardown = () => {
        nodeStream.destroy();
        void webBody.cancel().catch(() => undefined);
      };
      nodeStream.on("error", (err) => {
        teardown();
        reject(err);
      });
      res.on("error", (err) => {
        teardown();
        reject(err);
      });
      res.on("close", () => {
        teardown();
        resolve();
      });
      res.on("finish", resolve);
      nodeStream.pipe(res);
    });
  }

  return webRes.arrayBuffer().then((buf) => {
    res.end(Buffer.from(buf));
  });
}

function isSpaHtmlNavigation(req: IncomingMessage, pathname: string): boolean {
  const method = (req.method || "GET").toUpperCase();
  if (method !== "GET" || pathname.startsWith("/api/") || pathname.startsWith("/@") || pathname.startsWith("/src/")) {
    return false;
  }
  const isPresenterRoute = /^\/presenter\/[A-Za-z0-9_.:%-]+\/?$/.test(pathname);
  const hasAssetExt = /\.[a-z0-9]{1,8}$/i.test(pathname) && pathname !== "/index.html";
  if (hasAssetExt && !isPresenterRoute) return false;
  const accept = String(req.headers.accept || "");
  if (accept && !accept.includes("text/html") && !accept.includes("*/*")) return false;
  const fetchMode = String(req.headers["sec-fetch-mode"] || "").trim().toLowerCase();
  return !fetchMode || fetchMode === "navigate";
}

/** Dispatches incoming Node HTTP requests into Vite BFF route handlers or passes through to Vite. */
async function dispatchMiddlewareRequest(
  req: IncomingMessage,
  res: ServerResponse,
  next: Connect.NextFunction,
  server?: ViteDevServer
): Promise<void> {
  const host = String(req.headers.host || "127.0.0.1");
  let parsedUrl: URL;
  try {
    parsedUrl = new URL(req.url || "/", `http://${host}`);
  } catch {
    return writeJson(res, HTTP_FORBIDDEN, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" });
  }
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
  const isCasUpload = method === "POST" && parsedUrl.pathname === "/api/v1/cas/upload";
  const rawBuf = await readRawBody(req, isCasUpload ? MAX_CAS_UPLOAD_BODY_BYTES : MAX_BODY_BYTES);
  if (rawBuf === "TOO_LARGE") {
    if (isCasUpload) {
      return writeJson(res, HTTP_PAYLOAD_TOO_LARGE, {
        code: "CAS_QUOTA_EXCEEDED",
        message: "Request upload size limit exceeded",
        retryable: false,
        limit: "REQUEST_BYTES",
      });
    }
    return writeJson(res, HTTP_PAYLOAD_TOO_LARGE, { code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" });
  }
  const headers = new Headers();
  for (const [k, v] of Object.entries(req.headers)) {
    if (Array.isArray(v)) headers.set(k, v.join(", "));
    else if (typeof v === "string") headers.set(k, v);
  }
  const reqAbort = new AbortController();
  const onReqClose = () => reqAbort.abort();
  req.on("close", onReqClose);
  res.on("close", onReqClose);
  try {
    const webReq = new Request(parsedUrl.toString(), {
      method,
      headers,
      body: method === "GET" || method === "HEAD" || rawBuf.length === 0 ? undefined : new Uint8Array(rawBuf),
      signal: reqAbort.signal,
    });
    await sendWebResponse(res, await matched.handler(webReq, { params: matched.params }));
  } finally {
    req.off("close", onReqClose);
    res.off("close", onReqClose);
  }
}

function bindServerPortLifecycle(
  httpServer: ViteDevServer["httpServer"] | PreviewServer["httpServer"],
  bootCapToken: string,
  onClose?: () => void
): void {
  httpServer?.once("listening", () => {
    const addr = httpServer?.address();
    if (addr && typeof addr === "object") setBoundBffPort(addr.port);
  });
  httpServer?.once("close", () => {
    setBoundBffPort(null);
    revokeBootCapToken(bootCapToken);
    onClose?.();
  });
}

function tryServePreviewIndexHtml(
  req: IncomingMessage,
  res: ServerResponse,
  server: PreviewServer,
  bootCapToken: string
): boolean {
  const pathname = (req.url || "/").split("?")[0];
  if (!isSpaHtmlNavigation(req, pathname)) return false;
  const distHtmlPath = path.resolve(server.config.root, server.config.build?.outDir || "dist", "index.html");
  if (!fs.existsSync(distHtmlPath)) return false;
  res.statusCode = HTTP_OK;
  res.setHeader("Content-Type", "text/html; charset=utf-8");
  res.setHeader("Cache-Control", "no-store");
  res.setHeader("Set-Cookie", buildWorkbenchProofCookieHeader(bootCapToken));
  res.end(injectBootCapMeta(fs.readFileSync(distHtmlPath, "utf8"), bootCapToken));
  return true;
}

/** Vite plugin providing the strict-loopback BFF API routes and local session bootstrapper. */
export function cowokLoopbackBffPlugin(): Plugin {
  const bootCapToken = issueBootCapToken();
  return {
    name: "cowok-loopback-bff",
    transformIndexHtml(html) {
      return injectBootCapMeta(html, bootCapToken);
    },
    configureServer(server) {
      bindServerPortLifecycle(server.httpServer, bootCapToken);
      server.middlewares.use((req, res, next) => {
        const pathname = (req.url || "/").split("?")[0];
        if (isSpaHtmlNavigation(req, pathname)) {
          res.setHeader("Set-Cookie", buildWorkbenchProofCookieHeader(bootCapToken));
        }
        dispatchMiddlewareRequest(req, res, next, server).catch(next);
      });
    },
    configurePreviewServer(server) {
      bindServerPortLifecycle(server.httpServer, bootCapToken, () => {
        void closePreviewBffServer();
      });
      server.middlewares.use((req, res, next) => {
        if (tryServePreviewIndexHtml(req, res, server, bootCapToken)) return;
        dispatchMiddlewareRequest(req, res, next).catch(next);
      });
    },
  };
}
