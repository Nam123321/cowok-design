// @story 02.4
// @implements AC-02, AC-04
import { guardedRoute } from "@/lib/api-guard";
import {
  buildSsotErrorResponse,
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  AppShellLockMutationResponseSchema,
  AppShellLockRequestSchema,
} from "@/lib/validations/ssot-registry";

const HTTP_FORBIDDEN = 403;

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, AppShellLockRequestSchema, requestId);
    if (!parsed.ok) return parsed.response;

    const effectiveOwnerId = parsed.data.lockOwnerId ?? ctx.userId ?? "local-operator";
    if (parsed.data.action === "TRANSFER") {
      const reason = parsed.data.forceTransferReason?.trim() ?? "";
      if (ctx.role !== "admin" || reason.length === 0) {
        return buildSsotErrorResponse({
          status: HTTP_FORBIDDEN,
          code: "LOCK_TRANSFER_FORBIDDEN",
          message: "TRANSFER requires ADMIN role and non-empty forceTransferReason",
          requestId,
          field: !reason ? "forceTransferReason" : undefined,
        });
      }
    }

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/ssot/appshell/lock",
      body: {
        ...parsed.data,
        lockOwnerId: effectiveOwnerId,
      },
      req,
      ctx,
      requestId,
      successSchema: AppShellLockMutationResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    rawBinary: true,
    body: AppShellLockRequestSchema,
    roles: ["editor", "admin"],
  }
);
