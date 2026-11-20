// @story 06.2
// @implements AC-02, AC-04, AC-06
import { useCallback, useEffect, useRef } from "react";
import { apiPresentation } from "../../../api/presentation";
import { clearRecoveryDraft } from "../utils/cms-draft-storage";
import type { PendingSlideBuffer, SlideCmsSlice } from "./useCmsSlideSupportHooks";

export const CMS_FLUSH_DEBOUNCE_MS = 200;

export interface BatchFlusherOpts {
  projectId?: string;
  deckId: string;
  slideUid: string;
  isReadOnly: boolean;
  isRemoteBacked: boolean;
  deckModeRef: React.MutableRefObject<"multi-page" | "one-page">;
  isComposingRef: React.MutableRefObject<boolean>;
  revisionBySlideRef: React.MutableRefObject<Map<string, number>>;
  contentSha256BySlideRef?: React.MutableRefObject<Map<string, string>>;
  pendingBySlideRef: React.MutableRefObject<Map<string, PendingSlideBuffer>>;
  updateSlideSlice: (targetUid: string, updater: (prev: SlideCmsSlice) => SlideCmsSlice) => void;
  onDiscardReload?: (dirtyUids: string[]) => void;
}

export function syncCommittedRevision(
  revisionBySlide: Map<string, number>,
  deckMode: "multi-page" | "one-page",
  targetSlideUid: string,
  newRevision: number
) {
  revisionBySlide.set(targetSlideUid, newRevision);
  if (deckMode === "one-page") {
    for (const uid of revisionBySlide.keys()) {
      revisionBySlide.set(uid, newRevision);
    }
  }
}

export function syncCommittedContentSha(
  shaBySlide: Map<string, string> | undefined,
  deckMode: "multi-page" | "one-page",
  targetSlideUid: string,
  contentSha256: string | undefined
) {
  if (!shaBySlide || !contentSha256) return;
  shaBySlide.set(targetSlideUid, contentSha256);
  if (deckMode === "one-page") {
    for (const uid of shaBySlide.keys()) {
      shaBySlide.set(uid, contentSha256);
    }
  }
}

function pruneCommittedBuffer(
  pendingBySlide: Map<string, PendingSlideBuffer>,
  targetSlideUid: string,
  sentPatchSeqs: Array<[string, number]>,
  sentNotesSeq: number | undefined
): boolean {
  const buf = pendingBySlide.get(targetSlideUid);
  if (!buf) return false;
  for (const [ffId, sentSeq] of sentPatchSeqs) {
    const cur = buf.patches.get(ffId);
    if (cur && cur.editSeq <= sentSeq) buf.patches.delete(ffId);
  }
  if (sentNotesSeq !== undefined && buf.notes && buf.notes.editSeq <= sentNotesSeq) {
    buf.notes = undefined;
  }
  const hasRemaining = buf.patches.size > 0 || buf.notes !== undefined;
  if (!hasRemaining) pendingBySlide.delete(targetSlideUid);
  return hasRemaining;
}

function commitLocalSlideBatch(args: {
  cur: BatchFlusherOpts;
  targetSlideUid: string;
  sentPatchSeqs: Array<[string, number]>;
  sentNotesSeq: number | undefined;
  expectedRevision: number;
  isMountedRef?: React.MutableRefObject<boolean>;
}) {
  const { cur, targetSlideUid, sentPatchSeqs, sentNotesSeq, expectedRevision, isMountedRef } = args;
  const hasRemaining = pruneCommittedBuffer(cur.pendingBySlideRef.current, targetSlideUid, sentPatchSeqs, sentNotesSeq);
  if (!hasRemaining && (!isMountedRef || isMountedRef.current)) clearRecoveryDraft(cur.deckId, targetSlideUid);
  const nextRev = expectedRevision + 1;
  syncCommittedRevision(cur.revisionBySlideRef.current, cur.deckModeRef.current, targetSlideUid, nextRev);
  cur.updateSlideSlice(targetSlideUid, (p) => ({ ...p, revision: nextRev, syncState: hasRemaining ? "DIRTY_FAST_PATCHED" : "SYNCED", lastFlushError: null }));
  return { ok: true, hasRemaining };
}

function buildSlideBatchIdemKey(opts: { deckId: string; slideUid: string; rev: number; notesSeq?: number; patchSeqs?: Array<[string, number]> }): string {
  const seqs = opts.patchSeqs ? opts.patchSeqs.map(([, s]) => s) : [];
  const maxSeq = Math.max(opts.notesSeq ?? 0, ...seqs, 0);
  return `cms-${opts.deckId}-${opts.slideUid}-r${opts.rev}-s${maxSeq}`;
}

