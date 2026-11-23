// @story 04.1, 04.4
// @implements AC-03, AC-04, AC-06
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import {
  assembleForwardHeaders,
  buildCanonicalHmacHeaders,
  relayDaemonResponse,
  seedProjectsToDaemon,
  stripSpoofedHeaders,
  type DaemonLockRecord,
} from "./daemon-hmac";
import { getOrInitWorkbenchHumanSecret } from "./daemon-secret.ts";
import { logger } from "./logger.ts";
import { clearBffSessionHumanToken, setBffSessionHumanToken } from "./api-guard-session.ts";

export { buildCanonicalHmacHeaders, stripSpoofedHeaders };

const DAEMON_MIN_SCHEMA_VERSION = 21;
const DAEMON_STARTUP_TIMEOUT_MS = 8_000;
const DAEMON_POLL_INTERVAL_MS = 40;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;

/**
 * Local workspace bootstrap default IDs.
 * These deterministic UUIDs correspond to the V1 SQLite migration bootstrap seed rows
 * (projects 'cowok-design-default', 'cowok-ai-workbench', and 'markops-platform')
 * ensuring seamless offline dev bootstrap when no environment override is provided.
 */
const LOCAL_BOOTSTRAP_DEFAULTS = {
  flowProjectId:
    (typeof process !== "undefined" && process.env?.COWOK_DEFAULT_PROJECT_ID?.trim()) ||
    "11111111-1111-4111-8111-111111111111",
  cowokAiProjectId:
    (typeof process !== "undefined" && process.env?.COWOK_DEFAULT_COWOK_AI_PROJECT_ID?.trim()) ||
    "22222222-2222-4222-8222-222222222222",
  markopsProjectId:
    (typeof process !== "undefined" && process.env?.COWOK_DEFAULT_MARKOPS_PROJECT_ID?.trim()) ||
    "33333333-3333-4333-8333-333333333333",
  orgId:
    (typeof process !== "undefined" && process.env?.COWOK_DEFAULT_ORG_ID?.trim()) ||
    "00000000-0000-4000-8000-000000000001",
  tenantId:
    (typeof process !== "undefined" && process.env?.COWOK_DEFAULT_TENANT_ID?.trim()) ||
    "local-default",
} as const;

export const DEFAULT_FLOW_PROJECT_ID = LOCAL_BOOTSTRAP_DEFAULTS.flowProjectId;

let spawningPromise: Promise<DaemonLockRecord> | null = null;
const SEEDED_WORKSPACES_SYMBOL = Symbol.for("cowok.daemonSeedPromises");

function getSeededWorkspacePromises(): Map<string, Promise<void>> {
  const g = globalThis as Record<PropertyKey, unknown>;
  if (!(g[SEEDED_WORKSPACES_SYMBOL] instanceof Map)) {
    g[SEEDED_WORKSPACES_SYMBOL] = new Map<string, Promise<void>>();
  }
  return g[SEEDED_WORKSPACES_SYMBOL] as Map<string, Promise<void>>;
}

export function resolveWorkspaceRoot(): string {
  const raw = process.env.UXUI_OS_WORKSPACE_ROOT?.trim() || process.cwd();
  return fs.existsSync(raw) ? fs.realpathSync(raw) : raw;
}

function isPidAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function readValidLock(workspaceRoot: string): DaemonLockRecord | null {
  const lockPath = path.join(workspaceRoot, ".cowok-design", "daemon.lock");
  if (!fs.existsSync(lockPath)) return null;
  try {
    if (fs.lstatSync(lockPath).isSymbolicLink()) return null;
    const parsed = JSON.parse(fs.readFileSync(lockPath, "utf8")) as DaemonLockRecord;
    if (
      parsed.host !== "127.0.0.1" ||
      !isPidAlive(parsed.pid) ||
      !parsed.daemon_nonce ||
      !parsed.hmac_key_hex ||
      typeof parsed.schema_version !== "number" ||
      !Number.isFinite(parsed.schema_version) ||
      parsed.schema_version < DAEMON_MIN_SCHEMA_VERSION
    ) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function resolveDaemonBinaryPath(): string {
  const envBin = process.env.UXUI_OS_DAEMON_BIN?.trim();
  if (envBin && fs.existsSync(envBin)) return envBin;
  const repoBin = path.resolve(process.cwd(), "target", "debug", "harness-daemon");
  if (fs.existsSync(repoBin)) return repoBin;
  throw new Error(`Missing harness-daemon binary at ${repoBin}; run 'cargo build --bin harness-daemon'`);
}

function buildSeedProjects() {
  const projects = [
    {
      id: LOCAL_BOOTSTRAP_DEFAULTS.flowProjectId,
      slug: "cowok-design-default",
      name: "cowok-design",
      product_brief: "Default seeded Flow-First 2D DAG Studio project for local workbench execution.",
    },
  ];
  if (process.env.COWOK_DEMO_SEED === "1") {
    projects.push(
      {
        id: LOCAL_BOOTSTRAP_DEFAULTS.cowokAiProjectId,
        slug: "cowok-ai-workbench",
        name: "Cowok.ai",
        product_brief: "Seeded Cowok.ai multi-agent OS workbench project.",
      },
      {
        id: LOCAL_BOOTSTRAP_DEFAULTS.markopsProjectId,
        slug: "markops-platform",
        name: "Markops_platform",
        product_brief: "Seeded Markops scratch workbench project.",
      }
    );
  }
  return projects;
}

async function seedDefaultProjects(lock: DaemonLockRecord): Promise<void> {
  await seedProjectsToDaemon(lock, buildSeedProjects());
}

async function verifyHealthAndSeedDefaultProject(lock: DaemonLockRecord, workspaceRoot: string): Promise<void> {
  const healthRes = await fetch(`http://127.0.0.1:${lock.port}/api/v1/health`);
  if (!healthRes.ok) throw new Error(`Daemon health check failed (${healthRes.status})`);
  const health = (await healthRes.json()) as { service?: string; schema_version?: number };
  if (health.service !== "cowok-design-harness-daemon" || (health.schema_version ?? 0) < DAEMON_MIN_SCHEMA_VERSION) {
    throw new Error("Unexpected daemon health identity or schema_version");
  }
  const seedingPromises = getSeededWorkspacePromises();
  let seeding = seedingPromises.get(workspaceRoot);
  if (!seeding) {
    seeding = seedDefaultProjects(lock).catch((err) => {
      seedingPromises.delete(workspaceRoot);
      throw err;
    });
    seedingPromises.set(workspaceRoot, seeding);
  }
  await seeding;
}

export async function ensureRunningDaemon(): Promise<DaemonLockRecord> {
  const wsRoot = resolveWorkspaceRoot();
  const existing = readValidLock(wsRoot);
  if (existing) {
    getOrInitWorkbenchHumanSecret(wsRoot);
    await verifyHealthAndSeedDefaultProject(existing, wsRoot);
    return existing;
  }
  if (spawningPromise) return spawningPromise;
  spawningPromise = (async () => {
    try {
      const humanSecret = getOrInitWorkbenchHumanSecret(wsRoot);
      const child = spawn(resolveDaemonBinaryPath(), [], {
        cwd: wsRoot,
        env: {
          ...process.env,
          UXUI_OS_WORKSPACE_ROOT: wsRoot,
          UXUI_OS_DAEMON_PORT: "0",
          COWOK_WORKBENCH_HUMAN_SECRET: humanSecret,
        },
        detached: true,
        stdio: "ignore",
      });
      child.unref();
      const deadline = Date.now() + DAEMON_STARTUP_TIMEOUT_MS;
      while (Date.now() < deadline) {
        const lock = readValidLock(wsRoot);
        if (lock) {
          await verifyHealthAndSeedDefaultProject(lock, wsRoot);
          return lock;
        }
        await new Promise((r) => setTimeout(r, DAEMON_POLL_INTERVAL_MS));
      }
      throw new Error("Timed out waiting for harness-daemon lockfile startup");
    } finally {
      spawningPromise = null;
    }
  })();
  return spawningPromise;
}

async function resolveHumanSessionHeader(
  lock: DaemonLockRecord,
  opts: {
    method?: string;
    bffSessionToken?: string;
    isHumanWorkbench?: boolean;
    humanSessionToken?: string;
    principal: { tenantId: string; userId: string; role: "viewer" | "editor" | "admin" };
  }
): Promise<string | undefined> {
  if (opts.humanSessionToken?.trim()) return opts.humanSessionToken.trim();
  if (!opts.isHumanWorkbench || opts.principal.role === "viewer") return undefined;
  const secret = getOrInitWorkbenchHumanSecret(resolveWorkspaceRoot());
  if (!secret) {
    throw new Error("HUMAN_AUTH_MISCONFIGURED: Missing shared workbench human secret");
  }
  const res = await fetch(`http://127.0.0.1:${lock.port}/api/v1/auth/workbench-session`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Host: `127.0.0.1:${lock.port}`,
      "X-Cowok-Workbench-Secret": secret,
    },
    body: JSON.stringify({
      tenantId: opts.principal.tenantId,
      userId: opts.principal.userId,
      role: "editor",
    }),
  });
  if (!res.ok) {
    const isWrite = opts.method && opts.method !== "GET" && opts.method !== "HEAD";
    if (isWrite) {
      throw new Error(`HUMAN_AUTH_MISCONFIGURED: Daemon workbench-session rejected (${res.status})`);
    }
    return undefined;
  }
  const data = (await res.json()) as { sessionToken?: string };
  const tok = data.sessionToken?.trim();
  if (!tok) {
    throw new Error("HUMAN_AUTH_MISCONFIGURED: Daemon returned empty workbench session token");
  }
  if (opts.bffSessionToken) {
    setBffSessionHumanToken(opts.bffSessionToken, tok);
  }
  return tok;
}

