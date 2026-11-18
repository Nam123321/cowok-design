// @story 04.3, 04.5
// @implements Clean-Room EV-002 (Synced CLI & BYOS Provider Usage / 5h & Weekly Quota Probe)
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => forwardFromGuardedContext(req, ctx, "/api/v1/doctor/usage"),
  { authMode: "strict-loopback" }
);
