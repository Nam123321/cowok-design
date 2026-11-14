// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-06
import crypto from "node:crypto";
import fs from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { getBoundBffPort, isAllowedLoopbackHost, issueBffLoopbackSession } from "./api-guard";
import { resolveWorkspaceRoot } from "./daemon-client";
import { LocalSessionBootstrapInputSchema } from "./validations/harness-daemon";

export const MAX_BODY_BYTES = 1_048_576;
export const HTTP_OK = 200;
export const HTTP_BAD_REQUEST = 400;
export const HTTP_FORBIDDEN = 403;
export const HTTP_PAYLOAD_TOO_LARGE = 413;
const BOOT_CAP_TOKEN_BYTES = 24;

const globalBootCapKey = Symbol.for("cowok.loopback.bootCapTokens");

function getBootCapTokens(): Set<string> {
  const g = globalThis as unknown as Record<symbol, Set<string> | undefined>;
  if (!(g[globalBootCapKey] instanceof Set)) {
    g[globalBootCapKey] = new Set<string>();
  }
  return g[globalBootCapKey]!;
}

/** Mints a per-server boot capability token injected into HTML so only served browser pages can mint editor sessions. */
export function issueBootCapToken(): string {
  const tok = crypto.randomBytes(BOOT_CAP_TOKEN_BYTES).toString("hex");
  getBootCapTokens().add(tok);
  return tok;
}

/** Revokes an active boot capability token on server shutdown. */
export function revokeBootCapToken(token: string): void {
  getBootCapTokens().delete(token);
}

/** Injects the boot capability meta tag into served HTML documents. */
export function injectBootCapMeta(html: string, bootCapToken: string): string {
  const metaTag = `<meta name="cowok-boot-cap" content="${bootCapToken}">`;
  if (/<head[^>]*>/i.test(html)) {
    return html.replace(/<head[^>]*>/i, (headOpen) => `${headOpen}\n    ${metaTag}`);
  }
  return `${metaTag}\n${html}`;
}

