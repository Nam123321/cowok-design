// @story 06.3
// @implements AC-02, AC-03, AC-04 (Slide Copilot React Hook Controller)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { apiFlowExecution } from "../../../api/flow-execution";
import { apiPresentationCharts } from "../../../api/presentation-charts";
import type { StructuredCommentEnvelope } from "../../../lib/validations/element-comment";
import { CANONICAL_SLIDE_UID_REGEX, type WordingSlotDto } from "../../../lib/validations/presentation";
import type { ChartKind2D, ChartSpecV12 } from "../../../lib/validations/presentation-chart-specs";
import { processCopilotSseEvent } from "./slide-copilot-sse";
import { runChartValidation, runCopilotDispatch } from "./slide-copilot-actions";
import {
  COPILOT_FALLBACK_UID,
  COPILOT_IME_KEY_CODE,
  COPILOT_LEASE_GRACE_MS,
  COPILOT_LEASE_TICK_MS,
  COPILOT_MAX_SLOTS,
  COPILOT_PREFLUSH_TIMEOUT_MS,
  COPILOT_QUEUED_TIMEOUT_MS,
  COPILOT_TOAST_TTL_MS,
  appendUniqueCopilotMessage,
  computeLeaseRemainingSec,
  createEmptyCopilotLease,
  extractCopilotApiError,
  type SlideCopilotLeaseState,
  type SlideCopilotMessage,
  type SlideCopilotReceiptToast,
} from "./useSlideCopilotHelpers";

export type { SlideCopilotLeaseState, SlideCopilotMessage, SlideCopilotReceiptToast };

export interface UseSlideCopilotControllerParams {
  enabled: boolean;
  projectId?: string;
  deckId?: string;
  slideUid: string;
  isReadOnly: boolean;
  slots: WordingSlotDto[];
  flushSlideBeforeDispatch: (targetSlideUid?: string, drainAll?: boolean) => Promise<boolean>;
  getSlideCommitSnapshot: (targetUid?: string) => { revision: number; contentSha256: string };
  reloadServerRevision: () => void;
  applyCopilotBatchCommit: (args: {
    copilotJobId: string;
    targetSlideUid: string;
    newRevision: number;
    contentSha256: string;
    slots: WordingSlotDto[];
    sanitizedHtml?: string;
  }) => void;
}

function tickActiveLeases(
  prev: Record<string, SlideCopilotLeaseState>,
  now: number,
  t: (key: string, fallback: string) => string
): { next: Record<string, SlideCopilotLeaseState>; timedOut: Array<{ uid: string; message: string }>; changed: boolean } {
  let changed = false;
  const timedOut: Array<{ uid: string; message: string }> = [];
  const next: Record<string, SlideCopilotLeaseState> = {};
  for (const [uid, item] of Object.entries(prev)) {
    if (item.leaseState === "preflushing") {
      const started = item.startedAtMs ?? now;
      if (now >= started + COPILOT_PREFLUSH_TIMEOUT_MS) {
        const bannerMsg = t("presentation.copilot.errors.preflushTimeout", "CMS pre-flush timed out after 15s; slide unlocked.");
        changed = true;
        timedOut.push({ uid, message: `E_PREFLUSH_TIMEOUT: ${bannerMsg}` });
        next[uid] = { ...createEmptyCopilotLease(uid), errorBanner: { code: "E_PREFLUSH_TIMEOUT", message: bannerMsg, targetFfId: item.targetFfId } };
      } else {
        next[uid] = item;
      }
      continue;
    }
    if (item.leaseState === "queued") {
      const started = item.startedAtMs ?? now;
      if (now >= started + COPILOT_QUEUED_TIMEOUT_MS) {
        const bannerMsg = t("presentation.copilot.errors.queueTimeout", "Slide Copilot queue wait timed out after 120s; slide unlocked.");
        changed = true;
        timedOut.push({ uid, message: `E_COPILOT_QUEUE_TIMEOUT: ${bannerMsg}` });
        next[uid] = { ...createEmptyCopilotLease(uid), errorBanner: { code: "E_COPILOT_QUEUE_TIMEOUT", message: bannerMsg, targetFfId: item.targetFfId } };
      } else {
        next[uid] = item;
      }
      continue;
    }
    if (item.leaseState !== "active" || !item.expiresAtMs) { next[uid] = item; continue; }
    if (now >= item.expiresAtMs + COPILOT_LEASE_GRACE_MS) {
      const bannerMsg = t("presentation.copilot.errors.leaseTimeout", "Slide Copilot lease expired after 60s.");
      changed = true;
      timedOut.push({ uid, message: `E_COPILOT_LEASE_TIMEOUT: ${bannerMsg}` });
      next[uid] = { ...createEmptyCopilotLease(uid), errorBanner: { code: "E_COPILOT_LEASE_TIMEOUT", message: bannerMsg, targetFfId: item.targetFfId } };
    } else {
      const sec = computeLeaseRemainingSec(item.expiresAtMs, now);
      if (sec !== item.remainingSeconds) changed = true;
      next[uid] = { ...item, remainingSeconds: sec };
    }
  }
  return { next, timedOut, changed };
}

