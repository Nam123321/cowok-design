// @story 08.3
// @implements AC-04, AC-06
import type { GateReport } from "@cowok-design/shared-types";
import { HumanSessionAuthority, readAuthorityKeyEntry } from "./learn-authority";
import { ConsumedNonceRecord, LearnError } from "./lifecycle-errors";
import { CardDecisionAction, CardStatus, HumanApproverToken, KnowledgeCard, LearnCardCategory, LearnCardCategorySchema } from "./lifecycle-types";
import { computeCardSha256, nowUnixSeconds, validateLearnCardField, validateScopeSegment } from "./learn-sanitizer";
import {
  PipelineState,
  decodeScopeNonceRecords,
  hydratePersistedCard,
  renderCardXmlBlock,
  scopeNonceKey,
  scopeNoncePrefix,
  verifyTokenSignatureAndTtl,
} from "./learn-store-helpers";
import type { LintIssue } from "./types";
import { countExactCl100kTokens } from "./validator";

export const MAX_LEARN_PROMPT_TOKENS = 1500;
const MAX_CARDS_PER_SCOPE = 5000;
const MAX_NONCES_PER_SCOPE = 10000;
const PIPELINE_STATES = new WeakMap<LearnPipeline, PipelineState>();

export interface ProposeCardInput {
  tenant_id: string;
  project_id: string;
  id: string;
  idempotency_key?: string;
  category: LearnCardCategory | string;
  title: string;
  content: string;
  source_evidence: string;
  created_at?: number;
}

export interface PromptInjectionResult {
  card_ids: string[];
  total_tokens: number;
  fenced_xml: string;
}

export interface GateReportInput {
  gate_report: GateReport;
  issues?: LintIssue[];
}

/** Multi-tenant, human-gated LEARN card store with WeakMap secret isolation and cl100k_base prompt fencing. */
export class LearnPipeline {
  constructor(authority?: HumanSessionAuthority) {
    const entry = readAuthorityKeyEntry(authority ?? HumanSessionAuthority.randomEphemeral());
    PIPELINE_STATES.set(this, { cards: [], keysByKid: new Map([[entry.kid, Buffer.from(entry.key)]]), consumedNonces: new Map() });
  }

  private getState(): PipelineState {
    const st = PIPELINE_STATES.get(this);
    if (!st) throw new LearnError("UnauthorizedApprover", "Pipeline state unavailable");
    return st;
  }

  public registerAuthority(authority: HumanSessionAuthority): void {
    const entry = readAuthorityKeyEntry(authority);
    this.getState().keysByKid.set(entry.kid, Buffer.from(entry.key));
  }

  public static fromPersistedState(authority: HumanSessionAuthority, cards: KnowledgeCard[], consumedNonces: ConsumedNonceRecord[], nowS: number): LearnPipeline {
    const pipe = new LearnPipeline(authority);
    const st = pipe.getState();
    for (const rawCard of cards) {
      const hydrated = hydratePersistedCard(rawCard);
      if (st.cards.some((c) => c.tenant_id === hydrated.tenant_id && c.project_id === hydrated.project_id && c.id === hydrated.id)) {
        throw new LearnError("DuplicateCardId", `'${hydrated.id}' already exists in scope`);
      }
      st.cards.push(hydrated);
    }
    for (const rec of consumedNonces) {
      if (Number.isSafeInteger(rec.expires_at_s) && rec.expires_at_s >= nowS) {
        const key = scopeNonceKey(validateScopeSegment("nonce.tenant_id", rec.tenant_id), validateScopeSegment("nonce.project_id", rec.project_id), validateScopeSegment("nonce", rec.nonce));
        st.consumedNonces.set(key, rec.expires_at_s);
      }
    }
    return pipe;
  }

  public exportConsumedNonces(): ConsumedNonceRecord[] {
    return decodeScopeNonceRecords(this.getState().consumedNonces);
  }

  public pruneExpiredNonces(nowS: number): number {
    if (!Number.isSafeInteger(nowS)) return 0;
    const nonces = this.getState().consumedNonces;
    let removed = 0;
    for (const [k, exp] of nonces.entries()) {
      if (exp < nowS) {
        nonces.delete(k);
        removed += 1;
      }
    }
    return removed;
  }

