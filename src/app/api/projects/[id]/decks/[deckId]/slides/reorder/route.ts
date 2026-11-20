// @story 06.1
// @implements AC-02, AC-03
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ReorderSlidesRequestSchema } from "@/lib/validations/presentation";

export const PUT = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const deckId = ctx.params?.deckId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/reorder`,
      body
    );
  },
  { authMode: "strict-loopback", roles: ["editor", "admin"], body: ReorderSlidesRequestSchema }
);
