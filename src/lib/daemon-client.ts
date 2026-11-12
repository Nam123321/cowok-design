// @story 04.1
// @implements AC-03, AC-04, AC-06
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { logger } from "./logger";

export const DEFAULT_FLOW_PROJECT_ID = "11111111-1111-4111-8111-111111111111";

interface DaemonLockRecord {
  pid: number;
  host: string;
  port: number;
  daemon_nonce: string;
  hmac_key_hex: string;
  schema_version: number;
}

const STRIP_PREFIXES = ["x-cowok-", "x-tenant-", "x-user-", "x-forwarded-"];
let spawningPromise: Promise<DaemonLockRecord> | null = null;
const seededWorkspaces = new Set<string>();

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

export function stripSpoofedHeaders(inbound: Headers): Record<string, string> {
  const clean: Record<string, string> = {};
  inbound.forEach((value, key) => {
    const lower = key.toLowerCase();
    if (STRIP_PREFIXES.some((pfx) => lower.startsWith(pfx))) {
      return;
    }
    clean[key] = value;
  });
  return clean;
}

function readValidLock(workspaceRoot: string): DaemonLockRecord | null {
  const lockPath = path.join(workspaceRoot, ".cowok-design", "daemon.lock");
  if (!fs.existsSync(lockPath)) return null;
  try {
    const stat = fs.lstatSync(lockPath);
    if (stat.isSymbolicLink()) return null;
    const parsed = JSON.parse(fs.readFileSync(lockPath, "utf8")) as DaemonLockRecord;
    if (
      parsed.host !== "127.0.0.1" ||
      !isPidAlive(parsed.pid) ||
      !parsed.daemon_nonce ||
      !parsed.hmac_key_hex ||
      parsed.schema_version < 3
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

async function verifyHealthAndSeedDefaultProject(lock: DaemonLockRecord, workspaceRoot: string): Promise<void> {
  const healthRes = await fetch(`http://127.0.0.1:${lock.port}/api/v1/health`);
  if (!healthRes.ok) {
    throw new Error(`Daemon health check failed (${healthRes.status})`);
  }
  const health = (await healthRes.json()) as { service?: string; schema_version?: number; daemon_nonce?: string };
  if (health.service !== "cowok-design-harness-daemon" || (health.schema_version ?? 0) < 3) {
    throw new Error("Unexpected daemon health identity or schema_version");
  }
  if (!seededWorkspaces.has(workspaceRoot)) {
    seededWorkspaces.add(workspaceRoot);
    const defaultProjects = [
      {
        id: DEFAULT_FLOW_PROJECT_ID,
        slug: "cowok-design-default",
        name: "cowok-design",
        product_brief: "Default seeded Flow-First 2D DAG Studio project for local workbench execution.",
      },
      {
        id: "22222222-2222-4222-8222-222222222222",
        slug: "cowok-ai-workbench",
        name: "Cowok.ai",
        product_brief: "Seeded Cowok.ai multi-agent OS workbench project.",
      },
      {
        id: "33333333-3333-4333-8333-333333333333",
        slug: "markops-platform",
        name: "Markops_platform",
        product_brief: "Seeded Markops scratch workbench project.",
      },
    ];
    for (const proj of defaultProjects) {
      await fetch(`http://127.0.0.1:${lock.port}/api/v1/projects`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ...proj,
          workspace_root: null,
          inherit_brand_from_project_id: null,
        }),
      }).catch(() => undefined);
    }
  }
}