function useCopilotLeaseCountdown(
  leases: Record<string, SlideCopilotLeaseState>,
  setLeases: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotLeaseState>>>,
  setMessages: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotMessage[]>>>,
  t: (key: string, fallback: string) => string
) {
  const leasesRef = useRef(leases);
  leasesRef.current = leases;
  const hasWatchedLease = Object.values(leases).some(
    (l) => (l.leaseState === "active" && Boolean(l.expiresAtMs)) || l.leaseState === "preflushing" || l.leaseState === "queued"
  );
  useEffect(() => {
    if (!hasWatchedLease) return;
    const timer = window.setInterval(() => {
      const now = Date.now();
      const res = tickActiveLeases(leasesRef.current, now, t);
      if (res.changed) setLeases((prev) => tickActiveLeases(prev, now, t).next);
      for (const item of res.timedOut) {
        setMessages((m) => appendUniqueCopilotMessage(m, item.uid, { id: `timeout-${item.uid}-${now}`, role: "system", content: item.message, timestampMs: now, statusTag: "TIMEOUT" }));
      }
    }, COPILOT_LEASE_TICK_MS);
    return () => window.clearInterval(timer);
  }, [hasWatchedLease, setLeases, setMessages, t]);
}

function useCopilotSseSubscription(opts: {
  enabled: boolean;
  projectId?: string;
  deckId?: string;
  leases: Record<string, SlideCopilotLeaseState>;
  paramsRef: React.MutableRefObject<UseSlideCopilotControllerParams>;
  setLeases: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotLeaseState>>>;
  setMessages: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotMessage[]>>>;
  setChartErrors: React.Dispatch<React.SetStateAction<Record<string, { code: string; message: string } | null>>>;
  setToast: React.Dispatch<React.SetStateAction<SlideCopilotReceiptToast | null>>;
  setGlobalSlots: React.Dispatch<React.SetStateAction<number>>;
  t: (key: string, fallback: string) => string;
  formatNumber: (n: number) => string;
}) {
  const hasLiveLease = Object.values(opts.leases).some((l) => l.leaseState === "active" || l.leaseState === "queued");
  const lastSeqRef = useRef(0);
  useEffect(() => {
    if (!opts.enabled || !opts.projectId || !opts.deckId || opts.deckId === "local-deck" || !hasLiveLease) return;
    const controller = new AbortController();
    void apiFlowExecution
      .streamJobEvents(opts.projectId, {
        afterSeq: lastSeqRef.current,
        signal: controller.signal,
        onEvents: (events) => {
          for (const ev of events) {
            if (typeof ev.projectSeq === "number" && ev.projectSeq > lastSeqRef.current) lastSeqRef.current = ev.projectSeq;
            processCopilotSseEvent(ev, { getSnapshot: opts.paramsRef.current.getSlideCommitSnapshot, applyCommit: opts.paramsRef.current.applyCopilotBatchCommit, setLeases: opts.setLeases, setMessages: opts.setMessages, setChartErrors: opts.setChartErrors, setToast: opts.setToast, setGlobalSlots: opts.setGlobalSlots, t: opts.t, formatNumber: opts.formatNumber });
          }
        },
      })
      .catch((err) => {
        if (controller.signal.aborted) return;
        const msg = err instanceof Error && err.message ? err.message : opts.t("presentation.copilot.errors.streamDisconnected", "Slide Copilot event stream disconnected.");
        const uid = opts.paramsRef.current.slideUid;
        opts.setLeases((prev) => {
          const cur = prev[uid];
          if (!cur || (cur.leaseState !== "active" && cur.leaseState !== "queued")) return prev;
          return { ...prev, [uid]: { ...cur, errorBanner: { code: "E_COPILOT_STREAM_DISCONNECTED", message: msg, targetFfId: cur.targetFfId } } };
        });
      });
    return () => controller.abort();
  }, [hasLiveLease, opts.deckId, opts.enabled, opts.paramsRef, opts.projectId, opts.setChartErrors, opts.setGlobalSlots, opts.setLeases, opts.setMessages, opts.setToast]);
}

