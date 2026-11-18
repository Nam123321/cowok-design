// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-06
import crypto from "node:crypto";
import fs from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { getBoundBffPort, isAllowedLoopbackHost } from "./api-guard.ts";
import { resolveWorkspaceRoot } from "./daemon-client.ts";

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

export function singleHeader(val: string | string[] | undefined): string {
  if (Array.isArray(val)) return (val[0] || "").trim();
  return typeof val === "string" ? val.trim() : "";
}

export function timingSafeMatch(supplied: string, expected: string): boolean {
  if (!supplied || !expected || supplied.length !== expected.length) return false;
  return crypto.timingSafeEqual(Buffer.from(supplied, "utf8"), Buffer.from(expected, "utf8"));
}

function extractCookieValue(cookieHeader: string | string[] | undefined, cookieName: string): string {
  const raw = Array.isArray(cookieHeader) ? cookieHeader.join("; ") : cookieHeader || "";
  for (const part of raw.split(";")) {
    const idx = part.indexOf("=");
    if (idx <= 0) continue;
    if (part.slice(0, idx).trim() === cookieName) return part.slice(idx + 1).trim();
  }
  return "";
}

function deriveWorkbenchCookieProof(bootCapToken: string): string {
  return crypto.createHmac("sha256", bootCapToken).update("cowok-wb-proof-v1").digest("hex");
}

/** Formats the strict HttpOnly; SameSite=Strict cookie issued on document navigation responses. */
export function buildWorkbenchProofCookieHeader(bootCapToken: string): string {
  return `cowok_wb_proof=${deriveWorkbenchCookieProof(bootCapToken)}; HttpOnly; SameSite=Strict; Path=/api`;
}

export function isValidBootCapHeader(val: string | string[] | undefined): boolean {
  const supplied = singleHeader(val);
  if (!supplied) return false;
  for (const active of getBootCapTokens()) {
    if (timingSafeMatch(supplied, active)) return true;
  }
  return false;
}

export function isValidWorkbenchCookie(req: IncomingMessage, suppliedBootCap?: string): boolean {
  const proof = extractCookieValue(req.headers.cookie, "cowok_wb_proof");
  if (!proof) return false;
  if (suppliedBootCap) {
    return isValidBootCapHeader(suppliedBootCap) && timingSafeMatch(proof, deriveWorkbenchCookieProof(suppliedBootCap));
  }
  for (const active of getBootCapTokens()) {
    if (timingSafeMatch(proof, deriveWorkbenchCookieProof(active))) return true;
  }
  return false;
}

export function isValidDaemonNonceHeader(val: string | string[] | undefined): boolean {
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

export function isAuthorizedAdminBearer(authHeader: string | undefined): boolean {
  const expected = process.env.COWOK_ADMIN_TOKEN?.trim() || "";
  if (!expected || !authHeader || !authHeader.startsWith("Bearer ")) return false;
  return timingSafeMatch(authHeader.slice("Bearer ".length).trim(), expected);
}

export function hasSameOriginPortProof(req: IncomingMessage, sfs: string): boolean {
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

export function hasEditorBootstrapProof(req: IncomingMessage): boolean {
  if (isAuthorizedAdminBearer(req.headers.authorization)) return true;
  if (isValidDaemonNonceHeader(req.headers["x-cowok-daemon-nonce"])) return true;
  const sfs = singleHeader(req.headers["sec-fetch-site"]);
  if (req.headers["x-cowok-boot-cap"] !== undefined) {
    return isValidBootCapHeader(req.headers["x-cowok-boot-cap"]) && (!sfs || sfs === "same-origin");
  }
  if (getBootCapTokens().size > 0) return false;
  return hasSameOriginPortProof(req, sfs);
}

export function verifyBootstrapOrigin(req: IncomingMessage): boolean {
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