export async function ensureRunningDaemon(): Promise<DaemonLockRecord> {
  const wsRoot = resolveWorkspaceRoot();
  const existing = readValidLock(wsRoot);
  if (existing) {
    await verifyHealthAndSeedDefaultProject(existing, wsRoot);
    return existing;
  }
  if (spawningPromise) return spawningPromise;

  spawningPromise = (async () => {
    try {
      const bin = resolveDaemonBinaryPath();
      const child = spawn(bin, [], {
        cwd: wsRoot,
        env: { ...process.env, UXUI_OS_WORKSPACE_ROOT: wsRoot, UXUI_OS_DAEMON_PORT: "0" },
        detached: true,
        stdio: "ignore",
      });
      child.unref();

      const deadline = Date.now() + 8_000;
      while (Date.now() < deadline) {
        const lock = readValidLock(wsRoot);
        if (lock) {
          await verifyHealthAndSeedDefaultProject(lock, wsRoot);
          return lock;
        }
        await new Promise((r) => setTimeout(r, 40));
      }
      throw new Error("Timed out waiting for harness-daemon lockfile startup");
    } finally {
      spawningPromise = null;
    }
  })();

  return spawningPromise;
}

export function buildCanonicalHmacHeaders(params: {
  daemonNonce: string;
  hmacKeyHex: string;
  method: string;
  canonicalPath: string;
  rawQuery: string;
  principal: { tenantId: string; userId: string; role: string };
  rawBody: string;
  correlationId: string;
}): Record<string, string> {
  const timestamp = Math.floor(Date.now() / 1000).toString();
  const requestNonce = crypto.randomUUID();
  const principalB64 = Buffer.from(JSON.stringify(params.principal), "utf8").toString("base64");
  const bodySha = crypto.createHash("sha256").update(params.rawBody, "utf8").digest("hex");
  const canonical = [
    params.daemonNonce,
    timestamp,
    requestNonce,
    params.method.toUpperCase(),
    params.canonicalPath,
    params.rawQuery,
    principalB64,
    bodySha,
  ].join("\n");

  const sig = crypto
    .createHmac("sha256", Buffer.from(params.hmacKeyHex, "hex"))
    .update(canonical, "utf8")
    .digest("hex");

  return {
    "Content-Type": "application/json",
    Host: "127.0.0.1",
    "X-Correlation-Id": params.correlationId,
    "X-Cowok-Timestamp": timestamp,
    "X-Cowok-Nonce": requestNonce,
    "X-Cowok-Principal": principalB64,
    "X-Cowok-Hmac-Sig": sig,
  };
}

export async function forwardToDaemonFlow(opts: {
  method: string;
  daemonPath: string;
  rawQuery?: string;
  body?: unknown;
  inboundHeaders?: Headers;
  principal: { tenantId: string; userId: string; role: "viewer" | "editor" | "admin" };
  correlationId?: string;
}): Promise<Response> {
  if (opts.inboundHeaders) {
    stripSpoofedHeaders(opts.inboundHeaders);
  }
  const lock = await ensureRunningDaemon();
  const rawQuery = (opts.rawQuery || "").replace(/^\?/, "");
  const rawBody = opts.body !== undefined ? JSON.stringify(opts.body) : "";
  const correlationId = opts.correlationId || crypto.randomUUID();

  const headers = buildCanonicalHmacHeaders({
    daemonNonce: lock.daemon_nonce,
    hmacKeyHex: lock.hmac_key_hex,
    method: opts.method,
    canonicalPath: opts.daemonPath,
    rawQuery,
    principal: opts.principal,
    rawBody,
    correlationId,
  });

  const qs = rawQuery ? `?${rawQuery}` : "";
  const targetUrl = `http://127.0.0.1:${lock.port}${opts.daemonPath}${qs}`;
  logger.info("bff_forward_daemon", { method: opts.method, path: opts.daemonPath, correlationId });

  const upstream = await fetch(targetUrl, {
    method: opts.method,
    headers,
    body: opts.method === "GET" || opts.method === "HEAD" || rawBody === "" ? undefined : rawBody,
  });

  const text = await upstream.text();
  return new Response(text, {
    status: upstream.status,
    headers: {
      "Content-Type": "application/json",
      "X-Correlation-Id": correlationId,
    },
  });
}