async function executeSlideBatchCommit(
  optsRef: React.MutableRefObject<BatchFlusherOpts>,
  targetSlideUid: string,
  isMountedRef?: React.MutableRefObject<boolean>
): Promise<{ ok: boolean; hasRemaining: boolean }> {
  const cur = optsRef.current;
  const buf = cur.pendingBySlideRef.current.get(targetSlideUid);
  if (!buf || (buf.patches.size === 0 && buf.notes === undefined)) return { ok: true, hasRemaining: false };
  const sentEntries = Array.from(buf.patches.entries());
  const patches = sentEntries.map(([, v]) => v.item);
  const sentPatchSeqs: Array<[string, number]> = sentEntries.map(([k, v]) => [k, v.editSeq]);
  const sentNotes = buf.notes?.value;
  const sentNotesSeq = buf.notes?.editSeq;
  const expectedRevision = cur.revisionBySlideRef.current.get(targetSlideUid) ?? 1;

  if (!cur.isRemoteBacked || !cur.projectId) {
    return commitLocalSlideBatch({ cur, targetSlideUid, sentPatchSeqs, sentNotesSeq, expectedRevision, isMountedRef });
  }
  cur.updateSlideSlice(targetSlideUid, (p) => ({ ...p, syncState: "COMMITTING" }));
  const idemKey = buildSlideBatchIdemKey({ deckId: cur.deckId, slideUid: targetSlideUid, rev: expectedRevision, notesSeq: sentNotesSeq, patchSeqs: sentPatchSeqs });
  try {
    const res = await apiPresentation.commitSlotBatch({ projectId: cur.projectId, deckId: cur.deckId, slideUid: targetSlideUid }, { expectedRevision, speakerNotes: sentNotes, patches }, { idempotencyKey: idemKey });
    if (isMountedRef && !isMountedRef.current) return { ok: false, hasRemaining: true };
    const latest = optsRef.current;
    const hasRemaining = pruneCommittedBuffer(latest.pendingBySlideRef.current, targetSlideUid, sentPatchSeqs, sentNotesSeq);
    if (!hasRemaining) clearRecoveryDraft(latest.deckId, targetSlideUid);
    syncCommittedRevision(latest.revisionBySlideRef.current, latest.deckModeRef.current, targetSlideUid, res.newRevision);
    syncCommittedContentSha(latest.contentSha256BySlideRef?.current, latest.deckModeRef.current, targetSlideUid, res.contentSha256);
    const keepInputsUntouched = latest.isComposingRef.current && latest.slideUid === targetSlideUid;
    latest.updateSlideSlice(targetSlideUid, (p) => ({ ...p, revision: res.newRevision, contentSha256: res.contentSha256, slots: keepInputsUntouched ? p.slots : res.slots, syncState: hasRemaining ? "DIRTY_FAST_PATCHED" : "SYNCED", lastFlushError: null }));
    return { ok: true, hasRemaining };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "E_FLUSH_FAILED";
    optsRef.current.updateSlideSlice(targetSlideUid, (p) => ({ ...p, syncState: "FLUSH_FAILED_RETRY", lastFlushError: msg }));
    return { ok: false, hasRemaining: false };
  }
}

function scheduleSlideTimer(
  timers: Map<string, ReturnType<typeof setTimeout>>,
  uid: string,
  fn: () => unknown
) {
  const existing = timers.get(uid);
  if (existing) clearTimeout(existing);
  timers.set(uid, setTimeout(() => { timers.delete(uid); fn(); }, CMS_FLUSH_DEBOUNCE_MS));
}

async function awaitPriorSlideFlush(deps: {
  uid: string;
  drainAll: boolean;
  inFlightPromises: Map<string, Promise<boolean>>;
  queued: Set<string>;
  isMountedRef?: React.MutableRefObject<boolean>;
}): Promise<boolean | null> {
  const existing = deps.inFlightPromises.get(deps.uid);
  if (!existing) return null;
  deps.queued.add(deps.uid);
  const prevOk = await existing;
  if (!deps.drainAll || !prevOk || (deps.isMountedRef && !deps.isMountedRef.current)) return prevOk;
  return null;
}

async function runDrainableSlideFlush(deps: {
  uid: string;
  optsRef: React.MutableRefObject<BatchFlusherOpts>;
  inFlightPromises: Map<string, Promise<boolean>>;
  queued: Set<string>;
  drainAll: boolean;
  isMountedRef?: React.MutableRefObject<boolean>;
}): Promise<boolean> {
  const { uid, optsRef, inFlightPromises, queued, drainAll, isMountedRef } = deps;
  if (optsRef.current.isReadOnly) return true;
  if (isMountedRef && !isMountedRef.current) return false;
  const prior = await awaitPriorSlideFlush(deps);
  if (prior !== null) return prior;
  const runPromise = (async () => {
    let ok = true;
    while (ok) {
      if (isMountedRef && !isMountedRef.current) return false;
      const outcome = await executeSlideBatchCommit(optsRef, uid, isMountedRef);
      ok = outcome.ok;
      const more = outcome.hasRemaining || queued.delete(uid);
      if (!more || !drainAll) {
        if (more) queued.add(uid);
        break;
      }
    }
    return ok;
  })();
  inFlightPromises.set(uid, runPromise);
  try {
    return await runPromise;
  } finally {
    if (inFlightPromises.get(uid) === runPromise) inFlightPromises.delete(uid);
  }
}

