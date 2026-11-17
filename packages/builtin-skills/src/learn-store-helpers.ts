// @story 08.3
// @implements AC-04, AC-06
import crypto from "node:crypto";
import { buildTokenPayload } from "./learn-authority";
import { ConsumedNonceRecord, LearnError } from "./lifecycle-errors";
import { HumanApproverToken, KnowledgeCard, KnowledgeCardSchema } from "./lifecycle-types";
import { computeCardSha256, escapeXml, isValidSha256Hex, utf8Bytes, validateLearnCardField, validateScopeSegment } from "./learn-sanitizer";

const MAX_CARD_TITLE_BYTES = 160;
const MAX_CARD_CONTENT_BYTES = 4096;
const MAX_CARD_EVIDENCE_BYTES = 512;

export interface PipelineState {
  cards: KnowledgeCard[];
  keysByKid: Map<string, Buffer>;
  consumedNonces: Map<string, number>;
}

export const scopeNoncePrefix = (tid: string, pid: string): string =>
  `${utf8Bytes(tid)}:${tid}|${utf8Bytes(pid)}:${pid}|`;

export const scopeNonceKey = (tid: string, pid: string, nonce: string): string =>
  `${scopeNoncePrefix(tid, pid)}${nonce}`;

function decodeLenPrefixedSegment(buf: Buffer, offset: number): { value: string; nextOffset: number } | null {
  const colonIdx = buf.indexOf(0x3a, offset);
  if (colonIdx <= offset) return null;
  const len = Number(buf.subarray(offset, colonIdx).toString("utf8"));
  const start = colonIdx + 1;
  if (!Number.isSafeInteger(len) || start + len + 1 > buf.length) return null;
  return { value: buf.subarray(start, start + len).toString("utf8"), nextOffset: start + len + 1 };
}

export function decodeScopeNonceRecords(nonces: Map<string, number>): ConsumedNonceRecord[] {
  const out: ConsumedNonceRecord[] = [];
  for (const [k, expiresAtS] of nonces.entries()) {
    const buf = Buffer.from(k, "utf8");
    const seg1 = decodeLenPrefixedSegment(buf, 0);
    const seg2 = seg1 ? decodeLenPrefixedSegment(buf, seg1.nextOffset) : null;
    if (seg1 && seg2) {
      out.push({ tenant_id: seg1.value, project_id: seg2.value, nonce: buf.subarray(seg2.nextOffset).toString("utf8"), expires_at_s: expiresAtS });
    }
  }
  return out;
}

export function renderCardXmlBlock(card: KnowledgeCard): string {
  return (
    `  <card id="${escapeXml(card.id)}" category="${card.category}" tenant="${escapeXml(card.tenant_id)}" project="${escapeXml(card.project_id)}">\n` +
    `    <title>${escapeXml(card.title)}</title>\n` +
    `    <evidence>${escapeXml(card.source_evidence)}</evidence>\n` +
    `    <content>${escapeXml(card.content)}</content>\n` +
    `  </card>\n`
  );
}

export function hydratePersistedCard(rawCard: KnowledgeCard): KnowledgeCard {
  const card = KnowledgeCardSchema.parse(rawCard);
  const [tid, pid, cid] = [validateScopeSegment("tenant_id", card.tenant_id), validateScopeSegment("project_id", card.project_id), validateScopeSegment("id", card.id)];
  const title = validateLearnCardField("title", card.title, MAX_CARD_TITLE_BYTES);
  const content = validateLearnCardField("content", card.content, MAX_CARD_CONTENT_BYTES);
  const evidence = validateLearnCardField("source_evidence", card.source_evidence, MAX_CARD_EVIDENCE_BYTES);
  const sha = computeCardSha256(title, content);
  if (card.content_sha256 !== sha || card.revision < 1) {
    throw new LearnError("InvalidCardInput", `Persisted card '${cid}' failed revision or content_sha256 invariant`);
  }
  return {
    ...card,
    id: cid, tenant_id: tid, project_id: pid,
    idempotency_key: card.idempotency_key ? validateScopeSegment("idempotency_key", card.idempotency_key) : null,
    title, content, content_sha256: sha, source_evidence: evidence,
  };
}

export function verifyTokenSignatureAndTtl(st: PipelineState, tenantId: string, projectId: string, token: HumanApproverToken, nowS: number): void {
  const badText = [token.kid, token.actor_id, token.tenant_id, token.project_id, token.card_id, token.nonce].some((f) => !f || f !== f.trim());
  const badMeta = badText || token.reason !== token.reason.trim() || utf8Bytes(token.reason) < 4 || !isValidSha256Hex(token.content_sha256) || !Number.isSafeInteger(token.expected_revision) || token.expected_revision < 1;
  if (token.actor_kind !== "HUMAN_WORKBENCH_SESSION" || badMeta || token.tenant_id !== tenantId || token.project_id !== projectId) {
    throw new LearnError("UnauthorizedApprover", "Invalid actor, token metadata, or tenant/project scope");
  }
  const ttl = token.expires_at_s - token.issued_at_s;
  const timeOk = Number.isSafeInteger(nowS) && nowS > 0 && Number.isSafeInteger(token.issued_at_s) && Number.isSafeInteger(token.expires_at_s) && token.issued_at_s > 0 && ttl >= 1 && ttl <= 900 && nowS >= token.issued_at_s && nowS <= token.expires_at_s;
  const key = st.keysByKid.get(token.kid);
  if (!timeOk || !key) throw new LearnError("UnauthorizedApprover", key ? "Token expired, not yet valid, or bounded TTL violated" : `Unknown signing kid '${token.kid}'`);
  const expected = crypto.createHmac("sha256", key).update(buildTokenPayload(token), "utf8").digest("hex");
  if (!isValidSha256Hex(token.hmac_sha256) || !crypto.timingSafeEqual(Buffer.from(expected, "utf8"), Buffer.from(token.hmac_sha256, "utf8"))) {
    throw new LearnError("UnauthorizedApprover", "Forged or mismatched HumanApproverToken HMAC signature");
  }
}
