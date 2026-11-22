// @story 04.4
// @implements AC-03, AC-04 (HMAC Canonical Signing & Daemon Response Relay helpers)
import crypto from "node:crypto";

export interface DaemonLockRecord {
  pid: number;
  host: string;
  port: number;
  daemon_nonce: string;
  hmac_key_hex: string;
  schema_version: number;
}

const STRIP_PREFIXES = ["x-cowok-", "x-tenant-", "x-user-", "x-forwarded-"];

export function stripSpoofedHeaders(inbound: Headers): Record<string, string> {
  const clean: Record<string, string> = {};
  inbound.forEach((value, key) => {
    const lower = key.toLowerCase();
    if (STRIP_PREFIXES.some((pfx) => lower.startsWith(pfx))) return;
    clean[key] = value;
  });
  return clean;
}

export function buildCanonicalHmacHeaders(params: {
  daemonNonce: string;
  hmacKeyHex: string;
  method: string;
  canonicalPath: string;
  rawQuery: string;
  principal: { tenantId: string; userId: string; role: string };
  rawBody: string | Uint8Array;
  contentType?: string;
  correlationId: string;
}): Record<string, string> {
  const timestamp = Math.floor(Date.now() / 1000).toString();
  const requestNonce = crypto.randomUUID();
  const principalB64 = Buffer.from(JSON.stringify(params.principal), "utf8").toString("base64");
  const bodyHasher = crypto.createHash("sha256");
  if (typeof params.rawBody === "string") {
    bodyHasher.update(params.rawBody, "utf8");
  } else {
    bodyHasher.update(params.rawBody);
  }
  const bodySha = bodyHasher.digest("hex");
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
    "Content-Type": params.contentType || "application/json",
    "X-Correlation-Id": params.correlationId,
    "X-Cowok-Timestamp": timestamp,
    "X-Cowok-Nonce": requestNonce,
    "X-Cowok-Principal": principalB64,
    "X-Cowok-Hmac-Sig": sig,
  };
}

export function assembleForwardHeaders(
  safeInbound: Record<string, string>,
  signedHeaders: Record<string, string>,
  extraHeaders?: Record<string, string>
): Record<string, string> {
  const headers: Record<string, string> = {};
  if (safeInbound["if-match"]) headers["If-Match"] = safeInbound["if-match"];
  if (safeInbound["last-event-id"]) headers["Last-Event-ID"] = safeInbound["last-event-id"];
  if (safeInbound["idempotency-key"]) headers["Idempotency-Key"] = safeInbound["idempotency-key"];
  if (safeInbound["x-idempotency-key"]) headers["X-Idempotency-Key"] = safeInbound["x-idempotency-key"];
  if (safeInbound["x-session-id"]) headers["X-Session-Id"] = safeInbound["x-session-id"];
  if (safeInbound["x-project-id"]) headers["X-Project-Id"] = safeInbound["x-project-id"];
  if (safeInbound["x-filename"]) headers["X-Filename"] = safeInbound["x-filename"];
  if (safeInbound.accept) headers.Accept = safeInbound.accept;
  if (extraHeaders) {
    for (const [k, v] of Object.entries(extraHeaders)) {
      if (v) headers[k] = v;
    }
  }
  return Object.assign(headers, signedHeaders);
}

const HTTP_CONFLICT = 409;

export async function seedProjectsToDaemon(
  lock: DaemonLockRecord,
  projects: Array<{ id: string; slug: string; name: string; product_brief: string }>
): Promise<void> {
  for (const proj of projects) {
    const rawBody = JSON.stringify({
      ...proj,
      workspace_root: null,
      inherit_brand_from_project_id: null,
    });
    const headers = buildCanonicalHmacHeaders({
      daemonNonce: lock.daemon_nonce,
      hmacKeyHex: lock.hmac_key_hex,
      method: "POST",
      canonicalPath: "/api/v1/projects",
      rawQuery: "",
      principal: { tenantId: "local-default", userId: "bff-bootstrap", role: "editor" },
      rawBody,
      correlationId: crypto.randomUUID(),
    });
    const res = await fetch(`http://127.0.0.1:${lock.port}/api/v1/projects`, {
      method: "POST",
      headers,
      body: rawBody,
    });
    if (!res.ok && res.status !== HTTP_CONFLICT) {
      throw new Error(`Seed project '${proj.slug}' failed (${res.status})`);
    }
  }
}

export async function relayDaemonResponse(
  upstream: Response,
  correlationId: string
): Promise<Response> {
  const contentType = upstream.headers.get("content-type") || "application/json";
  const responseHeaders: Record<string, string> = {
    "Content-Type": contentType,
    "X-Correlation-Id": correlationId,
  };
  const disposition = upstream.headers.get("content-disposition");
  if (disposition) responseHeaders["Content-Disposition"] = disposition;
  const bundleSha = upstream.headers.get("x-cowok-bundle-sha256");
  if (bundleSha) responseHeaders["X-Cowok-Bundle-Sha256"] = bundleSha;
  const retryAfter = upstream.headers.get("retry-after");
  if (retryAfter) responseHeaders["Retry-After"] = retryAfter;

  if (contentType.includes("text/event-stream") && upstream.body) {
    responseHeaders["Cache-Control"] = "no-cache";
    responseHeaders.Connection = "keep-alive";
    return new Response(upstream.body, { status: upstream.status, headers: responseHeaders });
  }
  if (contentType.includes("application/zip") || contentType.includes("application/octet-stream")) {
    return new Response(await upstream.arrayBuffer(), { status: upstream.status, headers: responseHeaders });
  }
  return new Response(await upstream.text(), { status: upstream.status, headers: responseHeaders });
}