function discardPendingSlideBuffers(
  timers: Map<string, ReturnType<typeof setTimeout>>,
  queued: Set<string>,
  cur: BatchFlusherOpts
) {
  timers.forEach((tm) => clearTimeout(tm));
  timers.clear();
  queued.clear();
  const dirtyUids = Array.from(cur.pendingBySlideRef.current.keys());
  cur.pendingBySlideRef.current.clear();
  for (const uid of dirtyUids) {
    clearRecoveryDraft(cur.deckId, uid);
    cur.updateSlideSlice(uid, (p) => ({ ...p, syncState: "SYNCED", lastFlushError: null }));
  }
  cur.onDiscardReload?.(dirtyUids);
}

function useFlusherMountLifecycle(args: {
  slideUid: string;
  optsRef: React.MutableRefObject<BatchFlusherOpts>;
  timersBySlideRef: React.MutableRefObject<Map<string, ReturnType<typeof setTimeout>>>;
  isMountedRef: React.MutableRefObject<boolean>;
  flushSlideNow: (targetSlideUid?: string, drainAll?: boolean) => Promise<boolean>;
}) {
  const { slideUid, optsRef, timersBySlideRef, isMountedRef, flushSlideNow } = args;
  const prevSlideUidRef = useRef<string>(slideUid);
  useEffect(() => {
    const departing = prevSlideUidRef.current;
    prevSlideUidRef.current = slideUid;
    if (departing && departing !== slideUid && optsRef.current.pendingBySlideRef.current.has(departing)) {
      flushSlideNow(departing, false).catch(() => {});
    }
  }, [flushSlideNow, optsRef, slideUid]);

  useEffect(() => {
    isMountedRef.current = true;
    const timers = timersBySlideRef.current;
    return () => {
      isMountedRef.current = false;
      timers.forEach((tm) => clearTimeout(tm));
      timers.clear();
    };
  }, [isMountedRef, timersBySlideRef]);
}

export function useCmsBatchFlusher(opts: BatchFlusherOpts) {
  const timersBySlideRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const inFlightPromisesRef = useRef<Map<string, Promise<boolean>>>(new Map());
  const queuedBySlideRef = useRef<Set<string>>(new Set());
  const isMountedRef = useRef<boolean>(true);
  const optsRef = useRef(opts);
  optsRef.current = opts;

  const flushSlideNow = useCallback(async (targetSlideUid?: string, drainAll = false): Promise<boolean> => {
    if (!isMountedRef.current) return false;
    const uid = targetSlideUid || optsRef.current.slideUid;
    const ok = await runDrainableSlideFlush({ uid, optsRef, inFlightPromises: inFlightPromisesRef.current, queued: queuedBySlideRef.current, drainAll, isMountedRef });
    if (ok && isMountedRef.current && queuedBySlideRef.current.delete(uid)) {
      scheduleSlideTimer(timersBySlideRef.current, uid, () => flushSlideNow(uid, false));
    }
    return ok;
  }, []);

  const scheduleDebouncedFlush = useCallback((targetSlideUid?: string) => {
    if (!isMountedRef.current) return;
    scheduleSlideTimer(timersBySlideRef.current, targetSlideUid || optsRef.current.slideUid, () => flushSlideNow(targetSlideUid || optsRef.current.slideUid, false));
  }, [flushSlideNow]);

  useFlusherMountLifecycle({ slideUid: opts.slideUid, optsRef, timersBySlideRef, isMountedRef, flushSlideNow });

  const flushAllPendingNow = useCallback(async (): Promise<boolean> => {
    const uids = new Set<string>([...optsRef.current.pendingBySlideRef.current.keys(), ...inFlightPromisesRef.current.keys()]);
    for (const uid of uids) {
      const tm = timersBySlideRef.current.get(uid);
      if (tm) { clearTimeout(tm); timersBySlideRef.current.delete(uid); }
      if (!(await flushSlideNow(uid, true))) return false;
    }
    return true;
  }, [flushSlideNow]);

  const discardAllPendingNow = useCallback(() => {
    discardPendingSlideBuffers(timersBySlideRef.current, queuedBySlideRef.current, optsRef.current);
  }, []);

  return { flushPendingBatchNow: flushSlideNow, flushAllPendingNow, discardAllPendingNow, scheduleDebouncedFlush };
}
