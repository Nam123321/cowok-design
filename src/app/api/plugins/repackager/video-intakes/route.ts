// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { VideoIntakeRequestSchema } from "@/lib/validations/plugins-repackager";

export const POST = guardedRoute(
  (req, ctx, { body }) =>
    forwardFromGuardedContext(req, ctx, "/api/v1/plugins/repackager/video-intakes", body),
  { authMode: "strict-loopback", body: VideoIntakeRequestSchema }
);
