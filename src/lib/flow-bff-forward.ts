// @story 04.4
// @implements AC-01, AC-02, AC-04 (Shared BFF Forwarder for Flow Job & Session Routes)
import { type GuardedContext } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";

export interface ForwardFlowProjectOptions<T = unknown> {
  method: "GET" | "POST";
  req: Request;
  ctx: GuardedContext;
  suffix: string;
  body?: T;
}

export function forwardFlowProjectRequest<T = unknown>(opts: ForwardFlowProjectOptions<T>) {
  const { method, req, ctx, suffix, body } = opts;
  const projectId = ctx.projectId || ctx.params?.id || "";
  return forwardToDaemonFlow({
    method,
    daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/${suffix}`,
    ...(body !== undefined ? { body } : {}),
    ...(method === "GET" ? { rawQuery: req.url.includes("?") ? req.url.slice(req.url.indexOf("?") + 1) : "" } : {}),
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
