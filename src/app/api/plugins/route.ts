// @story 04.3, 04.4, 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => forwardFromGuardedContext(req, ctx, "/api/v1/plugins"),
  { authMode: "strict-loopback" }
);
