// @story 06.2
// @implements AC-04, AC-06
import { useCallback, useEffect, useRef, useState } from "react";
import { apiPresentation } from "../../../api/presentation";
import type { CmsRecoveryDraft, PatchSlotItem, WordingSlotDto } from "../../../lib/validations/presentation";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import { clearRecoveryDraft, loadRecoveryDraft, loadRecoveryDraftWithIdbFallback } from "../utils/cms-draft-storage";
import { syncCommittedRevision, type SlideCmsSlice } from "./useCmsSlideSupportHooks";

export interface CmsRecoveryEditActions {
  applySlideFieldEdit: (args: {
    ffId: string;
    fieldKey: keyof SlidePhaseItem | "slot";
    slotKind: PatchSlotItem["slotKind"];
    nextValue: string;
    previousValue: string;
    bulletItems?: string[];
    chartsJson?: string;
    recordUndo?: boolean;
    targetSlideId?: string;
  }) => void;
  applySpeakerNotesEdit: (nextNotes: string, recordUndo?: boolean, targetSlideId?: string) => void;
}

function resolveDraftFieldKey(ffId: string): keyof SlidePhaseItem | "slot" {
  if (ffId === "ff-slide-headline" || ffId.endsWith("-headline")) return "headline";
  if (ffId === "ff-slide-subtitle" || ffId.endsWith("-body")) return "subtitle";
  return "slot";
}

function hasDraftContent(draft: CmsRecoveryDraft | null): draft is CmsRecoveryDraft {
  return Boolean(draft && (draft.pendingPatches.length > 0 || draft.speakerNotes.length > 0));
}

function resolveCanonicalDraftFfId(
  rawFfId: string,
  fieldKey: keyof SlidePhaseItem | "slot",
  slots: WordingSlotDto[]
): string {
  if (slots.some((sl) => sl.ffId === rawFfId)) return rawFfId;
  if (fieldKey === "headline") {
    const match = slots.find((sl) => sl.slotKind === "heading" || sl.ffId.endsWith("-headline"));
    if (match) return match.ffId;
  }
  if (fieldKey === "subtitle") {
    const match = slots.find((sl) => sl.ffId.endsWith("-body") || sl.slotKind === "text");
    if (match) return match.ffId;
  }
  return rawFfId;
}

function replayDraftPatches(
  draft: CmsRecoveryDraft,
  activeSlide: SlidePhaseItem,
  slots: WordingSlotDto[],
  edits: CmsRecoveryEditActions
) {
  for (const p of draft.pendingPatches) {
    const fieldKey = resolveDraftFieldKey(p.ffId);
    const resolvedFfId = resolveCanonicalDraftFfId(p.ffId, fieldKey, slots);
    const prevVal =
      fieldKey === "slot"
        ? (slots.find((sl) => sl.ffId === resolvedFfId)?.textValue ?? "")
        : String(activeSlide[fieldKey] ?? "");
    edits.applySlideFieldEdit({
      ffId: resolvedFfId, fieldKey, slotKind: p.slotKind, bulletItems: p.bulletItems,
      chartsJson: p.chartsJson, nextValue: p.textValue, previousValue: prevVal, recordUndo: true,
    });
  }
  if (draft.speakerNotes) edits.applySpeakerNotesEdit(draft.speakerNotes, true);
}

export function useCmsDraftRecovery(opts: {
  deckId: string; slideUid: string; activeSlide: SlidePhaseItem; slots: WordingSlotDto[];
  updateSlideSlice: (uid: string, u: (p: SlideCmsSlice) => SlideCmsSlice) => void;
  clearOverflowAlert: () => void; edits: CmsRecoveryEditActions;
}) {
  const [recoveredDraft, setRecoveredDraft] = useState<CmsRecoveryDraft | null>(null);
  useEffect(() => {
    let cancelled = false;
    const syncFound = loadRecoveryDraft(opts.deckId, opts.slideUid);
    setRecoveredDraft(hasDraftContent(syncFound) ? syncFound : null);
    opts.clearOverflowAlert();
    if (!hasDraftContent(syncFound)) {
      void loadRecoveryDraftWithIdbFallback(opts.deckId, opts.slideUid).then((idbFound) => {
        if (!cancelled && hasDraftContent(idbFound)) setRecoveredDraft(idbFound);
      });
    }
    return () => { cancelled = true; };
  }, [opts.clearOverflowAlert, opts.deckId, opts.slideUid]);

  const restoreRecoveredDraft = useCallback(() => {
    if (!recoveredDraft) return;
    replayDraftPatches(recoveredDraft, opts.activeSlide, opts.slots, opts.edits);
    setRecoveredDraft(null);
  }, [opts.activeSlide, opts.edits, opts.slots, recoveredDraft]);

  const dismissRecoveredDraft = useCallback(() => {
    clearRecoveryDraft(opts.deckId, opts.slideUid);
    setRecoveredDraft(null);
    opts.updateSlideSlice(opts.slideUid, (p) => ({
      ...p, lastFlushError: null, syncState: p.syncState === "FLUSH_FAILED_RETRY" ? "SYNCED" : p.syncState,
    }));
  }, [opts.deckId, opts.slideUid, opts.updateSlideSlice]);

  return { recoveredDraft, restoreRecoveredDraft, dismissRecoveredDraft };
}