/** Reads an incoming request body up to MAX_BODY_BYTES. */
export function readRawBody(req: IncomingMessage): Promise<Buffer | "TOO_LARGE"> {
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

/** Writes a JSON payload to a Node ServerResponse. */
export function writeJson(res: ServerResponse, statusCode: number, payload: unknown): void {
  res.statusCode = statusCode;
  res.setHeader("Content-Type", "application/json");
  res.end(JSON.stringify(payload));
}

function timingSafeMatch(supplied: string, expected: string): boolean {
  if (!supplied || !expected || supplied.length !== expected.length) return false;
  return crypto.timingSafeEqual(Buffer.from(supplied, "utf8"), Buffer.from(expected, "utf8"));
}

function singleHeader(val: string | string[] | undefined): string {
  if (Array.isArray(val)) return (val[0] || "").trim();
  return typeof val === "string" ? val.trim() : "";
}

function isValidBootCapHeader(val: string | string[] | undefined): boolean {
  const supplied = singleHeader(val);
  if (!supplied) return false;
  for (const active of getBootCapTokens()) {
    if (timingSafeMatch(supplied, active)) return true;
  }
  return false;
}

function isValidDaemonNonceHeader(val: string | string[] | undefined): boolean {
  const supplied = singleHeader(val);
  if (!supplied) return false;
  try {
    const lockPath = path.join(resolveWorkspaceRoot(), ".cowok-design", "daemon.lock");
    if (!fs.existsSync(lockPath) || fs.lstatSync(lockPath).isSymbolicLink()) return false;
    const parsed = JSON.parse(fs.readFileSync(lockPath, "utf8")) as { daemon_nonce?: unknown };
    const expected = typeof parsed.daemon_nonce === "string" ? parsed.daemon_nonce.trim() : "";
    return timingSafeMatch(supplied, expected);
  } catch {
    return false;
  }
}

function isAuthorizedAdminBearer(authHeader: string | undefined): boolean {
  const expected = process.env.COWOK_ADMIN_TOKEN?.trim() || "";
  if (!expected || !authHeader || !authHeader.startsWith("Bearer ")) return false;
  return timingSafeMatch(authHeader.slice("Bearer ".length).trim(), expected);
}

function hasSameOriginPortProof(req: IncomingMessage, sfs: string): boolean {
  const host = singleHeader(req.headers.host);
  const origin = singleHeader(req.headers.origin);
  if (sfs !== "same-origin" || !origin || !host) return false;
  try {
    const originHost = new URL(origin).host;
    return isAllowedLoopbackHost(host, getBoundBffPort()) && originHost.toLowerCase() === host.toLowerCase();
  } catch {
    return false;
  }
}

function hasEditorBootstrapProof(req: IncomingMessage): boolean {
  if (isAuthorizedAdminBearer(req.headers.authorization)) return true;
  if (isValidDaemonNonceHeader(req.headers["x-cowok-daemon-nonce"])) return true;
  const sfs = singleHeader(req.headers["sec-fetch-site"]);
  if (isValidBootCapHeader(req.headers["x-cowok-boot-cap"])) {
    return !sfs || sfs === "same-origin";
  }
  if (getBootCapTokens().size > 0) return false;
  return hasSameOriginPortProof(req, sfs);
}

function verifyBootstrapOrigin(req: IncomingMessage): boolean {
  const host = req.headers.host;
  if (typeof host !== "string" || !host.trim()) return false;
  const boundPort = getBoundBffPort();
  const sfs = singleHeader(req.headers["sec-fetch-site"]);
  if (!isAllowedLoopbackHost(host, boundPort) || (sfs && sfs !== "same-origin" && sfs !== "none")) return false;
  const origin = req.headers.origin;
  if (!origin) return true;
  try {
    const originHost = new URL(String(origin)).host;
    return isAllowedLoopbackHost(originHost, boundPort) && originHost.toLowerCase() === host.trim().toLowerCase();
  } catch {
    return false;
  }
}

async function parseBootstrapInput(
  req: IncomingMessage,
  res: ServerResponse
): Promise<{ tenantId?: string; userId?: string; role: "viewer" | "editor" | "admin" } | null> {
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    writeJson(res, HTTP_PAYLOAD_TOO_LARGE, { code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" });
    return null;
  }
  let rawJson: unknown = {};
  if (rawBuf.length > 0) {
    try {
      rawJson = JSON.parse(rawBuf.toString("utf8"));
    } catch {
      writeJson(res, HTTP_BAD_REQUEST, { code: "INVALID_JSON", error: "INVALID_JSON", message: "Malformed JSON request body" });
      return null;
    }
  }
  const parsed = LocalSessionBootstrapInputSchema.safeParse(rawJson);
  if (!parsed.success) {
    writeJson(res, HTTP_BAD_REQUEST, { code: "VALIDATION_ERROR", error: "VALIDATION_ERROR", details: parsed.error.issues });
    return null;
  }
  return parsed.data;
}

/** Handles POST /api/auth/local-session with capability proof, tenant isolation, and role checks. */
export async function handleBootstrapSession(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (!verifyBootstrapOrigin(req)) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" });
  }
  const data = await parseBootstrapInput(req, res);
  if (!data) return;

  const isAdmin = isAuthorizedAdminBearer(req.headers.authorization);
  if (data.role === "admin" && !isAdmin) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE", message: "Unauthenticated local-session bootstrap cannot issue admin role" });
  }
  const requestedTenant = data.tenantId?.trim() || "local-default";
  if (requestedTenant !== "local-default" && !isAdmin) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "TENANT_ISOLATION_BREACH", error: "TENANT_ISOLATION_BREACH", message: "Unauthenticated local-session bootstrap cannot switch away from local-default tenant" });
  }
  if (data.role === "editor" && !hasEditorBootstrapProof(req)) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE", message: "Editor local-session bootstrap requires daemon nonce, boot capability token, or strict same-origin proof" });
  }
  return writeJson(res, HTTP_OK, issueBffLoopbackSession({ tenantId: requestedTenant, userId: data.userId, role: data.role }));
}
