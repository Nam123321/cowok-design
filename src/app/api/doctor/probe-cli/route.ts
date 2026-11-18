// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ProbeCliInputSchema } from "@/lib/validations/workbench-settings";

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFromGuardedContext(req, ctx, "/api/v1/doctor/probe-cli", body),
  { body: ProbeCliInputSchema, authMode: "strict-loopback" }
);
