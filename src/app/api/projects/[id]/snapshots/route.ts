// @story 05.3
import { guardedRoute } from "../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../lib/daemon-client";
import { CreateSnapshotSchema } from "../../../../../lib/validations/version-snapshot";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/snapshots`);
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/snapshots`, body);
  },
  { authMode: "strict-loopback", body: CreateSnapshotSchema }
);