  public proposeCardScoped(input: ProposeCardInput): KnowledgeCard {
    const [tid, pid, cid] = [validateScopeSegment("tenant_id", input.tenant_id), validateScopeSegment("project_id", input.project_id), validateScopeSegment("id", input.id)];
    const idemKey = input.idempotency_key !== undefined ? validateScopeSegment("idempotency_key", input.idempotency_key) : null;
    const category = LearnCardCategorySchema.parse(input.category);
    const title = validateLearnCardField("title", input.title, 160);
    const content = validateLearnCardField("content", input.content, 4096);
    const evidence = validateLearnCardField("source_evidence", input.source_evidence, 512);
    const sha = computeCardSha256(title, content);
    const { cards } = this.getState();

    const idemExisting = idemKey ? cards.find((c) => c.tenant_id === tid && c.project_id === pid && c.idempotency_key === idemKey) : undefined;
    if (idemExisting) {
      if (idemExisting.id === cid && idemExisting.category === category && idemExisting.content_sha256 === sha) {
        throw new LearnError("IdempotentReplay", `request already materialized as card '${idemExisting.id}'`, { card_id: idemExisting.id });
      }
      throw new LearnError("IdempotencyPayloadMismatch", "idempotency_key reused with conflicting card payload");
    }
    if (cards.filter((c) => c.tenant_id === tid && c.project_id === pid).length >= MAX_CARDS_PER_SCOPE) {
      throw new LearnError("CapacityExceeded", `Scope (${tid}, ${pid}) reached ${MAX_CARDS_PER_SCOPE} card limit`);
    }
    if (cards.some((c) => c.tenant_id === tid && c.project_id === pid && c.id === cid)) throw new LearnError("DuplicateCardId", `'${cid}' already exists in scope`);
    if (cards.some((c) => c.tenant_id === tid && c.project_id === pid && c.content_sha256 === sha && c.status !== "REJECTED")) {
      throw new LearnError("DuplicateActiveContent", "matching active/proposed content hash already exists");
    }
    if (input.created_at !== undefined && (!Number.isSafeInteger(input.created_at) || input.created_at <= 0)) {
      throw new LearnError("InvalidCardInput", "created_at must be a positive integer unix timestamp");
    }
    const card: KnowledgeCard = {
      id: cid, tenant_id: tid, project_id: pid, idempotency_key: idemKey, category, title, content,
      content_sha256: sha, status: "PROPOSED", source_evidence: evidence, revision: 1,
      reviewed_by: null, decision_reason: null, created_at: input.created_at ?? nowUnixSeconds(), accepted_at: null,
    };
    cards.push(card);
    return structuredClone(card);
  }

  public proposeCardFromGateReport(tenantId: string, projectId: string, id: string, reportInput: GateReportInput, idempotencyKey?: string): KnowledgeCard {
    const gr = reportInput.gate_report;
    if (gr.passed) throw new LearnError("InvalidCardInput", "Cannot synthesize failure card from a passed GateReport");
    const issues = reportInput.issues ?? [];
    const content = issues.length === 0
      ? `Gate '${gr.gate_name.trim()}' reported failure by checker '${gr.checker_id.trim()}' (evidence ${gr.evidence_hash.trim()}).`
      : issues.slice(0, 6).map((iss) => `[${iss.rule_id.trim()}] ${iss.message.trim()}`).join("; ");
    const rawTs = gr.timestamp >= 1_000_000_000_000 ? Math.floor(gr.timestamp / 1000) : Math.floor(gr.timestamp);
    const normalizedTs = Number.isSafeInteger(rawTs) ? Math.max(1, rawTs) : nowUnixSeconds();
    return this.proposeCardScoped({
      tenant_id: tenantId, project_id: projectId, id, idempotency_key: idempotencyKey, category: "ANTI_PATTERN",
      title: Array.from(`Gate failure remediation: ${gr.gate_name.trim()}`).slice(0, 120).join(""),
      content, source_evidence: `gate:${gr.gate_name.trim()}:${gr.evidence_hash.trim()}`, created_at: normalizedTs,
    });
  }