function useCopilotWindowEvents(opts: {
  enabled: boolean; isReadOnly: boolean; isOpen: boolean; slideUid: string;
  setIsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setIsCollapsed: React.Dispatch<React.SetStateAction<boolean>>;
  setTargetFfId: React.Dispatch<React.SetStateAction<string | null>>;
  setComment: React.Dispatch<React.SetStateAction<StructuredCommentEnvelope | null>>;
  setDraft: React.Dispatch<React.SetStateAction<string>>;
}) {
  const ref = useRef(opts);
  ref.current = opts;
  useEffect(() => {
    if (!ref.current.enabled || typeof window === "undefined") return;
    const onComment = (ev: Event) => {
      const d = (ev as CustomEvent<StructuredCommentEnvelope>).detail;
      if (!d || (d.slideUid && d.slideUid !== ref.current.slideUid)) return;
      ref.current.setTargetFfId(d.elementFfId);
      ref.current.setComment(d);
    };
    const onAssist = (ev: Event) => {
      const d = (ev as CustomEvent<{ targetFfId?: string; prompt?: string }>).detail;
      if (d?.targetFfId) ref.current.setTargetFfId(d.targetFfId);
      if (d?.prompt) ref.current.setDraft(d.prompt);
      ref.current.setIsOpen(true);
      ref.current.setIsCollapsed(false);
    };
    const onKey = (ev: KeyboardEvent) => {
      if (ev.isComposing || ev.keyCode === COPILOT_IME_KEY_CODE || ref.current.isReadOnly) return;
      if (ev.altKey && ev.key === "/") { ev.preventDefault(); ref.current.setIsOpen((p) => !p); ref.current.setIsCollapsed(false); return; }
      const activeEl = typeof document !== "undefined" ? document.activeElement : null;
      if (ev.key === "Escape" && ref.current.isOpen && activeEl?.closest?.("[data-testid='floating-slide-copilot']")) {
        ev.preventDefault(); ref.current.setIsCollapsed(true);
      }
    };
    window.addEventListener("cowok:structured-comment", onComment);
    window.addEventListener("cowok:ai-assist", onAssist);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("cowok:structured-comment", onComment);
      window.removeEventListener("cowok:ai-assist", onAssist);
      window.removeEventListener("keydown", onKey);
    };
  }, [opts.enabled, opts.slideUid]);
}

