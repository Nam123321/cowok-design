// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { DiscoverOrInstallPluginRequestSchema } from "@/lib/validations/plugins";

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFromGuardedContext(req, ctx, "/api/v1/plugins/installs", body),
  { authMode: "strict-loopback", body: DiscoverOrInstallPluginRequestSchema }
);
