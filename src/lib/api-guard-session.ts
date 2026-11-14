// @story 04.1
// @implements AC-06
import crypto from "node:crypto";

export interface LoopbackSessionRecord {
  tenantId: string;
  userId: string;
  role: "viewer" | "editor" | "admin";
  expiresAt: number;
}

const globalSessionRegistryKey = Symbol.for("cowok.loopback.sessionRegistry");
const globalBoundPortKey = Symbol.for("cowok.loopback.boundBffPort");
const sessionRegistry: Map<string, LoopbackSessionRecord> =
  ((globalThis as unknown as Record<symbol, Map<string, LoopbackSessionRecord>>)[globalSessionRegistryKey] ??=
    new Map<string, LoopbackSessionRecord>());

const SESSION_TTL_HOURS = 8;
const MS_PER_HOUR = 3_600_000;
const SESSION_TTL_MS = SESSION_TTL_HOURS * MS_PER_HOUR;
const MAX_SESSION_REGISTRY_SIZE = 4_096;
const SESSION_TOKEN_BYTES = 24;
const MIN_PORT = 1;
const MAX_PORT = 65_535;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;

/** Builds a JSON response with an explicit HTTP status code. */
export function jsonResp(status: number, body: Record<string, unknown>): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Pins the active BFF listening port for Host/Origin validation, or clears it when null. */
export function setBoundBffPort(port: number | null): void {
  const g = globalThis as unknown as Record<symbol, number | null>;
  g[globalBoundPortKey] = typeof port === "number" && Number.isInteger(port) && port > 0 ? port : null;
}

/** Returns the pinned BFF listening port, or null when unbound (e.g. unit tests). */
export function getBoundBffPort(): number | null {
  const g = globalThis as unknown as Record<symbol, number | null | undefined>;
  const val = g[globalBoundPortKey];
  return typeof val === "number" && Number.isInteger(val) && val > 0 ? val : null;
}

/** Removes all expired loopback sessions from the in-memory registry and returns the pruned count. */
export function pruneExpiredBffSessions(now = Date.now()): number {
  let pruned = 0;
  for (const [key, rec] of sessionRegistry.entries()) {
    if (rec.expiresAt <= now) {
      sessionRegistry.delete(key);
      pruned += 1;
    }
  }
  return pruned;
}

/** Returns the current number of entries in the BFF session registry. */
export function getBffSessionRegistrySize(): number {
  return sessionRegistry.size;
}

/** Issues a strict-loopback session token with TTL eviction and bounded registry capacity. */
export function issueBffLoopbackSession(input?: {
  tenantId?: string;
  userId?: string;
  role?: "viewer" | "editor" | "admin";
  ttlMs?: number;
  maxCapacity?: number;
}): { sessionToken: string } & LoopbackSessionRecord {
  const now = Date.now();
  pruneExpiredBffSessions(now);
  const cap = Math.max(1, input?.maxCapacity ?? MAX_SESSION_REGISTRY_SIZE);
  while (sessionRegistry.size >= cap) {
    const oldestKey = sessionRegistry.keys().next().value;
    if (!oldestKey) break;
    sessionRegistry.delete(oldestKey);
  }
  const token = `cwk_bff_${crypto.randomBytes(SESSION_TOKEN_BYTES).toString("hex")}`;
  const ttlMs = typeof input?.ttlMs === "number" && Number.isFinite(input.ttlMs) ? input.ttlMs : SESSION_TTL_MS;
  const rec: LoopbackSessionRecord = {
    tenantId: input?.tenantId?.trim() || "local-default",
    userId: input?.userId?.trim() || "local-operator",
    role: input?.role || "editor",
    expiresAt: now + ttlMs,
  };
  sessionRegistry.set(token, rec);
  return { sessionToken: token, ...rec };
}

/** Validates that a Host/Origin host string is loopback and matches the expected port when pinned. */
export function isAllowedLoopbackHost(rawHost: string, expectedPort?: number | null): boolean {
  const host = rawHost.trim().toLowerCase();
  let parsedPort: number | null = null;
  if (host !== "127.0.0.1" && host !== "localhost") {
    const match = /^(127\.0\.0\.1|localhost):(\d{1,5})$/.exec(host);
    if (!match) return false;
    const port = Number(match[2]);
    if (!Number.isInteger(port) || port < MIN_PORT || port > MAX_PORT) return false;
    parsedPort = port;
  }
  if (typeof expectedPort === "number" && Number.isInteger(expectedPort) && expectedPort > 0) {
    return parsedPort === expectedPort;
  }
  return true;
}

function isOriginAllowed(origin: string, hostHeader: string, boundPort: number | null): boolean {
  try {
    const originHost = new URL(origin).host;
    return isAllowedLoopbackHost(originHost, boundPort) && originHost.toLowerCase() === hostHeader.trim().toLowerCase();
  } catch {
    return false;
  }
}

/** Verifies loopback Host, Sec-Fetch-Site, Origin, session token expiry, and viewer write restrictions. */
export function verifyStrictLoopback(
  req: Request,
  fallbackUrl: URL
): { ok: true; session: LoopbackSessionRecord } | { ok: false; response: Response } {
  const boundPort = getBoundBffPort();
  const hostHeader = req.headers.get("host") || fallbackUrl.host;
  const sfs = req.headers.get("sec-fetch-site");
  const origin = req.headers.get("origin");
  if (
    !isAllowedLoopbackHost(hostHeader, boundPort) ||
    (sfs && sfs !== "same-origin" && sfs !== "none") ||
    (origin && !isOriginAllowed(origin, hostHeader, boundPort))
  ) {
    return { ok: false, response: jsonResp(HTTP_FORBIDDEN, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" }) };
  }
  const token = req.headers.get("x-cowok-session-token")?.trim() || "";
  const session = sessionRegistry.get(token);
  if (!session || session.expiresAt <= Date.now()) {
    if (session) sessionRegistry.delete(token);
    return { ok: false, response: jsonResp(HTTP_UNAUTHORIZED, { code: "UNAUTHENTICATED", error: "UNAUTHENTICATED" }) };
  }
  if (session.role === "viewer" && req.method !== "GET" && req.method !== "HEAD") {
    return { ok: false, response: jsonResp(HTTP_FORBIDDEN, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE" }) };
  }
  return { ok: true, session };
}
