// @story 06.2, 06.3
// @implements AC-01, AC-03, AC-06
import { useCallback, useRef, useState } from "react";
import { CANONICAL_SLIDE_UID_REGEX, DEFAULT_FALLBACK_SLIDE_UID, type PatchSlotItem } from "../../../lib/validations/presentation";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import { CMS_DRAFT_STORAGE_PREFIX, clearRecoveryDraft, loadRecoveryDraft, saveRecoveryDraft } from "../utils/cms-draft-storage";
import { useCmsCustomHtmlOverrideActions, useCmsDraftRecovery } from "./useCmsSlideRecoveryActions";
import { useCmsEditMutations } from "./useCmsEditMutations";
import {
  CMS_FLUSH_DEBOUNCE_MS,
  MAX_UNDO_HISTORY,
  applyAtomicCopilotCommit,
  createDefaultSlideSlice,
  replayCmsUndoEntry,
  useCmsBatchFlusher,
  useCmsDirtyBeforeUnload,
  useCmsRemoteHydration,
  useCmsTelemetryHandler,
  useCmsUndoRedoHistory,
  type CmsOverflowAlert,
  type CmsSyncState,
  type CmsUndoEntry,
  type CopilotBatchCommitArgs,
  type PendingSlideBuffer,
  type SlideCmsSlice,
} from "./useCmsSlideSupportHooks";

export { CMS_DRAFT_STORAGE_PREFIX, CMS_FLUSH_DEBOUNCE_MS, MAX_UNDO_HISTORY, clearRecoveryDraft, loadRecoveryDraft, saveRecoveryDraft };
export type { CmsOverflowAlert, CmsSyncState, CmsUndoEntry };

export interface PostPatchByFfIdFn {
  (ffId: string, patch: { textContent?: string; safeStyles?: Record<string, string>; bulletItems?: string[]; chartsJson?: string; slideUid?: string; slotKind?: string; revision?: number }): void;
}

export interface UseCmsSlideEditorControllerParams {
  projectId?: string;
  deckId?: string;
  activeSlide: SlidePhaseItem;
  isDrawerOpen: boolean;
  isReadOnly?: boolean;
  isDeckLoading?: boolean;
  onUpdateActiveSlide: (patch: Partial<SlidePhaseItem>) => void;
  onUpdateSlideById?: (slideId: string, patch: Partial<SlidePhaseItem>) => void;
  onHydrateCommittedHtml?: (slideId: string, sanitizedHtml: string) => void;
  onDiscardReloadDeck?: () => void;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
}

const FALLBACK_UID = DEFAULT_FALLBACK_SLIDE_UID;

function useCmsPerSlideStore(slideUid: string, activeSlideId: string) {
  const [slicesBySlide, setSlicesBySlide] = useState<Record<string, SlideCmsSlice>>({});
  const [speakerNotesBySlide, setSpeakerNotesBySlide] = useState<Record<string, string>>({});
  const [reloadToken, setReloadToken] = useState(0);
  const isComposingRef = useRef(false);
  const patchSentAtRef = useRef(0);
  const editSeqRef = useRef(0);
  const pendingBySlideRef = useRef<Map<string, PendingSlideBuffer>>(new Map());
  const revisionBySlideRef = useRef<Map<string, number>>(new Map());
  const contentSha256BySlideRef = useRef<Map<string, string>>(new Map());
  const deckModeRef = useRef<"multi-page" | "one-page">("multi-page");
  const updateSlideSlice = useCallback((targetUid: string, updater: (prev: SlideCmsSlice) => SlideCmsSlice) => {
    setSlicesBySlide((prev) => ({ ...prev, [targetUid]: updater(prev[targetUid] ?? createDefaultSlideSlice()) }));
  }, []);
  const activeSlice = slicesBySlide[slideUid] ?? createDefaultSlideSlice();
  const hasAnyDirtySlides = Object.values(slicesBySlide).some((sl) => sl.syncState !== "SYNCED");
  return { slicesBySlide, activeSlice, speakerNotes: speakerNotesBySlide[activeSlideId] ?? "", speakerNotesBySlide, setSpeakerNotesBySlide, updateSlideSlice, hasAnyDirtySlides, reloadToken, setReloadToken, isComposingRef, patchSentAtRef, editSeqRef, pendingBySlideRef, revisionBySlideRef, contentSha256BySlideRef, deckModeRef };
}

