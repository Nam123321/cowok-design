// @story 06.2
// @implements AC-02, AC-04, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const deckId = ctx.params?.deckId || "";
    const slideUid = ctx.params?.slideUid || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/${encodeURIComponent(slideUid)}/slots`
    );
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
