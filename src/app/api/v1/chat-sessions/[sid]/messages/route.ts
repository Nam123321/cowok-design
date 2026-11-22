// @story 04.6
// @implements AC-01, AC-04, AC-05 (Guarded BFF Route for Persistent SQLite Chat Messages)
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { CreateChatMessageRequestSchema } from "@/lib/validations/chat-vault";

export const GET = guardedRoute(
  async (req, ctx) => {
    const sid = encodeURIComponent(ctx.params?.sid || "");
    return forwardFromGuardedContext(req, ctx, `/api/v1/chat-sessions/${sid}/messages`);
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    const sid = encodeURIComponent(ctx.params?.sid || "");
    return forwardFromGuardedContext(req, ctx, `/api/v1/chat-sessions/${sid}/messages`, body);
  },
  { authMode: "strict-loopback", body: CreateChatMessageRequestSchema }
);