function useCmsSlideHydrationSync(opts: {
  isRemoteBacked: boolean; projectId?: string; deckId: string; slideUid: string;
  activeSlideId: string; s: ReturnType<typeof useCmsPerSlideStore>;
  onHydrateCommittedHtml?: (slideId: string, sanitizedHtml: string) => void;
  onUpdateSlideById?: (slideId: string, patch: Partial<SlidePhaseItem>) => void;
}) {
  const { s } = opts;
  useCmsRemoteHydration({
    isRemoteBacked: opts.isRemoteBacked, projectId: opts.projectId, deckId: opts.deckId, slideUid: opts.slideUid,
    activeSlideId: opts.activeSlideId, reloadToken: s.reloadToken, pendingBySlideRef: s.pendingBySlideRef, isComposingRef: s.isComposingRef,
    onHydrationStart: (uid) => s.updateSlideSlice(uid, (p) => ({ ...p, isHydrating: true })),
    onHydrated: (uid, sid, snap) => {
      if (snap.deckMode) s.deckModeRef.current = snap.deckMode;
      s.revisionBySlideRef.current.set(uid, snap.revision);
      if (snap.contentSha256) s.contentSha256BySlideRef.current.set(uid, snap.contentSha256);
      s.updateSlideSlice(uid, (p) => ({ ...p, revision: snap.revision, contentSha256: snap.contentSha256 ?? p.contentSha256, slots: snap.slots, customHtmlOverride: snap.customHtmlOverride, unmappedSlotIds: snap.unmappedSlotIds, lastFlushError: null, syncState: "SYNCED" }));
      s.setSpeakerNotesBySlide((prev) => ({ ...prev, [sid]: snap.speakerNotes }));
      if (snap.sanitizedHtml) opts.onHydrateCommittedHtml?.(sid, snap.sanitizedHtml);
      const hSlot = snap.slots.find((sl) => sl.slotKind === "heading" && sl.textValue);
      if (hSlot) opts.onUpdateSlideById?.(sid, { headline: hSlot.textValue });
    },
    onHydrationDone: (uid) => s.updateSlideSlice(uid, (p) => ({ ...p, isHydrating: false })),
    onHydrationError: (uid, msg) => s.updateSlideSlice(uid, (p) => ({ ...p, lastFlushError: msg, syncState: "FLUSH_FAILED_RETRY" })),
  });
}