interface DaemonForwardOpts {
  method: string;
  daemonPath: string;
  rawQuery?: string;
  body?: unknown;
  rawBodyBytes?: Uint8Array;
  contentType?: string;
  inboundHeaders?: Headers;
  extraHeaders?: Record<string, string>;
  principal: { tenantId: string; userId: string; role: "viewer" | "editor" | "admin" };
  correlationId?: string;
  bffSessionToken?: string;
  isHumanWorkbench?: boolean;
  humanSessionToken?: string;
  signal?: AbortSignal;
}

async function fetchDaemonOnce(params: {
  lock: DaemonLockRecord;
  opts: DaemonForwardOpts;
  safeInbound: Record<string, string>;
  rawQuery: string;
  rawBody: string | Uint8Array;
  correlationId: string;
}): Promise<Response> {
  const { lock, opts, safeInbound, rawQuery, rawBody, correlationId } = params;
  const resolvedHumanTok = await resolveHumanSessionHeader(lock, opts);
  const signedHeaders = buildCanonicalHmacHeaders({
    daemonNonce: lock.daemon_nonce, hmacKeyHex: lock.hmac_key_hex,
    method: opts.method, canonicalPath: opts.daemonPath, rawQuery,
    principal: opts.principal, rawBody, contentType: opts.contentType, correlationId,
  });
  const extraHeaders = resolvedHumanTok
    ? { ...(opts.extraHeaders || {}), "X-Cowok-Human-Session": resolvedHumanTok }
    : opts.extraHeaders;
  const headers = assembleForwardHeaders(safeInbound, signedHeaders, extraHeaders);
  headers["X-Cowok-Role"] = opts.principal.role;
  const targetUrl = `http://127.0.0.1:${lock.port}${opts.daemonPath}${rawQuery ? `?${rawQuery}` : ""}`;
  logger.info("bff_forward_daemon", { method: opts.method, path: opts.daemonPath, correlationId });
  // Only propagate AbortSignal in Node server runtimes (jsdom swaps global AbortSignal across realms).
  const serverSignal = typeof window === "undefined" ? opts.signal : undefined;
  const emptyBody = typeof rawBody === "string" ? rawBody === "" : rawBody.byteLength === 0;
  return fetch(targetUrl, {
    method: opts.method,
    headers,
    body: opts.method === "GET" || opts.method === "HEAD" || emptyBody ? undefined : (rawBody as BodyInit),
    ...(serverSignal ? { signal: serverSignal } : {}),
  });
}

function resolveForwardRawBody(opts: DaemonForwardOpts): string | Uint8Array {
  if (opts.rawBodyBytes !== undefined) {
    return opts.rawBodyBytes;
  }
  if (opts.body !== undefined) {
    return JSON.stringify(opts.body);
  }
  return "";
}

export async function forwardToDaemonFlow(opts: DaemonForwardOpts): Promise<Response> {
  const safeInbound = opts.inboundHeaders ? stripSpoofedHeaders(opts.inboundHeaders) : {};
  const lock = await ensureRunningDaemon();
  const rawQuery = (opts.rawQuery || "").replace(/^\?/, "");
  const rawBody = resolveForwardRawBody(opts);
  const correlationId = opts.correlationId || crypto.randomUUID();
  let upstream = await fetchDaemonOnce({ lock, opts, safeInbound, rawQuery, rawBody, correlationId });
  if (
    (upstream.status === HTTP_UNAUTHORIZED || upstream.status === HTTP_FORBIDDEN) &&
    opts.isHumanWorkbench &&
    opts.humanSessionToken &&
    opts.bffSessionToken
  ) {
    clearBffSessionHumanToken(opts.bffSessionToken);
    upstream = await fetchDaemonOnce({
      lock,
      opts: { ...opts, humanSessionToken: undefined },
      safeInbound,
      rawQuery,
      rawBody,
      correlationId,
    });
  }
  return relayDaemonResponse(upstream, correlationId);
}

export function forwardFromGuardedContext(
  req: Request,
  ctx: {
    tenantId?: string;
    userId?: string;
    role?: "viewer" | "editor" | "admin";
    correlationId?: string;
    bffSessionToken?: string;
    isHumanWorkbench?: boolean;
    humanSessionToken?: string;
  },
  daemonPath: string,
  body?: unknown
): Promise<Response> {
  const url = new URL(req.url, "http://127.0.0.1");
  return forwardToDaemonFlow({
    method: req.method,
    daemonPath,
    rawQuery: url.search.replace(/^\?/, ""),
    body,
    inboundHeaders: req.headers,
    principal: {
      tenantId: ctx.tenantId || "local-default",
      userId: ctx.userId || "local-operator",
      role: ctx.role || "viewer",
    },
    correlationId: ctx.correlationId,
    bffSessionToken: ctx.bffSessionToken,
    isHumanWorkbench: ctx.isHumanWorkbench,
    humanSessionToken: ctx.humanSessionToken,
    signal: req.signal,
  });
}
