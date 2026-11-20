// @story 06.2
// @implements AC-01, AC-03, AC-06 (Live CMS Field & Speaker Notes Mutations)
import { useCallback, useRef } from "react";
import { CANONICAL_SLIDE_UID_REGEX, type PatchSlotItem } from "../../../lib/validations/presentation";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import { saveRecoveryDraft } from "../utils/cms-draft-storage";
import type { PostPatchByFfIdFn } from "./useCmsSlideEditorController";
import type { CmsUndoEntry, PendingSlideBuffer, SlideCmsSlice } from "./useCmsSlideSupportHooks";

export interface SlideFieldEditArgs {
  ffId: string;
  fieldKey: keyof SlidePhaseItem | "slot";
  slotKind: PatchSlotItem["slotKind"];
  nextValue: string;
  previousValue: string;
  bulletItems?: string[];
  chartsJson?: string;
  recordUndo?: boolean;
  targetSlideId?: string;
}

export interface CmsMutationCtx {
  deckId: string;
  slideUid: string;
  activeSlideId: string;
  speakerNotesBySlide: Record<string, string>;
  slicesBySlide: Record<string, SlideCmsSlice>;
  isReadOnly: boolean;
  revisionBySlideRef: React.MutableRefObject<Map<string, number>>;
  editSeqRef: React.MutableRefObject<number>;
  isComposingRef: React.MutableRefObject<boolean>;
  patchSentAtRef: React.MutableRefObject<number>;
  pendingBySlideRef: React.MutableRefObject<Map<string, PendingSlideBuffer>>;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
  onUpdateActiveSlide: (patch: Partial<SlidePhaseItem>) => void;
  onUpdateSlideById?: (slideId: string, patch: Partial<SlidePhaseItem>) => void;
  updateSlideSlice: (uid: string, u: (p: SlideCmsSlice) => SlideCmsSlice) => void;
  setSpeakerNotesBySlide: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  pushUndoEntry: (entry: CmsUndoEntry) => void;
  scheduleDebouncedFlush: (uid?: string) => void;
}

function persistSlideRecoveryBuffer(c: CmsMutationCtx, targetUid: string, targetSlideId: string) {
  const buf = c.pendingBySlideRef.current.get(targetUid);
  if (!buf || !CANONICAL_SLIDE_UID_REGEX.test(targetUid)) return;
  saveRecoveryDraft({
    deckId: c.deckId,
    slideUid: targetUid,
    baseRevision: c.revisionBySlideRef.current.get(targetUid) ?? 1,
    speakerNotes: buf.notes?.value ?? c.speakerNotesBySlide[targetSlideId] ?? "",
    pendingPatches: Array.from(buf.patches.values()).map((v) => v.item),
    savedAtMs: Date.now(),
  });
}

function applySlotMutationState(c: CmsMutationCtx, args: SlideFieldEditArgs, targetUid: string, targetSlideId: string) {
  const isCurrent = targetSlideId === c.activeSlideId || targetUid === c.slideUid;
  if (args.fieldKey !== "slot") {
    const patch = { [args.fieldKey]: args.nextValue } as Partial<SlidePhaseItem>;
    if (isCurrent) c.onUpdateActiveSlide(patch);
    else c.onUpdateSlideById?.(targetSlideId, patch);
  }
  c.updateSlideSlice(targetUid, (prev) => ({
    ...prev,
    syncState: "DIRTY_FAST_PATCHED",
    lastFlushError: null,
    slots: prev.slots.map((sl) =>
      sl.ffId === args.ffId
        ? { ...sl, textValue: args.nextValue, chartsJson: args.chartsJson ?? sl.chartsJson, updatedBy: "human" as const }
        : sl
    ),
  }));
}

function runFieldMutation(c: CmsMutationCtx, args: SlideFieldEditArgs) {
  const targetSlideId = args.targetSlideId || c.activeSlideId;
  const targetUid = CANONICAL_SLIDE_UID_REGEX.test(targetSlideId) ? targetSlideId : c.slideUid;
  if (c.isReadOnly || c.slicesBySlide[targetUid]?.customHtmlOverride) return;
  applySlotMutationState(c, args, targetUid, targetSlideId);
  c.patchSentAtRef.current = Date.now();
  c.postPatchByFfIdRef.current?.(args.ffId, {
    textContent: args.nextValue, bulletItems: args.bulletItems, chartsJson: args.chartsJson,
    slideUid: targetUid, slotKind: args.slotKind, revision: c.revisionBySlideRef.current.get(targetUid) ?? 1,
  });
  const buf: PendingSlideBuffer = c.pendingBySlideRef.current.get(targetUid) ?? { patches: new Map() };
  c.editSeqRef.current += 1;
  buf.patches.set(args.ffId, {
    item: { ffId: args.ffId, slotKind: args.slotKind, textValue: args.nextValue, bulletItems: args.bulletItems, chartsJson: args.chartsJson },
    editSeq: c.editSeqRef.current,
  });
  c.pendingBySlideRef.current.set(targetUid, buf);
  persistSlideRecoveryBuffer(c, targetUid, targetSlideId);
  if (args.recordUndo !== false) {
    c.pushUndoEntry({ slideId: targetSlideId, ffId: args.ffId, fieldKey: args.fieldKey, slotKind: args.slotKind, previousValue: args.previousValue, nextValue: args.nextValue, timestampMs: Date.now() });
  }
  if (!c.isComposingRef.current) c.scheduleDebouncedFlush(targetUid);
}

function runNotesMutation(c: CmsMutationCtx, nextNotes: string, recordUndo: boolean, targetSlideId?: string) {
  if (c.isReadOnly) return;
  const sid = targetSlideId || c.activeSlideId;
  const targetUid = CANONICAL_SLIDE_UID_REGEX.test(sid) ? sid : c.slideUid;
  const previousValue = c.speakerNotesBySlide[sid] ?? "";
  c.setSpeakerNotesBySlide((prev) => ({ ...prev, [sid]: nextNotes }));
  const buf: PendingSlideBuffer = c.pendingBySlideRef.current.get(targetUid) ?? { patches: new Map() };
  c.editSeqRef.current += 1;
  buf.notes = { value: nextNotes, editSeq: c.editSeqRef.current };
  c.pendingBySlideRef.current.set(targetUid, buf);
  c.updateSlideSlice(targetUid, (prev) => ({ ...prev, syncState: "DIRTY_FAST_PATCHED", lastFlushError: null }));
  persistSlideRecoveryBuffer(c, targetUid, sid);
  if (recordUndo) {
    c.pushUndoEntry({ slideId: sid, ffId: "speakerNotes", fieldKey: "speakerNotes", previousValue, nextValue: nextNotes, timestampMs: Date.now() });
  }
  if (!c.isComposingRef.current) c.scheduleDebouncedFlush(targetUid);
}

export function useCmsEditMutations(ctx: CmsMutationCtx) {
  const ctxRef = useRef(ctx);
  ctxRef.current = ctx;
  const applySlideFieldEdit = useCallback((args: SlideFieldEditArgs) => runFieldMutation(ctxRef.current, args), []);
  const applySpeakerNotesEdit = useCallback((nextNotes: string, recordUndo = true, targetSlideId?: string) => runNotesMutation(ctxRef.current, nextNotes, recordUndo, targetSlideId), []);
  return { applySlideFieldEdit, applySpeakerNotesEdit };
}
