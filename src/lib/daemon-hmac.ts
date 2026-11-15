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

export function assembleForwardHeaders(
  safeInbound: Record<string, string>,
  signedHeaders: Record<string, string>,
  extraHeaders?: Record<string, string>
): Record<string, string> {
  const headers: Record<string, string> = {};
  if (safeInbound["if-match"]) headers["If-Match"] = safeInbound["if-match"];
  if (safeInbound["last-event-id"]) headers["Last-Event-ID"] = safeInbound["last-event-id"];
  if (safeInbound.accept) headers.Accept = safeInbound.accept;
  if (extraHeaders) {
    for (const [k, v] of Object.entries(extraHeaders)) {
      if (v) headers[k] = v;
    }
  }
  return Object.assign(headers, signedHeaders);
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
