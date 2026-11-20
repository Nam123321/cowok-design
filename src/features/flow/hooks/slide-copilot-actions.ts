// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04 (Slide Copilot Dispatch & Chart Validation Actions)
import { apiPresentationCharts } from "../../../api/presentation-charts";
import type { StructuredCommentEnvelope } from "../../../lib/validations/element-comment";
import { CANONICAL_SLIDE_UID_REGEX, type WordingSlotDto } from "../../../lib/validations/presentation";
import {
  estimateChartSvgPrimitives,
  type ChartKind2D,
  type ChartSpecV12,
  type ValidateChartResponse,
} from "../../../lib/validations/presentation-chart-specs";
import { buildChartAriaLabel, parseChartSpecSafely } from "../utils/offline-chart-2d-renderer";
import {
  COPILOT_EMPTY_SHA256,
  appendUniqueCopilotMessage,
  createEmptyCopilotLease,
  extractCopilotApiError,
  toCopilotCommentPayload,
  toLeaseStateFromDto,
  type SlideCopilotLeaseState,
  type SlideCopilotMessage,
} from "./useSlideCopilotHelpers";

export interface CopilotDispatchContext {
  projectId?: string;
  deckId?: string;
  slideUid: string;
  flushSlideBeforeDispatch: (targetSlideUid?: string, drainAll?: boolean) => Promise<boolean>;
  getSlideCommitSnapshot: (targetUid?: string) => { revision: number; contentSha256: string };
  reloadServerRevision: () => void;
  slots: WordingSlotDto[];
}

export async function runCopilotDispatch(opts: {
  p: CopilotDispatchContext;
  uid: string;
  target: string;
  prompt: string;
  kind?: ChartKind2D;
  comment: StructuredCommentEnvelope | null;
  setLeases: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotLeaseState>>>;
  setMessages: React.Dispatch<React.SetStateAction<Record<string, SlideCopilotMessage[]>>>;
  setComment: React.Dispatch<React.SetStateAction<StructuredCommentEnvelope | null>>;
  setGlobalSlots: React.Dispatch<React.SetStateAction<number>>;
  t?: (key: string, fallback: string) => string;
}): Promise<boolean> {
  const tr = opts.t ?? ((_k: string, f: string) => f);
  const now = Date.now();
  const restoreErrorLease = (prev: Record<string, SlideCopilotLeaseState>, banner: { code: string; message: string; targetFfId: string }) => {
    const cur = prev[opts.uid];
    if (cur?.leaseState === "active" && cur.jobId) return { ...prev, [opts.uid]: { ...cur, errorBanner: banner } };
    return { ...prev, [opts.uid]: { ...createEmptyCopilotLease(opts.uid), errorBanner: banner } };
  };
  try {
    const flushed = await opts.p.flushSlideBeforeDispatch(opts.uid, true);
    if (!flushed) {
      opts.setLeases((prev) => restoreErrorLease(prev, { code: "E_FLUSH_FAILED", message: tr("presentation.copilot.errors.flushFailed", "Pending CMS buffer failed to flush."), targetFfId: opts.target }));
      return false;
    }
    const snap = opts.p.getSlideCommitSnapshot(opts.uid);
    if (!opts.p.projectId || !opts.p.deckId || opts.p.deckId === "local-deck") {
      opts.setLeases((prev) => ({ ...prev, [opts.uid]: createEmptyCopilotLease(opts.uid) }));
      opts.setMessages((m) => appendUniqueCopilotMessage(m, opts.uid, { id: `local-${now}`, role: "system", content: tr("presentation.copilot.errors.serverDeckRequired", "Copilot requires persisted server deck."), timestampMs: now }));
      return false;
    }
    const dto = await apiPresentationCharts.dispatchSlideCopilot(
      { projectId: opts.p.projectId, deckId: opts.p.deckId, slideUid: opts.uid },
      { sessionId: `slide-copilot-${opts.p.deckId}`, targetFfId: opts.target, prompt: opts.prompt, expectedRevision: snap.revision, preFlushedSha256: snap.contentSha256 || COPILOT_EMPTY_SHA256, snapshotReadOnly: false, requestedChartKind: opts.kind, structuredComment: toCopilotCommentPayload(opts.comment) },
      { idempotencyKey: `copilot-${opts.uid}-${snap.revision}-${now}` }
    );
    opts.setComment(null);
    opts.setGlobalSlots(dto.activeGlobalSlots);
    opts.setLeases((prev) => ({ ...prev, [opts.uid]: { ...toLeaseStateFromDto(dto, Date.now(), prev[opts.uid]), startedAtMs: Date.now() } }));
    return true;
  } catch (err) {
    const e = extractCopilotApiError(err);
    if (e.code === "E_PREFLUSH_CONFLICT") opts.p.reloadServerRevision();
    opts.setLeases((prev) => restoreErrorLease(prev, { code: e.code, message: e.message, targetFfId: opts.target }));
    opts.setMessages((m) => appendUniqueCopilotMessage(m, opts.uid, { id: `err-${Date.now()}`, role: "system", content: `${e.code}: ${e.message}`, timestampMs: Date.now(), statusTag: "ERROR" }));
    return false;
  }
}

export async function runChartValidation(
  p: Pick<CopilotDispatchContext, "projectId" | "deckId" | "slideUid">,
  ffId: string,
  spec: ChartSpecV12,
  setChartErrors: React.Dispatch<React.SetStateAction<Record<string, { code: string; message: string } | null>>>
): Promise<ValidateChartResponse | null> {
  const parsed = parseChartSpecSafely(JSON.stringify(spec));
  if (!parsed.ok) {
    setChartErrors((prev) => ({ ...prev, [ffId]: { code: parsed.code, message: parsed.message } }));
    return null;
  }
  if (p.projectId && p.deckId && p.deckId !== "local-deck" && CANONICAL_SLIDE_UID_REGEX.test(p.slideUid)) {
    try {
      const res = await apiPresentationCharts.validateChartSpec({ projectId: p.projectId, deckId: p.deckId, slideUid: p.slideUid }, { ffId, spec: parsed.spec });
      setChartErrors((prev) => ({ ...prev, [ffId]: null }));
      return res;
    } catch (err) {
      const e = extractCopilotApiError(err);
      setChartErrors((prev) => ({ ...prev, [ffId]: { code: e.code, message: e.message } }));
      return null;
    }
  }
  setChartErrors((prev) => ({ ...prev, [ffId]: null }));
  return { valid: true, ffId, kind: parsed.spec.kind, estimatedSvgPrimitives: estimateChartSvgPrimitives(parsed.spec), canonicalChartsJson: JSON.stringify(parsed.spec), ariaLabel: buildChartAriaLabel(parsed.spec), normalizedSpec: parsed.spec };
}
