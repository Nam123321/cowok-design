// @story 04.6
// @implements AC-02, AC-05 (Guarded BFF Binary/Multipart Stream Route for CAS Upload)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { CasUploadMetadataSchema } from "@/lib/validations/chat-vault";

const MAX_CAS_REQUEST_BYTES = 26_214_400;
const MAX_MULTIPART_OVERHEAD_BYTES = 262_144;
const MAX_BFF_CAS_INGRESS_BYTES = MAX_CAS_REQUEST_BYTES + MAX_MULTIPART_OVERHEAD_BYTES;
const HTTP_PAYLOAD_TOO_LARGE = 413;

function quotaExceededResponse() {
  return new Response(
    JSON.stringify({
      code: "CAS_QUOTA_EXCEEDED",
      message: "Request upload size limit exceeded",
      retryable: false,
      limit: "REQUEST_BYTES",
    }),
    { status: HTTP_PAYLOAD_TOO_LARGE, headers: { "Content-Type": "application/json" } }
  );
}

async function readBoundedIngressBytes(req: Request, maxBytes: number): Promise<Uint8Array | null> {
  const lenHeader = Number(req.headers.get("content-length") || "0");
  if (Number.isFinite(lenHeader) && lenHeader > maxBytes) return null;
  if (!req.body || typeof req.body.getReader !== "function") {
    const fallback = new Uint8Array(await req.arrayBuffer());
    return fallback.byteLength <= maxBytes ? fallback : null;
  }
  const reader = req.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (!value) continue;
    total += value.byteLength;
    if (total > maxBytes) {
      await reader.cancel().catch(() => {});
      return null;
    }
    chunks.push(value);
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    merged.set(c, offset);
    offset += c.byteLength;
  }
  return merged;
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const rawBytes = await readBoundedIngressBytes(req, MAX_BFF_CAS_INGRESS_BYTES);
    if (!rawBytes) {
      return quotaExceededResponse();
    }
    const url = new URL(req.url || "/", "http://127.0.0.1");
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/cas/upload",
      rawQuery: url.search.replace(/^\?/, ""),
      rawBodyBytes: rawBytes,
      contentType: req.headers.get("content-type") || "application/octet-stream",
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
      bffSessionToken: ctx.bffSessionToken,
      isHumanWorkbench: ctx.isHumanWorkbench,
      humanSessionToken: ctx.humanSessionToken,
      signal: req.signal,
    });
  },
  {
    authMode: "strict-loopback",
    rawBinary: true,
    body: CasUploadMetadataSchema,
  }
);
