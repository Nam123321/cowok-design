// @story 05.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext, forwardToDaemonFlow } from "@/lib/daemon-client";
import { CreateSnapshotSchema } from "@/lib/validations/version-snapshot";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const daemonPath = `/api/v1/projects/${encodeURIComponent(projectId)}/snapshots`;
    const url = new URL(req.url, "http://127.0.0.1");
    const kind = url.searchParams.get("kind")?.trim() || "";
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath,
      rawQuery: kind ? `kind=${encodeURIComponent(kind)}` : "",
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/snapshots`,
      body
    );
  },
  { authMode: "strict-loopback", body: CreateSnapshotSchema }
);
