// @story 01.3
// @implements AC-01, AC-02, AC-04, AC-05
import { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { PutSpecInputSchema } from "@/lib/validations/harness-daemon";

const DeleteSpecQuerySchema = z.object({
  expected_sha256: z.string().min(1).optional(),
});

export const PUT = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const specKey = ctx.params?.spec_key || "";
    return forwardToDaemonFlow({
      method: "PUT",
      daemonPath: `/api/v1/projects/${projectId}/specs/${encodeURIComponent(specKey)}`,
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", body: PutSpecInputSchema }
);

export const DELETE = guardedRoute(
  async (req, ctx, { query }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const specKey = ctx.params?.spec_key || "";
    const rawQuery = query?.expected_sha256
      ? `expected_sha256=${encodeURIComponent(query.expected_sha256)}`
      : "";
    return forwardToDaemonFlow({
      method: "DELETE",
      daemonPath: `/api/v1/projects/${projectId}/specs/${encodeURIComponent(specKey)}`,
      rawQuery,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", query: DeleteSpecQuerySchema }
);
