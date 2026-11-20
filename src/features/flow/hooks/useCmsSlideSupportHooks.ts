// @story 06.2
// @implements AC-03, AC-06
import { useCallback, useEffect, useRef, useState } from "react";
import { apiPresentation } from "../../../api/presentation";
import type {
  IframeToHostDeckMessage,
  PatchSlotItem,
  WordingSlotDto,
} from "../../../lib/validations/presentation";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import {
  E3_DANGER_OVERFLOW_PX,
  IME_COMPOSING_KEY_CODE,
} from "../utils/cms-draft-storage";
import {
  CMS_FLUSH_DEBOUNCE_MS,
  syncCommittedContentSha,
  syncCommittedRevision,
  useCmsBatchFlusher,
  type BatchFlusherOpts,
} from "./useCmsBatchFlusher";
import type { useCmsEditMutations } from "./useCmsEditMutations";

export { CMS_FLUSH_DEBOUNCE_MS, syncCommittedRevision, useCmsBatchFlusher, type BatchFlusherOpts };

export const MAX_UNDO_HISTORY = 50;

export type CmsSyncState =
  | "SYNCED"
  | "DIRTY_FAST_PATCHED"
  | "COMMITTING"
  | "FLUSH_FAILED_RETRY";

export interface CopilotAtomicUndoBatch {
  copilotJobId: string;
  previousSanitizedHtml?: string;
  nextSanitizedHtml?: string;
  patches: Array<{
    ffId: string;
    fieldKey: keyof SlidePhaseItem | "slot";
    slotKind: PatchSlotItem["slotKind"];
    previousValue: string;
    nextValue: string;
  }>;
}

export interface CmsUndoEntry {
  slideId: string;
  ffId: string;
  fieldKey: keyof SlidePhaseItem | "speakerNotes" | "slot";
  slotKind?: PatchSlotItem["slotKind"];
  previousValue: string;
  nextValue: string;
  timestampMs: number;
  copilotAtomicBatch?: CopilotAtomicUndoBatch;
}

export interface CmsOverflowAlert {
  slideUid?: string;
  ffId?: string;
  scrollHeight: number;
  clientHeight: number;
  overflowPx: number;
  severity: "warning" | "danger";
}

export interface PendingSlideBuffer {
  patches: Map<string, { item: PatchSlotItem; editSeq: number }>;
  notes?: { value: string; editSeq: number };
}

export interface SlideCmsSlice {
  revision: number;
  contentSha256: string;
  slots: WordingSlotDto[];
  customHtmlOverride: boolean;
  unmappedSlotIds: string[];
  syncState: CmsSyncState;
  lastFlushError: string | null;
  isHydrating: boolean;
}

export function createDefaultSlideSlice(): SlideCmsSlice {
  return {
    revision: 1,
    contentSha256: "",
    slots: [],
    customHtmlOverride: false,
    unmappedSlotIds: [],
    syncState: "SYNCED",
    lastFlushError: null,
    isHydrating: false,
  };
}

