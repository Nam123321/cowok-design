// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Vite BFF Chat Persistence, CAS Intake & Vault Subtable)
import * as chatMessagesRoute from "../app/api/v1/chat-sessions/[sid]/messages/route";
import * as casUploadRoute from "../app/api/v1/cas/upload/route";
import * as vaultIngestRoute from "../app/api/v1/vault/chat-ingest/route";
import * as vaultReceiptRoute from "../app/api/v1/vault/receipts/[receipt_id]/route";
import { safeDecodeSegment, type RouteEntry } from "./vite-bff-flow-routes";

const noParams = (): Record<string, string> => ({});
const sidOnly = (m: RegExpExecArray): Record<string, string> => ({
  sid: safeDecodeSegment(m[1]),
});
const receiptOnly = (m: RegExpExecArray): Record<string, string> => ({
  receipt_id: safeDecodeSegment(m[1]),
});

export const CHAT_VAULT_ROUTES: RouteEntry[] = [
  {
    method: "GET",
    pattern: /^\/api\/chat-sessions\/([^/]+)\/messages$/,
    extractParams: sidOnly,
    resolveHandler: () => chatMessagesRoute.GET,
  },
  {
    method: "POST",
    pattern: /^\/api\/chat-sessions\/([^/]+)\/messages$/,
    extractParams: sidOnly,
    resolveHandler: () => chatMessagesRoute.POST,
  },
  {
    method: "POST",
    pattern: /^\/api\/cas\/upload$/,
    extractParams: noParams,
    resolveHandler: () => casUploadRoute.POST,
  },
  {
    method: "POST",
    pattern: /^\/api\/vault\/chat-ingest$/,
    extractParams: noParams,
    resolveHandler: () => vaultIngestRoute.POST,
  },
  {
    method: "POST",
    pattern: /^\/api\/vault\/receipts\/([^/]+)$/,
    extractParams: receiptOnly,
    resolveHandler: () => vaultReceiptRoute.POST,
  },
  {
    method: "DELETE",
    pattern: /^\/api\/vault\/receipts\/([^/]+)$/,
    extractParams: receiptOnly,
    resolveHandler: () => vaultReceiptRoute.DELETE,
  },
];
