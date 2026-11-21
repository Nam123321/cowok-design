// @story 04.6
// @implements AC-04, AC-05 (Guarded BFF Route for Zero-Leak In-Chat Key Vault Ingestion)
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ChatVaultIngestRequestSchema } from "@/lib/validations/chat-vault";

export const POST = guardedRoute(
  async (req, ctx, { body }) =>
    forwardFromGuardedContext(req, ctx, "/api/v1/vault/chat-ingest", body),
  { authMode: "strict-loopback", body: ChatVaultIngestRequestSchema }
);
