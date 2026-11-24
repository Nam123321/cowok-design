// @story 14.1
// @implements AC-01, AC-02 (BFF Proxy for GET/POST /api/v1/support/tickets)
import { z } from "zod";
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  CreateSupportTicketRequestSchema,
  CreateSupportTicketResponseSchema,
  DEFAULT_SUPPORT_TICKETS_LIMIT,
  ListSupportTicketsResponseSchema,
  MAX_SUPPORT_TICKETS_LIMIT,
  MIN_SUPPORT_TICKETS_LIMIT,
  type SupportErrorCode,
  SupportErrorEnvelopeSchema,
} from "@/lib/validations/support";

const MAX_SUPPORT_TICKET_BYTES = 8_388_608;
const DECIMAL_RADIX = 10;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildSupportError(
  status: number,
  code: SupportErrorCode,
  message: string,
  requestId: string
): Response {
  return new Response(
    JSON.stringify({ error: { code, message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

function mapFlatAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  let code: SupportErrorCode = "INVALID_REQUEST";
  if (status === HTTP_UNAUTHORIZED) code = "UNAUTHORIZED";
  else if (status === HTTP_FORBIDDEN) code = "AUTH_FORBIDDEN";
  else if (status === HTTP_PAYLOAD_TOO_LARGE) code = "PAYLOAD_TOO_LARGE";
  return buildSupportError(status, code, msg, requestId);
}

async function validateDaemonResp<T>(
  daemonResp: Response,
  schema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string,
  label: string
): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = schema.safeParse(parsed);
    if (daemonResp.ok && okData.success) {
      return new Response(JSON.stringify(okData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const errData = SupportErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) {
      return new Response(JSON.stringify(errData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const flatErr = !daemonResp.ok ? mapFlatAuthErr(daemonResp.status, parsed, requestId) : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed error envelope
  }
  return buildSupportError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    `Daemon ${label} contract violation`,
    requestId
  );
}

interface ForwardSupportTicketsOptions {
  method: "GET" | "POST";
  req: Request;
  ctx: GuardedContext;
  requestId: string;
  rawQuery?: string;
  body?: unknown;
}

async function forwardSupportTickets(opts: ForwardSupportTicketsOptions): Promise<Response> {
  const { method, req, ctx, requestId, rawQuery, body } = opts;
  return forwardToDaemonFlow({
    method,
    daemonPath: "/api/v1/support/tickets",
    rawQuery,
    body,
    inboundHeaders: req.headers,
    principal: {
      tenantId: ctx.tenantId || "local-default",
      userId: ctx.userId || "local-operator",
      role: ctx.role || "viewer",
    },
    correlationId: requestId,
    bffSessionToken: ctx.bffSessionToken,
    isHumanWorkbench: ctx.isHumanWorkbench,
    humanSessionToken: ctx.humanSessionToken,
    signal: req.signal,
  });
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const limitParam = Number.parseInt(
      url.searchParams.get("limit") ?? String(DEFAULT_SUPPORT_TICKETS_LIMIT),
      DECIMAL_RADIX
    );
    const safeLimit = Number.isFinite(limitParam)
      ? Math.max(MIN_SUPPORT_TICKETS_LIMIT, Math.min(MAX_SUPPORT_TICKETS_LIMIT, limitParam))
      : DEFAULT_SUPPORT_TICKETS_LIMIT;
    const daemonResp = await forwardSupportTickets({
      method: "GET",
      req,
      ctx,
      requestId,
      rawQuery: new URLSearchParams({ limit: String(safeLimit) }).toString(),
    });
    return validateDaemonResp(daemonResp, ListSupportTicketsResponseSchema, requestId, "/support/tickets");
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      CreateSupportTicketRequestSchema,
      requestId,
      MAX_SUPPORT_TICKET_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardSupportTickets({
      method: "POST",
      req,
      ctx,
      requestId,
      body: parsed.data,
    });
    return validateDaemonResp(daemonResp, CreateSupportTicketResponseSchema, requestId, "/support/tickets");
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: CreateSupportTicketRequestSchema,
  }
);