  private decideCard(tenantId: string, projectId: string, id: string, action: CardDecisionAction, token: HumanApproverToken, expectedRevision: number, nowS: number): void {
    const [tid, pid, cid] = [tenantId.trim(), projectId.trim(), id.trim()];
    const st = this.getState();
    verifyTokenSignatureAndTtl(st, tid, pid, token, nowS);
    const nKey = scopeNonceKey(tid, pid, token.nonce);
    if (st.consumedNonces.has(nKey)) throw new LearnError("ReplayedApprovalNonce", `nonce '${token.nonce}' already consumed`);
    const target: Exclude<CardStatus, "PROPOSED"> = action === "ACCEPT" ? "ACCEPTED" : "REJECTED";
    const card = st.cards.find((c) => c.id === cid && c.tenant_id === tid && c.project_id === pid);
    if (!card) throw new LearnError("CardNotFound", `'${cid}'`);
    if (card.status !== "PROPOSED") throw new LearnError("InvalidCardTransition", `cannot move card from ${card.status} to ${target}`);
    if (card.revision !== expectedRevision) throw new LearnError("RevisionConflict", `expected ${expectedRevision}, actual ${card.revision}`);
    if (nowS < card.created_at || token.card_id !== card.id || token.content_sha256 !== card.content_sha256 || token.action !== action || token.expected_revision !== expectedRevision) {
      throw new LearnError("UnauthorizedApprover", "HumanApproverToken binding mismatch or decision precedes creation");
    }
    this.pruneExpiredNonces(nowS);
    const scopePrefix = scopeNoncePrefix(tid, pid);
    let scopeNonces = 0;
    for (const k of st.consumedNonces.keys()) {
      if (k.startsWith(scopePrefix)) scopeNonces += 1;
    }
    if (!st.consumedNonces.has(nKey) && scopeNonces >= MAX_NONCES_PER_SCOPE) {
      throw new LearnError("CapacityExceeded", `Scope (${tid}, ${pid}) reached ${MAX_NONCES_PER_SCOPE} active nonce limit`);
    }
    st.consumedNonces.set(nKey, token.expires_at_s);
    card.status = target;
    card.reviewed_by = token.actor_id;
    card.decision_reason = token.reason;
    card.revision += 1;
    if (target === "ACCEPTED") card.accepted_at = nowS;
  }

  public acceptCardVerified(tenantId: string, projectId: string, id: string, token: HumanApproverToken, expectedRevision: number, nowS: number): void {
    this.decideCard(tenantId, projectId, id, "ACCEPT", token, expectedRevision, nowS);
  }

  public rejectCardVerified(tenantId: string, projectId: string, id: string, token: HumanApproverToken, expectedRevision: number, nowS: number): void {
    this.decideCard(tenantId, projectId, id, "REJECT", token, expectedRevision, nowS);
  }

  public listCardsForScope(tenantId: string, projectId: string): KnowledgeCard[] {
    const [tid, pid] = [tenantId.trim(), projectId.trim()];
    return structuredClone(this.getState().cards.filter((c) => c.tenant_id === tid && c.project_id === pid));
  }

  public renderPromptFencedCards(tenantId: string, projectId: string, maxTokens = MAX_LEARN_PROMPT_TOKENS): PromptInjectionResult {
    const [tid, pid] = [tenantId.trim(), projectId.trim()];
    const safeMax = Number.isFinite(maxTokens) ? maxTokens : MAX_LEARN_PROMPT_TOKENS;
    const limit = Math.min(Math.max(0, Math.floor(safeMax)), MAX_LEARN_PROMPT_TOKENS);
    const eligible = this.getState().cards.filter((c) => c.status === "ACCEPTED" && c.tenant_id === tid && c.project_id === pid)
      .sort((a, b) => (a.created_at !== b.created_at ? (a.created_at < b.created_at ? -1 : 1) : a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    const cardIds: string[] = [];
    let body = "";
    for (const card of eligible) {
      const block = renderCardXmlBlock(card);
      if (countExactCl100kTokens(`<LEARNED_CARDS_DATA>\n${body}${block}</LEARNED_CARDS_DATA>`) <= limit) {
        body += block;
        cardIds.push(card.id);
      }
    }
    const fencedXml = cardIds.length === 0 ? "" : `<LEARNED_CARDS_DATA>\n${body}</LEARNED_CARDS_DATA>`;
    return { card_ids: cardIds, total_tokens: countExactCl100kTokens(fencedXml), fenced_xml: fencedXml };
  }

  public getActivePromptInjections(tenantId: string, projectId: string, maxTokens = MAX_LEARN_PROMPT_TOKENS): KnowledgeCard[] {
    const [tid, pid] = [tenantId.trim(), projectId.trim()];
    const byId = new Map(this.getState().cards.filter((c) => c.tenant_id === tid && c.project_id === pid).map((c) => [c.id, c]));
    return this.renderPromptFencedCards(tid, pid, maxTokens).card_ids.map((id) => byId.get(id)).filter((c): c is KnowledgeCard => c !== undefined).map((c) => structuredClone(c));
  }
}