export function useCmsSlideEditorController(params: UseCmsSlideEditorControllerParams) {
  const { projectId, deckId = "local-deck", activeSlide, isDrawerOpen, isReadOnly = false, isDeckLoading = false, onUpdateActiveSlide, onUpdateSlideById, onHydrateCommittedHtml, onDiscardReloadDeck, postPatchByFfIdRef } = params;
  const isCanonicalUid = CANONICAL_SLIDE_UID_REGEX.test(activeSlide.id);
  const slideUid = isCanonicalUid ? activeSlide.id : FALLBACK_UID;
  const isRemoteBacked = Boolean(projectId && deckId !== "local-deck" && !isDeckLoading && isCanonicalUid);
  const s = useCmsPerSlideStore(slideUid, activeSlide.id);
  const telemetry = useCmsTelemetryHandler(s.patchSentAtRef);
  useCmsDirtyBeforeUnload(s.hasAnyDirtySlides);
  useCmsSlideHydrationSync({ isRemoteBacked, projectId, deckId, slideUid, activeSlideId: activeSlide.id, s, onHydrateCommittedHtml, onUpdateSlideById });
  const onDiscardReload = useCallback((dirtyUids: string[]) => { s.setSpeakerNotesBySlide((prev) => { const next = { ...prev }; dirtyUids.forEach((id) => delete next[id]); return next; }); s.setReloadToken((n) => n + 1); onDiscardReloadDeck?.(); }, [onDiscardReloadDeck, s]);
  const flusher = useCmsBatchFlusher({ projectId, deckId, slideUid, isReadOnly, isRemoteBacked, deckModeRef: s.deckModeRef, isComposingRef: s.isComposingRef, revisionBySlideRef: s.revisionBySlideRef, contentSha256BySlideRef: s.contentSha256BySlideRef, pendingBySlideRef: s.pendingBySlideRef, updateSlideSlice: s.updateSlideSlice, onDiscardReload });
  const replayRef = useRef<(entry: CmsUndoEntry, dir: "undo" | "redo") => void>(() => {});
  const history = useCmsUndoRedoHistory({ isDrawerOpen, isReadOnly, isComposingRef: s.isComposingRef, onReplayFieldRef: replayRef });
  const edits = useCmsEditMutations({ deckId, slideUid, activeSlideId: activeSlide.id, speakerNotesBySlide: s.speakerNotesBySlide, slicesBySlide: s.slicesBySlide, isReadOnly, revisionBySlideRef: s.revisionBySlideRef, editSeqRef: s.editSeqRef, isComposingRef: s.isComposingRef, patchSentAtRef: s.patchSentAtRef, pendingBySlideRef: s.pendingBySlideRef, postPatchByFfIdRef, onUpdateActiveSlide, onUpdateSlideById, updateSlideSlice: s.updateSlideSlice, setSpeakerNotesBySlide: s.setSpeakerNotesBySlide, pushUndoEntry: history.pushUndoEntry, scheduleDebouncedFlush: flusher.scheduleDebouncedFlush });
  replayRef.current = (entry, dir) => replayCmsUndoEntry(entry, dir, edits);
  const draft = useCmsDraftRecovery({ deckId, slideUid, activeSlide, slots: s.activeSlice.slots, updateSlideSlice: s.updateSlideSlice, clearOverflowAlert: telemetry.clearOverflowAlert, edits });
  const override = useCmsCustomHtmlOverrideActions({ projectId, deckId, slideUid, isRemoteBacked, deckModeRef: s.deckModeRef, revisionBySlideRef: s.revisionBySlideRef, contentSha256BySlideRef: s.contentSha256BySlideRef, updateSlideSlice: s.updateSlideSlice });
  const setComposing = useCallback((val: boolean) => { s.isComposingRef.current = val; if (!val && s.pendingBySlideRef.current.has(slideUid)) flusher.scheduleDebouncedFlush(slideUid); }, [flusher, s.isComposingRef, s.pendingBySlideRef, slideUid]);
  const reloadServerRevision = useCallback(() => { s.pendingBySlideRef.current.delete(slideUid); clearRecoveryDraft(deckId, slideUid); s.updateSlideSlice(slideUid, (p) => ({ ...p, lastFlushError: null, syncState: "SYNCED" })); s.setReloadToken((n) => n + 1); }, [deckId, s, slideUid]);
  const getSlideCommitSnapshot = useCallback((targetUid?: string) => { const uid = targetUid || slideUid; return { revision: s.revisionBySlideRef.current.get(uid) ?? s.slicesBySlide[uid]?.revision ?? 1, contentSha256: s.contentSha256BySlideRef.current.get(uid) ?? s.slicesBySlide[uid]?.contentSha256 ?? "" }; }, [s.contentSha256BySlideRef, s.revisionBySlideRef, s.slicesBySlide, slideUid]);
  const applyCopilotBatchCommit = useCallback((args: CopilotBatchCommitArgs) => applyAtomicCopilotCommit({ args, prevSlots: s.slicesBySlide[args.targetSlideUid]?.slots ?? [], revisionBySlideRef: s.revisionBySlideRef, contentSha256BySlideRef: s.contentSha256BySlideRef, deckMode: s.deckModeRef.current, updateSlideSlice: s.updateSlideSlice, pushUndoEntry: history.pushUndoEntry, onHydrateCommittedHtml, onUpdateSlideById }), [history.pushUndoEntry, onHydrateCommittedHtml, onUpdateSlideById, s.contentSha256BySlideRef, s.deckModeRef, s.revisionBySlideRef, s.slicesBySlide, s.updateSlideSlice]);
  return {
    deckId, slideUid, revision: s.activeSlice.revision, contentSha256: s.activeSlice.contentSha256, slots: s.activeSlice.slots, speakerNotes: s.speakerNotes, syncState: s.activeSlice.syncState,
    lastFlushError: s.activeSlice.lastFlushError, isHydrating: isDeckLoading || s.activeSlice.isHydrating, hasAnyDirtySlides: s.hasAnyDirtySlides, lastPatchLatencyMs: telemetry.lastPatchLatencyMs,
    overflowAlert: telemetry.overflowAlert, cspViolationCount: telemetry.cspViolationCount, customHtmlOverride: s.activeSlice.customHtmlOverride, unmappedSlotIds: s.activeSlice.unmappedSlotIds,
    ...draft, ...override, canUndo: history.canUndo, canRedo: history.canRedo, setComposing, reloadServerRevision, getSlideCommitSnapshot, applyCopilotBatchCommit, ...edits,
    flushPendingBatchNow: flusher.flushPendingBatchNow, flushAllPendingNow: flusher.flushAllPendingNow, discardAllPendingNow: flusher.discardAllPendingNow, undo: history.undo, redo: history.redo,
    setCustomHtmlOverrideState: (isOverride: boolean, unmapped: string[] = []) => s.updateSlideSlice(slideUid, (p) => ({ ...p, customHtmlOverride: isOverride, unmappedSlotIds: unmapped })),
    handleDeckTelemetryEvent: telemetry.handleDeckTelemetryEvent,
  };
}