export function useCmsDirtyBeforeUnload(hasDirty: boolean) {
  useEffect(() => {
    if (typeof window === "undefined" || !hasDirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [hasDirty]);
}

export interface RemoteHydrationOpts {
  isRemoteBacked: boolean;
  projectId?: string;
  deckId: string;
  slideUid: string;
  activeSlideId: string;
  reloadToken: number;
  pendingBySlideRef: React.MutableRefObject<Map<string, PendingSlideBuffer>>;
  isComposingRef: React.MutableRefObject<boolean>;
  onHydrationStart: (targetUid: string) => void;
  onHydrated: (
    targetUid: string,
    targetSlideId: string,
    snap: {
      deckMode?: "multi-page" | "one-page";
      revision: number;
      contentSha256?: string;
      slots: WordingSlotDto[];
      customHtmlOverride: boolean;
      unmappedSlotIds: string[];
      speakerNotes: string;
      sanitizedHtml?: string;
    }
  ) => void;
  onHydrationDone: (targetUid: string) => void;
  onHydrationError?: (targetUid: string, errorMessage: string) => void;
}

export function useCmsRemoteHydration(opts: RemoteHydrationOpts) {
  const cbRef = useRef(opts);
  cbRef.current = opts;
  useEffect(() => {
    if (!opts.isRemoteBacked || !opts.projectId) return;
    let cancelled = false;
    const targetUid = opts.slideUid;
    const targetSlideId = opts.activeSlideId;
    cbRef.current.onHydrationStart(targetUid);
    apiPresentation
      .listSlideSlots({ projectId: opts.projectId, deckId: opts.deckId, slideUid: targetUid })
      .then((snap) => {
        if (cancelled || opts.pendingBySlideRef.current.has(targetUid) || opts.isComposingRef.current) return;
        cbRef.current.onHydrated(targetUid, targetSlideId, snap);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const msg = err instanceof Error && err.message ? err.message : "E_CMS_SLOTS_HYDRATION_FAILED";
        cbRef.current.onHydrationError?.(targetUid, msg);
      })
      .finally(() => { if (!cancelled) cbRef.current.onHydrationDone(targetUid); });
    return () => { cancelled = true; };
  }, [opts.activeSlideId, opts.deckId, opts.isComposingRef, opts.isRemoteBacked, opts.pendingBySlideRef, opts.projectId, opts.reloadToken, opts.slideUid]);
}

function useUndoKeyboardShortcut(
  isDrawerOpen: boolean,
  hasHistory: boolean,
  undo: () => void,
  redo: () => void
) {
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onKeyDown = (ev: KeyboardEvent) => {
      if (ev.isComposing || ev.keyCode === IME_COMPOSING_KEY_CODE) return;
      if (!(ev.metaKey || ev.ctrlKey) || ev.key.toLowerCase() !== "z") return;
      if (!isDrawerOpen && !hasHistory) return;
      ev.preventDefault();
      if (ev.shiftKey) redo();
      else undo();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [hasHistory, isDrawerOpen, redo, undo]);
}

export function useCmsUndoRedoHistory(opts: {
  isDrawerOpen: boolean;
  isReadOnly: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
  onReplayFieldRef: React.MutableRefObject<(entry: CmsUndoEntry, direction: "undo" | "redo") => void>;
}) {
  const { isDrawerOpen, isReadOnly, isComposingRef, onReplayFieldRef } = opts;
  const [undoStack, setUndoStack] = useState<CmsUndoEntry[]>([]);
  const [redoStack, setRedoStack] = useState<CmsUndoEntry[]>([]);
  const undoRef = useRef<CmsUndoEntry[]>([]);
  const redoRef = useRef<CmsUndoEntry[]>([]);
  undoRef.current = undoStack;
  redoRef.current = redoStack;

  const pushUndoEntry = useCallback((entry: CmsUndoEntry) => {
    const jobId = entry.copilotAtomicBatch?.copilotJobId;
    if (jobId && undoRef.current.some((u) => u.copilotAtomicBatch?.copilotJobId === jobId)) return;
    const next = [...undoRef.current, entry];
    const trimmed = next.length > MAX_UNDO_HISTORY ? next.slice(next.length - MAX_UNDO_HISTORY) : next;
    undoRef.current = trimmed;
    redoRef.current = [];
    setUndoStack(trimmed);
    setRedoStack([]);
  }, []);

  const undo = useCallback(() => {
    if (isReadOnly || isComposingRef.current || undoRef.current.length === 0) return;
    const last = undoRef.current[undoRef.current.length - 1];
    undoRef.current = undoRef.current.slice(0, -1);
    redoRef.current = [...redoRef.current.slice(-(MAX_UNDO_HISTORY - 1)), last];
    setUndoStack(undoRef.current);
    setRedoStack(redoRef.current);
    onReplayFieldRef.current(last, "undo");
  }, [isComposingRef, isReadOnly, onReplayFieldRef]);

  const redo = useCallback(() => {
    if (isReadOnly || isComposingRef.current || redoRef.current.length === 0) return;
    const nextEntry = redoRef.current[redoRef.current.length - 1];
    redoRef.current = redoRef.current.slice(0, -1);
    undoRef.current = [...undoRef.current.slice(-(MAX_UNDO_HISTORY - 1)), nextEntry];
    setRedoStack(redoRef.current);
    setUndoStack(undoRef.current);
    onReplayFieldRef.current(nextEntry, "redo");
  }, [isComposingRef, isReadOnly, onReplayFieldRef]);

  useUndoKeyboardShortcut(isDrawerOpen, undoStack.length > 0 || redoStack.length > 0, undo, redo);
  return { canUndo: undoStack.length > 0, canRedo: redoStack.length > 0, pushUndoEntry, undo, redo };
}

export interface CopilotBatchCommitArgs {
  copilotJobId: string;
  targetSlideUid: string;
  newRevision: number;
  contentSha256: string;
  slots: WordingSlotDto[];
  sanitizedHtml?: string;
}

export function replayCmsUndoEntry(
  entry: CmsUndoEntry,
  dir: "undo" | "redo",
  edits: ReturnType<typeof useCmsEditMutations>
) {
  if (entry.copilotAtomicBatch) {
    for (const p of entry.copilotAtomicBatch.patches) {
      const targetVal = dir === "undo" ? p.previousValue : p.nextValue;
      const prevVal = dir === "undo" ? p.nextValue : p.previousValue;
      edits.applySlideFieldEdit({ ffId: p.ffId, fieldKey: p.fieldKey, slotKind: p.slotKind, bulletItems: p.slotKind === "bullet" ? targetVal.split("\n").filter(Boolean) : undefined, chartsJson: p.slotKind === "chart_json" ? targetVal : undefined, nextValue: targetVal, previousValue: prevVal, recordUndo: false, targetSlideId: entry.slideId });
    }
    return;
  }
  const targetVal = dir === "undo" ? entry.previousValue : entry.nextValue;
  const prevVal = dir === "undo" ? entry.nextValue : entry.previousValue;
  if (entry.fieldKey === "speakerNotes") { edits.applySpeakerNotesEdit(targetVal, false, entry.slideId); return; }
  const slotKind: PatchSlotItem["slotKind"] = entry.slotKind ?? (entry.fieldKey === "headline" ? "heading" : "text");
  edits.applySlideFieldEdit({ ffId: entry.ffId, fieldKey: entry.fieldKey, slotKind, bulletItems: slotKind === "bullet" ? targetVal.split("\n").filter(Boolean) : undefined, chartsJson: slotKind === "chart_json" ? targetVal : undefined, nextValue: targetVal, previousValue: prevVal, recordUndo: false, targetSlideId: entry.slideId });
}

export function applyAtomicCopilotCommit(opts: {
  args: CopilotBatchCommitArgs;
  prevSlots: WordingSlotDto[];
  revisionBySlideRef: React.MutableRefObject<Map<string, number>>;
  contentSha256BySlideRef: React.MutableRefObject<Map<string, string>>;
  deckMode: "multi-page" | "one-page";
  updateSlideSlice: (uid: string, u: (p: SlideCmsSlice) => SlideCmsSlice) => void;
  pushUndoEntry: (entry: CmsUndoEntry) => void;
  onHydrateCommittedHtml?: (slideId: string, sanitizedHtml: string) => void;
  onUpdateSlideById?: (slideId: string, patch: Partial<SlidePhaseItem>) => void;
}) {
  const { copilotJobId, targetSlideUid, newRevision, contentSha256, slots, sanitizedHtml } = opts.args;
  const undoPatches = slots
    .map((nextSlot) => {
      const prevSlot = opts.prevSlots.find((p) => p.ffId === nextSlot.ffId);
      const nextVal = nextSlot.slotKind === "chart_json" ? (nextSlot.chartsJson ?? nextSlot.textValue) : nextSlot.textValue;
      const prevVal = prevSlot ? (prevSlot.slotKind === "chart_json" ? (prevSlot.chartsJson ?? prevSlot.textValue) : prevSlot.textValue) : "";
      if (nextVal === prevVal) return null;
      return { ffId: nextSlot.ffId, fieldKey: (nextSlot.slotKind === "heading" ? "headline" : "slot") as keyof SlidePhaseItem | "slot", slotKind: nextSlot.slotKind, previousValue: prevVal, nextValue: nextVal };
    })
    .filter((x): x is NonNullable<typeof x> => Boolean(x));
  if (undoPatches.length > 0) {
    opts.pushUndoEntry({ slideId: targetSlideUid, ffId: undoPatches[0].ffId, fieldKey: undoPatches[0].fieldKey, slotKind: undoPatches[0].slotKind, previousValue: undoPatches[0].previousValue, nextValue: undoPatches[0].nextValue, timestampMs: Date.now(), copilotAtomicBatch: { copilotJobId, nextSanitizedHtml: sanitizedHtml, patches: undoPatches } });
  }
  syncCommittedRevision(opts.revisionBySlideRef.current, opts.deckMode, targetSlideUid, newRevision);
  syncCommittedContentSha(opts.contentSha256BySlideRef.current, opts.deckMode, targetSlideUid, contentSha256);
  opts.updateSlideSlice(targetSlideUid, (p) => ({ ...p, revision: newRevision, contentSha256, slots, lastFlushError: null, syncState: "SYNCED" }));
  if (sanitizedHtml) opts.onHydrateCommittedHtml?.(targetSlideUid, sanitizedHtml);
  const headingSlot = slots.find((sl) => sl.slotKind === "heading" && sl.textValue);
  if (headingSlot) opts.onUpdateSlideById?.(targetSlideUid, { headline: headingSlot.textValue });
}

export function useCmsTelemetryHandler(patchSentAtRef: React.MutableRefObject<number>) {
  const [lastPatchLatencyMs, setLastPatchLatencyMs] = useState<number | null>(null);
  const [overflowAlert, setOverflowAlert] = useState<CmsOverflowAlert | null>(null);
  const [cspViolationCount, setCspViolationCount] = useState<number>(0);

  const handleDeckTelemetryEvent = useCallback((msg: IframeToHostDeckMessage) => {
    if (msg.type === "PATCH_ACK") {
      const sentAt = patchSentAtRef.current;
      setLastPatchLatencyMs(sentAt > 0 ? Math.max(0, msg.appliedAtMs - sentAt) : 1);
      return;
    }
    if (msg.type === "SLIDE_OVERFLOW_WARN") {
      const overflowPx = typeof msg.overflowPx === "number"
        ? msg.overflowPx
        : Math.max(0, Math.round(msg.scrollHeight - msg.clientHeight));
      setOverflowAlert(overflowPx > 0 ? {
        slideUid: msg.slideUid, ffId: msg.ffId,
        scrollHeight: msg.scrollHeight, clientHeight: msg.clientHeight,
        overflowPx, severity: overflowPx > E3_DANGER_OVERFLOW_PX ? "danger" : "warning",
      } : null);
      return;
    }
    if (msg.type === "DECK_CSP_VIOLATION") setCspViolationCount((c) => c + 1);
  }, [patchSentAtRef]);

  const clearOverflowAlert = useCallback(() => setOverflowAlert(null), []);

  return { lastPatchLatencyMs, overflowAlert, setOverflowAlert, clearOverflowAlert, cspViolationCount, handleDeckTelemetryEvent };
}