function useSlideCopilotStateStore(
  params: UseSlideCopilotControllerParams,
  t: (key: string, fallback: string) => string,
  formatNumber: (n: number) => string
) {
  const [isOpen, setIsOpen] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [targetFfId, setTargetFfId] = useState<string | null>(null);
  const [draftPrompt, setDraftPrompt] = useState("");
  const [attachedComment, setAttachedComment] = useState<StructuredCommentEnvelope | null>(null);
  const [leasesBySlide, setLeasesBySlide] = useState<Record<string, SlideCopilotLeaseState>>({});
  const [messagesBySlide, setMessagesBySlide] = useState<Record<string, SlideCopilotMessage[]>>({});
  const [chartErrorByFfId, setChartErrorByFfId] = useState<Record<string, { code: string; message: string } | null>>({});
  const [receiptToast, setReceiptToast] = useState<SlideCopilotReceiptToast | null>(null);
  const [activeGlobalSlots, setActiveGlobalSlots] = useState(0);
  const paramsRef = useRef(params);
  paramsRef.current = params;
  useEffect(() => {
    if (!receiptToast) return;
    const timer = window.setTimeout(() => setReceiptToast(null), COPILOT_TOAST_TTL_MS);
    return () => window.clearTimeout(timer);
  }, [receiptToast]);
  useCopilotLeaseCountdown(leasesBySlide, setLeasesBySlide, setMessagesBySlide, t);
  useCopilotSseSubscription({ enabled: params.enabled, projectId: params.projectId, deckId: params.deckId, leases: leasesBySlide, paramsRef, setLeases: setLeasesBySlide, setMessages: setMessagesBySlide, setChartErrors: setChartErrorByFfId, setToast: setReceiptToast, setGlobalSlots: setActiveGlobalSlots, t, formatNumber });
  useCopilotWindowEvents({ enabled: params.enabled, isReadOnly: params.isReadOnly, isOpen, slideUid: params.slideUid, setIsOpen, setIsCollapsed, setTargetFfId, setComment: setAttachedComment, setDraft: setDraftPrompt });
  return { paramsRef, isOpen, setIsOpen, isCollapsed, setIsCollapsed, targetFfId, setTargetFfId, draftPrompt, setDraftPrompt, attachedComment, setAttachedComment, leasesBySlide, setLeasesBySlide, messagesBySlide, setMessagesBySlide, chartErrorByFfId, setChartErrorByFfId, receiptToast, setReceiptToast, activeGlobalSlots, setActiveGlobalSlots };
}

async function executeCancelCopilotLease(
  p: UseSlideCopilotControllerParams,
  curSt: ReturnType<typeof useSlideCopilotStateStore>,
  t: (key: string, fallback: string) => string,
  targetUid?: string
): Promise<void> {
  const uid = targetUid || p.slideUid;
  const lease = curSt.leasesBySlide[uid];
  if (!lease?.jobId || !p.projectId || !p.deckId || p.deckId === "local-deck") {
    curSt.setLeasesBySlide((prev) => ({ ...prev, [uid]: createEmptyCopilotLease(uid) }));
    return;
  }
  try {
    await apiPresentationCharts.cancelSlideCopilot(
      { projectId: p.projectId, deckId: p.deckId, slideUid: uid },
      { jobId: lease.jobId, expectedFencingToken: lease.fencingToken ?? 1, reason: "user_cancelled" }
    );
    curSt.setLeasesBySlide((prev) => ({ ...prev, [uid]: createEmptyCopilotLease(uid) }));
    curSt.setMessagesBySlide((m) =>
      appendUniqueCopilotMessage(m, uid, { id: `cancel-${Date.now()}`, role: "system", content: t("presentation.copilot.cancelledByUser", "Cancelled by user."), timestampMs: Date.now(), statusTag: "CANCELLED" })
    );
  } catch (err) {
    const e = extractCopilotApiError(err);
    curSt.setLeasesBySlide((prev) => ({
      ...prev,
      [uid]: { ...(prev[uid] ?? createEmptyCopilotLease(uid)), errorBanner: { code: e.code, message: e.message } },
    }));
  }
}

