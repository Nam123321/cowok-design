// @story 06.3
// @implements AC-02, AC-03, AC-04 (Slide Copilot Lease & Message Helpers)
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";
import type { StructuredCommentEnvelope } from "../../../lib/validations/element-comment";
import { DEFAULT_FALLBACK_SLIDE_UID, type WordingSlotDto } from "../../../lib/validations/presentation";
import {
  SLIDE_COPILOT_LEASE_TTL_MS,
  type SlideCopilotLeaseDto,
  type SlideCopilotStructuredComment,
} from "../../../lib/validations/presentation-charts";

export const COPILOT_LEASE_TICK_MS = 1_000;
export const COPILOT_LEASE_GRACE_MS = 1_500;
export const COPILOT_PREFLUSH_TIMEOUT_MS = 15_000;
export const COPILOT_QUEUED_TIMEOUT_MS = 120_000;
export const COPILOT_MAX_SLOTS = 4;
export const COPILOT_TOAST_TTL_MS = 6_000;
export const COPILOT_IME_KEY_CODE = 229;
export const COPILOT_EMPTY_SHA256 = "0".repeat(64);
export const COPILOT_FALLBACK_UID = DEFAULT_FALLBACK_SLIDE_UID;

export type SlideCopilotLeaseStatus = "unlocked" | "preflushing" | "active" | "queued";

export interface SlideCopilotMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  targetFfId?: string;
  timestampMs: number;
  statusTag?: string;
}

export interface SlideCopilotLeaseState {
  slideUid: string;
  leaseState: SlideCopilotLeaseStatus;
  jobId?: string;
  fencingToken?: number;
  targetFfId?: string;
  queuePosition: number;
  queuedJobId?: string;
  queuedFencingToken?: number;
  expiresAtMs?: number;
  startedAtMs?: number;
  remainingSeconds: number;
  errorBanner?: { code: string; message: string; targetFfId?: string } | null;
}

export interface SlideCopilotReceiptToast {
  id: string;
  slideUid: string;
  newRevision: number;
  patchedCount: number;
  resolvedCommentId?: string | null;
}

export interface CopilotSseHandlerDeps {
  getSnapshot: (uid: string) => { revision: number; contentSha256: string };
  applyCommit: (args: { copilotJobId: string; targetSlideUid: string; newRevision: number; contentSha256: string; slots: WordingSlotDto[]; sanitizedHtml?: string }) => void;
  setLeases: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotLeaseState>>>;
  setMessages: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotMessage[]>>>;
  setChartErrors: React.Dispatch<React.SetStateAction<Record<string, { code: string; message: string } | null>>>;
  setToast: React.Dispatch<React.SetStateAction<SlideCopilotReceiptToast | null>>;
  setGlobalSlots: React.Dispatch<React.SetStateAction<number>>;
  t?: (key: string, fallback: string) => string;
  formatNumber?: (n: number) => string;
}

export function createEmptyCopilotLease(slideUid: string): SlideCopilotLeaseState {
  return { slideUid, leaseState: "unlocked", queuePosition: 0, remainingSeconds: 0, errorBanner: null };
}

export function computeLeaseRemainingSec(expiresAtMs: number | undefined, nowMs: number): number {
  if (!expiresAtMs || expiresAtMs <= nowMs) return 0;
  return Math.max(0, Math.ceil((expiresAtMs - nowMs) / COPILOT_LEASE_TICK_MS));
}

export function extractCopilotApiError(err: unknown): { code: string; message: string; status: number } {
  if (err instanceof HarnessDaemonApiError) {
    return { code: err.code || "E_COPILOT_FAILED", message: err.message, status: err.status };
  }
  const msg = err instanceof Error ? err.message : "E_COPILOT_FAILED";
  const code = msg.includes("E_PREFLUSH_CONFLICT") ? "E_PREFLUSH_CONFLICT" : "E_COPILOT_FAILED";
  return { code, message: msg, status: 500 };
}

export function toLeaseStateFromDto(
  dto: SlideCopilotLeaseDto,
  nowMs: number,
  prevLease?: SlideCopilotLeaseState
): SlideCopilotLeaseState {
  const isRunning = dto.status === "RUNNING";
  const isQueued = dto.status === "QUEUED";
  if (isQueued && prevLease?.leaseState === "active" && prevLease.jobId && prevLease.jobId !== dto.jobId) {
    return { ...prevLease, queuePosition: Math.max(1, dto.queuePosition), queuedJobId: dto.jobId, queuedFencingToken: dto.fencingToken, errorBanner: null };
  }
  const leaseState: SlideCopilotLeaseStatus = isRunning ? "active" : isQueued ? "queued" : "unlocked";
  const expiresAtMs = isRunning ? (dto.leaseExpiresAtMs ?? nowMs + SLIDE_COPILOT_LEASE_TTL_MS) : undefined;
  return {
    slideUid: dto.slideUid,
    leaseState,
    jobId: isRunning || isQueued ? dto.jobId : undefined,
    fencingToken: dto.fencingToken,
    targetFfId: dto.targetFfId || prevLease?.targetFfId,
    queuePosition: isQueued ? Math.max(1, dto.queuePosition) : 0,
    expiresAtMs,
    remainingSeconds: computeLeaseRemainingSec(expiresAtMs, nowMs),
    errorBanner: null,
  };
}

export function toCopilotCommentPayload(env?: StructuredCommentEnvelope | null): SlideCopilotStructuredComment | undefined {
  if (!env) return undefined;
  return {
    commentId: env.commentId, projectId: env.projectId, nodeId: env.nodeId, slideUid: env.slideUid,
    targetType: env.targetType, elementFfId: env.elementFfId, coveredFfIds: env.coveredFfIds,
    regionRect: env.regionRect, domHash: env.domHash, artifactSha256: env.artifactSha256,
    commentBody: env.commentBody, targetLabel: env.targetLabel,
  };
}

export function appendUniqueCopilotMessage(
  prev: Record<string, SlideCopilotMessage[]>,
  slideUid: string,
  msg: SlideCopilotMessage
): Record<string, SlideCopilotMessage[]> {
  const existing = prev[slideUid] ?? [];
  if (existing.some((item) => item.id === msg.id)) return prev;
  return { ...prev, [slideUid]: [...existing, msg] };
}
