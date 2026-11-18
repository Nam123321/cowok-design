// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { VaultConfigInputSchema } from "@/lib/validations/workbench-settings";

export const PUT = guardedRoute(
  (req, ctx, { body }) => forwardFromGuardedContext(req, ctx, "/api/v1/settings/vault", body),
  { body: VaultConfigInputSchema, authMode: "strict-loopback" }
);
