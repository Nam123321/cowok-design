// @story 08.3
// @implements AC-04, AC-06
import crypto from "node:crypto";
import { LearnError } from "./lifecycle-errors";
import {
  ActorKind,
  ActorKindSchema,
  CardDecisionAction,
  CardDecisionActionSchema,
  HumanApproverToken,
} from "./lifecycle-types";
import { isValidSha256Hex, utf8Bytes, validateScopeSegment } from "./learn-sanitizer";

const APPROVER_HMAC_DOMAIN = "cowok-learn-approver-v2";
const AUTHORITY_KEYS = new WeakMap<HumanSessionAuthority, { kid: string; key: Buffer }>();

export interface MintTokenInput {
  actor_kind: ActorKind | string;
  actor_id: string;
  tenant_id: string;
  project_id: string;
  card_id: string;
  content_sha256: string;
  action: CardDecisionAction;
  expected_revision: number;
  reason: string;
  now_s: number;
  ttl_s: number;
  nonce: string;
}

export function buildTokenPayload(t: Omit<HumanApproverToken, "hmac_sha256">): string {
  return [
    APPROVER_HMAC_DOMAIN,
    utf8Bytes(t.kid),
    t.kid,
    t.actor_kind,
    utf8Bytes(t.actor_id),
    t.actor_id,
    utf8Bytes(t.tenant_id),
    t.tenant_id,
    utf8Bytes(t.project_id),
    t.project_id,
    utf8Bytes(t.card_id),
    t.card_id,
    utf8Bytes(t.content_sha256),
    t.content_sha256,
    t.action,
    t.expected_revision,
    utf8Bytes(t.reason),
    t.reason,
    t.issued_at_s,
    t.expires_at_s,
    utf8Bytes(t.nonce),
    t.nonce,
    "end",
  ].join("|");
}

export function readAuthorityKeyEntry(authority: HumanSessionAuthority): { kid: string; key: Buffer } {
  const entry = AUTHORITY_KEYS.get(authority);
  if (!entry) throw new LearnError("UnauthorizedApprover", "Signing authority invalid");
  return entry;
}

/** HMAC-SHA256 authority for HUMAN_WORKBENCH_SESSION LEARN approvals (AC-04, EC-E08-06). */
export class HumanSessionAuthority {
  private constructor(kid: string, keyBytes: Buffer) {
    if (keyBytes.length !== 32) throw new LearnError("UnauthorizedApprover", "HumanSessionAuthority requires a 32-byte signing key");
    AUTHORITY_KEYS.set(this, { kid: validateScopeSegment("kid", kid), key: Buffer.from(keyBytes) });
  }

  public static fromKeyBytes(keyBytes: Uint8Array | Buffer): HumanSessionAuthority {
    return new HumanSessionAuthority("default", Buffer.from(keyBytes));
  }

  public static fromKeyWithKid(kid: string, keyBytes: Uint8Array | Buffer): HumanSessionAuthority {
    return new HumanSessionAuthority(kid, Buffer.from(keyBytes));
  }

  public static randomEphemeral(): HumanSessionAuthority {
    return new HumanSessionAuthority("default", crypto.randomBytes(32));
  }

  public mintToken(req: MintTokenInput): HumanApproverToken {
    const actorKind = ActorKindSchema.parse(req.actor_kind);
    if (actorKind !== "HUMAN_WORKBENCH_SESSION") throw new LearnError("UnauthorizedApprover", "Only HUMAN_WORKBENCH_SESSION may mint approval tokens");
    const action = CardDecisionActionSchema.parse(req.action);
    const [aid, tid, pid, cid, sha, rs, n] = [req.actor_id, req.tenant_id, req.project_id, req.card_id, req.content_sha256, req.reason, req.nonce].map((s) => s.trim());
    const validTiming =
      Number.isSafeInteger(req.expected_revision) && req.expected_revision >= 1 &&
      Number.isSafeInteger(req.now_s) && req.now_s > 0 &&
      Number.isSafeInteger(req.ttl_s) && req.ttl_s >= 1 && req.ttl_s <= 900 && Number.isSafeInteger(req.now_s + req.ttl_s);
    if (!aid || !tid || !pid || !cid || !n || !isValidSha256Hex(sha) || utf8Bytes(rs) < 4 || !validTiming) {
      throw new LearnError("UnauthorizedApprover", "Malformed actor, scope, card binding, reason (<4B), nonce, timestamp, or TTL (1..=900s)");
    }
    const entry = readAuthorityKeyEntry(this);
    const unsigned: Omit<HumanApproverToken, "hmac_sha256"> = {
      kid: entry.kid, actor_kind: actorKind, actor_id: aid, tenant_id: tid, project_id: pid,
      card_id: cid, content_sha256: sha, action, expected_revision: req.expected_revision,
      reason: rs, issued_at_s: req.now_s, expires_at_s: req.now_s + req.ttl_s, nonce: n,
    };
    const hmacSha256 = crypto.createHmac("sha256", entry.key).update(buildTokenPayload(unsigned), "utf8").digest("hex");
    return { ...unsigned, hmac_sha256: hmacSha256 };
  }
}
