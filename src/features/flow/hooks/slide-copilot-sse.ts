// @story 06.3
// @implements AC-02, AC-03, AC-04 (Slide Copilot SSE Event Processor)
import {
  CANONICAL_SLIDE_UID_REGEX,
  type WordingSlotDto,
} from "../../../lib/validations/presentation";
import { SLIDE_COPILOT_LEASE_TTL_MS } from "../../../lib/validations/presentation-charts";
import type { ExecutionEventDto } from "../../../lib/validations/workbench-settings";
import {
  appendUniqueCopilotMessage,
  computeLeaseRemainingSec,
  createEmptyCopilotLease,
  type CopilotSseHandlerDeps,
  type SlideCopilotLeaseState,
} from "./useSlideCopilotHelpers";

const COMPLETED_KINDS = new Set(["completed", "slide_copilot_committed", "job_completed"]);
const CANCELLED_KINDS = new Set(["cancelled", "slide_copilot_cancelled", "job_cancelled"]);
const FAILED_KINDS = new Set(["failed", "interrupted", "slide_copilot_failed", "job_failed", "e_copilot_lease_timeout"]);
const RUNNING_KINDS = new Set(["running", "started", "lease_acquired", "promoted", "slide_copilot_promoted", "slide_copilot_running"]);
const QUEUED_KINDS = new Set(["queued", "enqueued", "slide_copilot_queued"]);

