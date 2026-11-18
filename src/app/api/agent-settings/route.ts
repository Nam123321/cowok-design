// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { UpsertAgentSettingInputSchema } from "@/lib/validations/workbench-settings";

export const GET = guardedRoute(
  (req, ctx) => forwardFromGuardedContext(req, ctx, "/api/v1/agent-settings"),
  { authMode: "strict-loopback" }
);

export const PUT = guardedRoute(
  (req, ctx, { body }) => forwardFromGuardedContext(req, ctx, "/api/v1/agent-settings", body),
  { body: UpsertAgentSettingInputSchema, authMode: "strict-loopback" }
);
