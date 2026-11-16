// @story 05.1
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { UpdateScreenFixtureSchema } from "@/lib/validations/screen-fixture";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    const fixtureId = ctx.params?.fixtureId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures/${encodeURIComponent(fixtureId)}`
    );
  },
  { authMode: "strict-loopback" }
);

export const PATCH = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    const fixtureId = ctx.params?.fixtureId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures/${encodeURIComponent(fixtureId)}`,
      body
    );
  },
  { authMode: "strict-loopback", body: UpdateScreenFixtureSchema }
);