function parseSseJson(raw: string): Record<string, unknown> {
  if (!raw || !raw.trim().startsWith("{")) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function resolveEventCategory(eventKind: string, jobStatus: string): string {
  const k = eventKind.trim().toLowerCase();
  if (COMPLETED_KINDS.has(k) || jobStatus === "COMPLETED") return "completed";
  if (CANCELLED_KINDS.has(k) || jobStatus === "CANCELLED") return "cancelled";
  if (FAILED_KINDS.has(k) || jobStatus === "FAILED" || jobStatus === "INTERRUPTED") return "failed";
  if (RUNNING_KINDS.has(k) || jobStatus === "RUNNING") return "running";
  if (QUEUED_KINDS.has(k) || jobStatus === "QUEUED") return "queued";
  return k;
}

function clearCommittedSlideChartErrors(
  prev: Record<string, { code: string; message: string } | null>,
  slots: WordingSlotDto[],
  targetFfId?: unknown
): Record<string, { code: string; message: string } | null> {
  const next = { ...prev };
  for (const s of slots) delete next[s.ffId];
  if (typeof targetFfId === "string" && targetFfId) delete next[targetFfId];
  return next;
}

function handleCompletedSse(event: ExecutionEventDto, uid: string, p: Record<string, unknown>, deps: CopilotSseHandlerDeps) {
  const slots = Array.isArray(p.slots) ? (p.slots as WordingSlotDto[]) : [];
  const newRevision = typeof p.newRevision === "number" ? p.newRevision : deps.getSnapshot(uid).revision + 1;
  const sha = typeof p.contentSha256 === "string" ? p.contentSha256 : "";
  const html = typeof p.sanitizedHtml === "string" ? p.sanitizedHtml : undefined;
  if (slots.length > 0) deps.applyCommit({ copilotJobId: event.jobId, targetSlideUid: uid, newRevision, contentSha256: sha, slots, sanitizedHtml: html });
  deps.setLeases((prev) => ({ ...prev, [uid]: createEmptyCopilotLease(uid) }));
  if (typeof p.activeGlobalSlots !== "number") deps.setGlobalSlots((prev) => Math.max(0, prev - 1));
  deps.setChartErrors((prev) => clearCommittedSlideChartErrors(prev, slots, p.targetFfId));
  const tr = deps.t ?? ((_k: string, f: string) => f);
  const fmtNum = deps.formatNumber ?? ((n: number) => String(n));
  const summary = typeof p.summary === "string" ? p.summary : tr("presentation.copilot.committedRev", "Committed rev {{rev}}").replace("{{rev}}", fmtNum(newRevision));
  deps.setMessages((m) => appendUniqueCopilotMessage(m, uid, { id: `evt-${event.id}`, role: "assistant", content: summary, timestampMs: Date.now(), statusTag: "COMPLETED" }));
  deps.setToast({ id: event.id, slideUid: uid, newRevision, patchedCount: slots.length, resolvedCommentId: typeof p.resolvedCommentId === "string" ? p.resolvedCommentId : null });
}

function handleErrorSse(opts: { event: ExecutionEventDto; uid: string; cat: string; p: Record<string, unknown>; deps: CopilotSseHandlerDeps }) {
  const { event, uid, cat, p, deps } = opts;
  const tr = deps.t ?? ((_k: string, f: string) => f);
  const fallbackCode = cat === "cancelled" ? "E_CANCELLED" : "E_COPILOT_FAILED";
  const code = typeof p.code === "string" ? p.code : (typeof p.reason === "string" ? p.reason : fallbackCode);
  const message = typeof p.message === "string" ? p.message : (code === "E_COPILOT_LEASE_TIMEOUT" ? tr("presentation.copilot.errors.leaseTimeout", "Slide Copilot lease expired after 60s.") : (event.payloadText || code));
  const targetFfId = typeof p.targetFfId === "string" ? p.targetFfId : undefined;
  const banner = cat === "failed" ? { code, message, targetFfId } : null;
  deps.setLeases((prev) => {
    const cur = prev[uid];
    if (cur?.leaseState === "active" && cur.jobId && event.jobId && cur.jobId !== event.jobId) return prev;
    return { ...prev, [uid]: { ...createEmptyCopilotLease(uid), errorBanner: banner } };
  });
  if (typeof p.activeGlobalSlots !== "number") deps.setGlobalSlots((prev) => Math.max(0, prev - 1));
  if (targetFfId && code.startsWith("E_CHART")) deps.setChartErrors((e) => ({ ...e, [targetFfId]: { code, message } }));
  deps.setMessages((m) => appendUniqueCopilotMessage(m, uid, { id: `evt-${event.id}`, role: "system", content: message, timestampMs: Date.now(), statusTag: cat.toUpperCase() }));
}

function mergeRunningOrQueuedLease(
  prev: Record<string, SlideCopilotLeaseState>,
  input: {
    uid: string;
    jobId: string;
    isRun: boolean;
    qp: number;
    exp: number;
    now: number;
    fencingToken?: number;
  }
): Record<string, SlideCopilotLeaseState> {
  const { uid, jobId, isRun, qp, exp, now, fencingToken } = input;
  const cur = prev[uid];
  if (!isRun && cur?.leaseState === "active" && cur.jobId && cur.jobId !== jobId) {
    return { ...prev, [uid]: { ...cur, queuePosition: Math.max(1, qp), queuedJobId: jobId } };
  }
  return {
    ...prev,
    [uid]: {
      ...(cur ?? createEmptyCopilotLease(uid)),
      slideUid: uid,
      jobId,
      leaseState: isRun ? "active" : "queued",
      fencingToken: fencingToken ?? cur?.fencingToken,
      queuePosition: isRun ? 0 : Math.max(1, qp),
      expiresAtMs: isRun ? exp : undefined,
      startedAtMs: !isRun ? (cur?.leaseState === "queued" && cur.startedAtMs ? cur.startedAtMs : now) : undefined,
      remainingSeconds: isRun ? computeLeaseRemainingSec(exp, now) : 0,
    },
  };
}

export function processCopilotSseEvent(event: ExecutionEventDto, deps: CopilotSseHandlerDeps) {
  const p = parseSseJson(event.payloadText);
  const uid = typeof p.slideUid === "string" && CANONICAL_SLIDE_UID_REGEX.test(p.slideUid) ? p.slideUid : event.nodeId;
  const cat = resolveEventCategory(event.eventKind, event.jobStatus);
  const now = Date.now();
  if (typeof p.activeGlobalSlots === "number") deps.setGlobalSlots(p.activeGlobalSlots);
  if (cat === "running" || cat === "queued") {
    const exp = typeof p.leaseExpiresAtMs === "number" ? p.leaseExpiresAtMs : now + SLIDE_COPILOT_LEASE_TTL_MS;
    const qp = typeof p.queuePosition === "number" ? p.queuePosition : 1;
    const fence = typeof p.fencingToken === "number" ? p.fencingToken : undefined;
    deps.setLeases((prev) => mergeRunningOrQueuedLease(prev, { uid, jobId: event.jobId, isRun: cat === "running", qp, exp, now, fencingToken: fence }));
    return;
  }
  if (cat === "completed") { handleCompletedSse(event, uid, p, deps); return; }
  if (cat === "cancelled" || cat === "failed") { handleErrorSse({ event, uid, cat, p, deps }); return; }
  if (typeof p.delta === "string" || typeof p.text === "string") {
    deps.setMessages((m) => appendUniqueCopilotMessage(m, uid, { id: `evt-${event.id}`, role: "assistant", content: String(p.delta ?? p.text), timestampMs: now }));
  }
}
