// @story 04.6
// @implements AC-04, AC-05 (Guarded BFF Route for Vault Receipt Replace & Revoke)
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ReplaceVaultReceiptRequestSchema } from "@/lib/validations/chat-vault";

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    const rid = encodeURIComponent(ctx.params?.receipt_id || "");
    return forwardFromGuardedContext(req, ctx, `/api/v1/vault/receipts/${rid}`, body);
  },
  { authMode: "strict-loopback", body: ReplaceVaultReceiptRequestSchema }
);

export const DELETE = guardedRoute(
  async (req, ctx) => {
    const rid = encodeURIComponent(ctx.params?.receipt_id || "");
    return forwardFromGuardedContext(req, ctx, `/api/v1/vault/receipts/${rid}`);
  },
  { authMode: "strict-loopback" }
);