interface CustomHtmlOverrideOpts {
  projectId?: string; deckId: string; slideUid: string; isRemoteBacked: boolean;
  deckModeRef: React.MutableRefObject<"multi-page" | "one-page">;
  revisionBySlideRef: React.MutableRefObject<Map<string, number>>;
  contentSha256BySlideRef?: React.MutableRefObject<Map<string, string>>;
  updateSlideSlice: (uid: string, u: (p: SlideCmsSlice) => SlideCmsSlice) => void;
}

export interface ApplyCustomHtmlOutcome {
  ok: boolean;
  appliedHtml?: string;
}

async function commitRemoteCustomHtml(
  opts: CustomHtmlOverrideOpts,
  targetUid: string,
  rawHtml: string,
  resyncStructuredSlots: boolean
): Promise<ApplyCustomHtmlOutcome> {
  if (!opts.projectId) return { ok: false };
  const expectedRevision = opts.revisionBySlideRef.current.get(targetUid) ?? 1;
  try {
    const res = await apiPresentation.applyCustomHtml(
      { projectId: opts.projectId, deckId: opts.deckId, slideUid: targetUid },
      { expectedRevision, rawHtml, resyncStructuredSlots },
      { idempotencyKey: `apply-${opts.deckId}-${targetUid}-r${expectedRevision}-${Date.now()}` }
    );
    syncCommittedRevision(opts.revisionBySlideRef.current, opts.deckModeRef.current, targetUid, res.newRevision);
    if (res.contentSha256) opts.contentSha256BySlideRef?.current.set(targetUid, res.contentSha256);
    opts.updateSlideSlice(targetUid, (p) => ({
      ...p, revision: res.newRevision, contentSha256: res.contentSha256, slots: res.slots, customHtmlOverride: res.customHtmlOverride,
      unmappedSlotIds: res.unmappedSlotIds, lastFlushError: null, syncState: "SYNCED",
    }));
    return { ok: true, appliedHtml: res.sanitizedHtml };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "E_APPLY_HTML_FAILED";
    opts.updateSlideSlice(targetUid, (p) => ({ ...p, lastFlushError: msg, syncState: "FLUSH_FAILED_RETRY" }));
    return { ok: false };
  }
}

export function useCmsCustomHtmlOverrideActions(opts: CustomHtmlOverrideOpts) {
  const lastHtmlBySlideRef = useRef<Map<string, string>>(new Map());
  const applyInlineCustomHtml = useCallback(
    async (rawHtml: string, resyncStructuredSlots = true): Promise<ApplyCustomHtmlOutcome> => {
      const targetUid = opts.slideUid;
      const normalizedHtml = rawHtml.replace(/<meta\s+charset=["']?utf-8["']?\s*\/?>\s*/gi, "");
      if (normalizedHtml.trim()) lastHtmlBySlideRef.current.set(targetUid, normalizedHtml);
      if (!opts.isRemoteBacked || !opts.projectId) {
        opts.updateSlideSlice(targetUid, (p) => ({
          ...p, customHtmlOverride: !resyncStructuredSlots, unmappedSlotIds: resyncStructuredSlots ? [] : p.unmappedSlotIds,
        }));
        return { ok: true, appliedHtml: normalizedHtml };
      }
      return commitRemoteCustomHtml(opts, targetUid, normalizedHtml, resyncStructuredSlots);
    },
    [opts]
  );

  const resyncCustomHtmlOverride = useCallback(() => {
    const cachedHtml = lastHtmlBySlideRef.current.get(opts.slideUid) ?? "";
    if (opts.isRemoteBacked) {
      void applyInlineCustomHtml(cachedHtml, true);
      return;
    }
    opts.updateSlideSlice(opts.slideUid, (p) => ({
      ...p, customHtmlOverride: false, unmappedSlotIds: [], lastFlushError: null, syncState: "SYNCED",
    }));
  }, [applyInlineCustomHtml, opts]);

  return { resyncCustomHtmlOverride, applyInlineCustomHtml };
}
