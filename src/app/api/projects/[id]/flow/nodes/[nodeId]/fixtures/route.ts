// @story 04.2
// @implements AC-01, AC-02, AC-03
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { CreateScreenFixtureSchema } from "@/lib/validations/screen-fixture";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures`
    );
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures`,
      body
    );
  },
  { authMode: "strict-loopback", body: CreateScreenFixtureSchema }
);
