// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { UpsertMcpServerInputSchema } from "@/lib/validations/workbench-settings";

export const GET = guardedRoute(
  (req, ctx) => forwardFromGuardedContext(req, ctx, "/api/v1/mcp-servers"),
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFromGuardedContext(req, ctx, "/api/v1/mcp-servers", body),
  { body: UpsertMcpServerInputSchema, authMode: "strict-loopback" }
);