export function useSlideCopilotController(params: UseSlideCopilotControllerParams) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const st = useSlideCopilotStateStore(params, t, formatNumber);
  const { paramsRef } = st;
  const stRef = useRef(st);
  stRef.current = st;
  const submitPrompt = useCallback(async (promptText: string, kind?: ChartKind2D) => {
    const p = paramsRef.current;
    const curSt = stRef.current;
    const trimmed = promptText.trim();
    if (!trimmed || p.isReadOnly) return;
    const uid = CANONICAL_SLIDE_UID_REGEX.test(p.slideUid) ? p.slideUid : COPILOT_FALLBACK_UID;
    const target = curSt.targetFfId?.trim() || p.slots.find((s) => s.slotKind === "chart_json")?.ffId || p.slots[0]?.ffId || "ff-slide-headline";
    curSt.setMessagesBySlide((m) => appendUniqueCopilotMessage(m, uid, { id: `usr-${Date.now()}`, role: "user", content: trimmed, targetFfId: target, timestampMs: Date.now() }));
    curSt.setLeasesBySlide((prev) => {
      const cur = prev[uid];
      if (cur?.leaseState === "active" && cur.jobId) return { ...prev, [uid]: { ...cur, errorBanner: null } };
      return { ...prev, [uid]: { ...(cur ?? createEmptyCopilotLease(uid)), leaseState: "preflushing", targetFfId: target, startedAtMs: Date.now(), errorBanner: null } };
    });
    const ok = await runCopilotDispatch({ p, uid, target, prompt: trimmed, kind, comment: curSt.attachedComment, setLeases: curSt.setLeasesBySlide, setMessages: curSt.setMessagesBySlide, setComment: curSt.setAttachedComment, setGlobalSlots: curSt.setActiveGlobalSlots, t });
    if (ok) curSt.setDraftPrompt("");
  }, [paramsRef, t]);
  const cancelCopilot = useCallback((targetUid?: string) => executeCancelCopilotLease(paramsRef.current, stRef.current, t, targetUid), [paramsRef, t]);
  const validateChartSpec = useCallback((ffId: string, spec: ChartSpecV12) => runChartValidation(paramsRef.current, ffId, spec, stRef.current.setChartErrorByFfId), [paramsRef]);
  const openCopilot = useCallback((opts?: { targetFfId?: string; presetPrompt?: string }) => {
    const curSt = stRef.current;
    if (opts?.targetFfId) curSt.setTargetFfId(opts.targetFfId);
    if (opts?.presetPrompt) curSt.setDraftPrompt(opts.presetPrompt);
    curSt.setIsOpen(true);
    curSt.setIsCollapsed(false);
  }, []);
  const retryChartWithCopilot = useCallback(
    (ffId: string, errors?: string[]) => {
      const base = t("presentation.copilot.fixChartPrompt", "Fix 2D chart schema for #{{ffId}}").replace("{{ffId}}", ffId);
      openCopilot({ targetFfId: ffId, presetPrompt: errors?.length ? `${base}: ${errors.join("; ")}` : base });
    },
    [openCopilot, t]
  );
  const activeLease = useMemo(() => st.leasesBySlide[params.slideUid] ?? createEmptyCopilotLease(params.slideUid), [st.leasesBySlide, params.slideUid]);
  return {
    isOpen: st.isOpen, isCollapsed: st.isCollapsed, targetFfId: st.targetFfId, draftPrompt: st.draftPrompt, attachedComment: st.attachedComment, activeLease, leasesBySlide: st.leasesBySlide,
    messages: st.messagesBySlide[params.slideUid] ?? [], chartErrorByFfId: st.chartErrorByFfId, receiptToast: st.receiptToast,
    activeGlobalSlots: Math.max(st.activeGlobalSlots, Object.values(st.leasesBySlide).filter((l) => l.leaseState === "active").length),
    maxGlobalSlots: COPILOT_MAX_SLOTS, setIsOpen: st.setIsOpen, setIsCollapsed: st.setIsCollapsed, setTargetFfId: st.setTargetFfId, setDraftPrompt: st.setDraftPrompt,
    clearAttachedComment: () => st.setAttachedComment(null), dismissReceiptToast: () => st.setReceiptToast(null), openCopilot, retryChartWithCopilot, submitPrompt, cancelCopilot, validateChartSpec,
  };
}
