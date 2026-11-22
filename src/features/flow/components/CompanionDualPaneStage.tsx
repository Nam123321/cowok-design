// @story 04.1, 05.1, 05.3, 06.3
// @implements AC-03, AC-04, AC-06 Dual Pane HTML Stage
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { submitInlineHtmlProposal } from "../../proposals/utils/proposal-events";
import { useCmsSlideEditorController, type PostPatchByFfIdFn } from "../hooks/useCmsSlideEditorController";
import { usePersistedPresentationDeck } from "../hooks/usePersistedPresentationDeck";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ChangeProposalResponse, DecideTradeoffInput } from "../../../lib/validations/change-proposal";
import { normalizeFixtureRef, type FixtureVariant } from "../../../lib/validations/screen-fixture";
import type { ChatArtifactSnapshotResponse } from "../../../lib/validations/version-snapshot";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";
import { guardReadOnlyAction, isHistoricalPreviewReadOnly, parseTurnManifestFiles } from "../../snapshots/utils/snapshot-engine";
import type { ScreenPreviewData } from "./companion-canvas-data";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";
import type { CanvasTabItem, CompanionCanvasTabProps } from "./CompanionCanvasTab";
import { useDualPaneViewportState } from "../hooks/useCompanionCanvasState";
import { useAppScreenCmsState } from "../hooks/useAppScreenCmsState";
import { useSlideCopilotController } from "../hooks/useSlideCopilotController";
import { DualPaneRightStage, StageLeftStructureSection, resolveBaseScreenDataForStage } from "./DualPaneStageSubpanes";
import { CompanionLiveHtmlRightPane } from "./CompanionLiveHtmlRightPane";
import { PresenterInlineSplitFallback } from "./PresenterInlineSplitFallback";
import { CowokToast } from "../../../components/ui/CowokToast";
import { usePresenterSessionController } from "../hooks/usePresenterSessionController";
import { usePresenterInlineCollapse } from "../hooks/usePresenterInlineCollapse";
import type { ChartSpecV12 } from "../../../lib/validations/presentation-chart-specs";
import type { PresenterBlackoutMode } from "../../../lib/validations/presentation-presenter";

const SPLITTER_Z_INDEX = 15;

export interface StageProps extends CompanionCanvasTabProps {
  activeTab: CanvasTabItem;
  vp: ReturnType<typeof useDualPaneViewportState>;
  splitContainerRef: React.RefObject<HTMLDivElement | null>;
  isLaunchpadOpen: boolean;
  onOpenLaunchpad: () => void;
  onCloseLaunchpad: () => void;
  isSampleDataOpen: boolean;
  onOpenSampleData: () => void;
  onCloseSampleData: () => void;
  isWalkthroughActive: boolean;
  onToggleWalkthrough: () => void;
  walkthroughViewMode?: "dual" | "full";
  onSetWalkthroughViewMode?: (mode: "dual" | "full") => void;
  walkthroughStepLabel?: string;
  walkthroughAutoPlay?: boolean;
  onToggleWalkthroughAutoPlay?: () => void;
  onWalkthroughPrev?: () => void;
  onWalkthroughNext?: () => void;
  onExitWalkthrough?: () => void;
  snapshotsCount?: number;
  onOpenVersionHistory?: () => void;
  onOpenExportSection?: (section: ExportInitialSection) => void;
  isDesignerTalkOpen: boolean;
  onCloseDesignerTalk: () => void;
  isProposalDrawerOpen: boolean;
  isModalOpen: boolean;
  activeProposal: ChangeProposalResponse | null;
  onDecideTradeoff: (tradeoffId: string, input: DecideTradeoffInput) => Promise<void>;
  onGuardedSubStateSelect: (subState: FixtureVariant) => void;
  onGuardedModeSelect: (mode: "interact" | "click-dom" | "drag-box") => void;
  historicalPreviewTurn: ChatArtifactSnapshotResponse | null;
  versionMenuNode?: React.ReactNode;
  onCmsDirtyChange?: (isDirty: boolean) => void;
  onRegisterCmsGuardActions?: (actions: { flushAll: () => Promise<boolean>; discardAll: () => void; applyCustomHtml: (rawHtml: string) => Promise<{ ok: boolean; appliedHtml?: string }> } | null) => void;
  onTogglePresentDeck?: () => void;
  deckAutoPlay?: boolean;
  onToggleDeckAutoPlay?: () => void;
  onDeckPrev?: () => void;
  onDeckNext?: () => void;
  onExitPresent?: () => void;
  onEnterPresentDeck?: () => void;
  onPostDeckKeyNav?: (dir: "prev" | "next" | "goto", targetIndex?: number) => void;
  onRegisterDeckKeyNav?: (fn: ((dir: "prev" | "next" | "goto", targetIndex?: number) => void) | null) => void;
  onRegisterPresenterBindings?: (bindings: { isPresenterActive: boolean; toggleBlackoutKey: (mode: "black" | "white") => void } | null) => void;
}

function StageSplitter({ isDragging, onDragStart }: { isDragging: boolean; onDragStart: () => void }) {
  return (
    <div
      role="separator" aria-orientation="vertical" onMouseDown={(e) => { e.preventDefault(); onDragStart(); }}
      style={{ width: "4px", minWidth: "4px", height: "100%", cursor: "col-resize", backgroundColor: isDragging ? "var(--action-primary)" : "var(--border-default)", zIndex: SPLITTER_Z_INDEX }}
    />
  );
}

function useDualPanePreviewModel(p: StageProps, isSlideLensActive: boolean, selectedNode: NonNullable<StageProps["nodes"]>[number] | null, activeScreenData: ScreenPreviewData) {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  const { activeTab, vp } = p;
  const effectiveStageLens: "userflow" | "slide-cms" = isSlideLensActive ? "slide-cms" : "userflow";
  const activeSlide = vp.slides[vp.activeSlideIndex] || vp.slides[0];
  const targetKey = effectiveStageLens === "slide-cms" ? `${activeTab.id}::slide-cms::${activeSlide.id}` : `${activeTab.id}::userflow::${p.selectedNodeId || "default"}::screen`;
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(p.historicalPreviewTurn?.id);
  const historicalHtmlFile = p.historicalPreviewTurn ? parseTurnManifestFiles(p.historicalPreviewTurn.manifestJson).find((f) => f.path.endsWith(".html")) : undefined;
  const customRawHtml = isHistoricalReadOnly ? historicalHtmlFile?.content : vp.customHtmlByTarget[targetKey];
  const rawKpi = effectiveStageLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount;
  const nextSeedHtml = buildPreviewDocumentHtml({
    structureLens: effectiveStageLens, activeSlide, activeScreenData,
    formattedAmount: typeof rawKpi === "number" ? formatCurrency(Number(rawKpi), "USD") : String(rawKpi),
    labels: { lang: locale, kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"), kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"), kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"), actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state.") },
  });
  return { selectedNode, activeScreenData, activeSlide, targetKey, isHistoricalReadOnly, customRawHtml, nextSeedHtml, effectiveStageLens };
}

function useInlineHtmlEditorActions(p: StageProps, model: ReturnType<typeof useDualPanePreviewModel>) {
  const { activeTab, vp } = p;
  const { selectedNode, targetKey, isHistoricalReadOnly, nextSeedHtml, customRawHtml } = model;
  const handleToggleInlineCode = () => {
    if (isHistoricalReadOnly) return;
    if (vp.isInlineCodeOpen && vp.dirtyBuffer) { vp.setShowDirtyGuardPrompt(true); return; }
    const seed = customRawHtml || nextSeedHtml;
    vp.setHtmlCodeText(seed); vp.setBaselineHtmlText(seed); vp.setShowDirtyGuardPrompt(false); vp.setIsInlineCodeOpen((prev) => !prev);
  };
  const handleApplyHtml = (html: string) => {
    if (isHistoricalReadOnly) return;
    const prevHtml = vp.baselineHtmlText;
    vp.setBaselineHtmlText(html); vp.setShowDirtyGuardPrompt(false); vp.setCustomHtmlForTarget(targetKey, html); vp.setIsInlineCodeOpen(false);
    submitInlineHtmlProposal({ projectId: p.projectId, tabTitle: activeTab.title, nodeId: p.selectedNodeId, screenKey: selectedNode?.screenKey, updatedHtml: html, previousHtml: prevHtml });
  };
  return { handleToggleInlineCode, handleApplyHtml, handleRegionClick: (id: string) => vp.setHighlightedRegionId(id) };
}

function buildLiveRightPaneProps(opts: { p: StageProps; model: ReturnType<typeof useDualPanePreviewModel>; editor: ReturnType<typeof useInlineHtmlEditorActions>; isDeckResolving: boolean }): React.ComponentProps<typeof CompanionLiveHtmlRightPane> {
  const { p, model, editor, isDeckResolving } = opts;
  const { activeTab, vp } = p;
  const { selectedNode, activeScreenData, activeSlide, isHistoricalReadOnly, customRawHtml, effectiveStageLens } = model;
  const isInlineReadOnly = isHistoricalReadOnly || isDeckResolving;
  return {
    projectId: p.projectId, activeTab, structureLens: effectiveStageLens, selectedNodeId: p.selectedNodeId, selectedNodeKey: selectedNode?.screenKey || undefined,
    activeSlide, activeScreenData, activeSubState: normalizeFixtureRef(selectedNode?.activeSubState ?? vp.activeSubState),
    onSelectSubState: guardReadOnlyAction(isHistoricalReadOnly, p.onGuardedSubStateSelect), pointerMode: vp.pointerMode,
    onSetPointerMode: (m) => (m === "interact" || !isHistoricalReadOnly) && p.onGuardedModeSelect(m), isInlineCodeOpen: vp.isInlineCodeOpen,
    dirtyBuffer: vp.dirtyBuffer, showDirtyGuardPrompt: vp.showDirtyGuardPrompt, onToggleInlineCode: guardReadOnlyAction(isInlineReadOnly, editor.handleToggleInlineCode),
    onConfirmDiscardDirty: () => { vp.setHtmlCodeText(vp.baselineHtmlText); vp.setShowDirtyGuardPrompt(false); vp.setIsInlineCodeOpen(false); },
    onCancelCloseEditor: () => vp.setShowDirtyGuardPrompt(false), inlineCodeText: vp.htmlCodeText, onChangeInlineCode: guardReadOnlyAction(isInlineReadOnly, vp.setHtmlCodeText),
    viewportPreset: vp.viewportPreset, onSetViewportPreset: guardReadOnlyAction(isHistoricalReadOnly, (nextVp) => { vp.setViewportPreset(nextVp); if (p.selectedNodeId) p.onUpdateNodeViewport?.(p.selectedNodeId, nextVp); }),
    aspectRatio: vp.aspectRatio, onSetAspectRatio: vp.setAspectRatio, zoomMode: vp.zoomMode, onSetZoomMode: vp.setZoomMode,
    isPresenting: vp.isPresenting, onTogglePresenting: p.onTogglePresentDeck ?? (() => vp.setIsPresenting((prev) => !prev)),
    isSampleDataOpen: p.isSampleDataOpen, onToggleSampleData: p.isSampleDataOpen ? p.onCloseSampleData : p.onOpenSampleData,
    isWalkthroughActive: p.isWalkthroughActive, onToggleWalkthrough: p.onToggleWalkthrough, walkthroughViewMode: p.walkthroughViewMode,
    onSetWalkthroughViewMode: p.onSetWalkthroughViewMode, walkthroughStepLabel: p.walkthroughStepLabel, walkthroughAutoPlay: p.walkthroughAutoPlay,
    onToggleWalkthroughAutoPlay: p.onToggleWalkthroughAutoPlay, onWalkthroughPrev: p.onWalkthroughPrev, onWalkthroughNext: p.onWalkthroughNext,
    onExitWalkthrough: p.onExitWalkthrough, deckAutoPlay: p.deckAutoPlay, onToggleDeckAutoPlay: p.onToggleDeckAutoPlay, onDeckPrev: p.onDeckPrev,
    onDeckNext: p.onDeckNext, onExitPresent: p.onExitPresent, slidesCount: vp.slides.length, activeSlideIndex: vp.activeSlideIndex,
    snapshotsCount: p.snapshotsCount, onOpenVersionHistory: p.onOpenVersionHistory, onOpenExportSection: p.onOpenExportSection,
    isLeftStructureHidden: vp.isLeftStructureHidden, onToggleLeftStructureHidden: () => vp.setIsLeftStructureHidden((prev) => !prev),
    versionMenuNode: p.versionMenuNode, isReadOnly: isInlineReadOnly, onRegisterDeckKeyNav: p.onRegisterDeckKeyNav, onChangeSlideIndex: vp.setActiveSlideIndex,
    highlightedRegionId: vp.highlightedRegionId, customRawHtml, onApplyHtml: editor.handleApplyHtml, onQuoteText: p.onQuoteText, onRegionClick: editor.handleRegionClick,
  };
}

function useStageCmsBindings(p: StageProps, isDeckTab: boolean) {
  const { vp } = p;
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(p.historicalPreviewTurn?.id);
  const [deckReloadToken, setDeckReloadToken] = useState(0);
  const { deckId: persistedDeckId, isDeckResolving } = usePersistedPresentationDeck({ projectId: p.projectId, enabled: isDeckTab && vp.structureLens === "slide-cms", slides: vp.slides, setSlides: vp.setSlides, reloadToken: deckReloadToken });
  const isCmsReadOnly = isHistoricalReadOnly || (isDeckTab && isDeckResolving);
  const activeSlide = vp.slides[vp.activeSlideIndex] || vp.slides[0];
  const postPatchByFfIdRef = useRef<PostPatchByFfIdFn | null>(null);
  const onRegisterPatchByFfId = useCallback((fn: PostPatchByFfIdFn | null) => { postPatchByFfIdRef.current = fn; }, []);
  const onUpdateActiveSlide = useCallback((patch: Partial<typeof activeSlide>) => { if (!isHistoricalReadOnly) vp.setSlides((prev) => prev.map((it, i) => (i === vp.activeSlideIndex ? { ...it, ...patch } : it))); }, [isHistoricalReadOnly, vp]);
  const onUpdateSlideById = useCallback((slideId: string, patch: Partial<typeof activeSlide>) => { if (!isHistoricalReadOnly) vp.setSlides((prev) => prev.map((it) => (it.id === slideId ? { ...it, ...patch } : it))); }, [isHistoricalReadOnly, vp]);
  const onHydrateCommittedHtml = useCallback((slideId: string, sanitizedHtml: string) => { if (sanitizedHtml) vp.setCustomHtmlForTarget(`${p.activeTab.id}::slide-cms::${slideId}`, sanitizedHtml); }, [p.activeTab.id, vp]);
  const onDiscardReloadDeck = useCallback(() => setDeckReloadToken((n) => n + 1), []);
  const cmsController = useCmsSlideEditorController({ projectId: p.projectId, deckId: persistedDeckId, activeSlide, isDrawerOpen: vp.isCmsDrawerOpen, isReadOnly: isCmsReadOnly, isDeckLoading: isDeckResolving, onUpdateActiveSlide, onUpdateSlideById, onHydrateCommittedHtml, onDiscardReloadDeck, postPatchByFfIdRef });
  useEffect(() => { p.onCmsDirtyChange?.(isDeckTab && cmsController.hasAnyDirtySlides); }, [cmsController.hasAnyDirtySlides, isDeckTab, p.onCmsDirtyChange]);
  useEffect(() => { p.onRegisterCmsGuardActions?.({ flushAll: cmsController.flushAllPendingNow, discardAll: cmsController.discardAllPendingNow, applyCustomHtml: (rawHtml) => cmsController.applyInlineCustomHtml(rawHtml, true) }); return () => p.onRegisterCmsGuardActions?.(null); }, [cmsController.applyInlineCustomHtml, cmsController.discardAllPendingNow, cmsController.flushAllPendingNow, p.onRegisterCmsGuardActions]);
  const pinnedSlideIds = useMemo(() => [activeSlide?.id, vp.slides[vp.activeSlideIndex + 1]?.id].filter((id): id is string => Boolean(id)), [activeSlide?.id, vp.activeSlideIndex, vp.slides]);
  return { cmsController, postPatchByFfIdRef, onRegisterPatchByFfId, pinnedSlideIds, isDeckResolving, isHistoricalReadOnly };
}

function buildSpeakerNotesMap(slides: StageProps["vp"]["slides"], isDeckTab: boolean, activeSlideId: string, cmsSpeakerNotes: string) {
  const map: Record<string, string> = {};
  for (const sl of slides) {
    if (typeof sl.speakerNotes === "string") map[sl.id] = sl.speakerNotes;
  }
  if (isDeckTab && activeSlideId) map[activeSlideId] = cmsSpeakerNotes;
  return map;
}

function buildCustomHtmlSlideMap(tabId: string, customHtmlByTarget: Record<string, string>) {
  const prefix = `${tabId}::slide-cms::`;
  const out: Record<string, string> = {};
  for (const [key, html] of Object.entries(customHtmlByTarget)) {
    if (key.startsWith(prefix)) out[key.slice(prefix.length)] = html;
  }
  return out;
}

function resolveSnapshotTagFromTurn(turn: ChatArtifactSnapshotResponse | null): string | null {
  if (!turn?.label) return null;
  const trimmed = turn.label.trim();
  return /^SNAP-[A-Za-z0-9_-]{1,58}$/.test(trimmed) ? trimmed : null;
}

function renderPresenterToastNode(
  presenter: ReturnType<typeof usePresenterSessionController>,
  t: ReturnType<typeof useTranslation>["t"]
): React.ReactNode {
  if (presenter.apiError) {
    return (
      <CowokToast
        tone="danger" testId="presenter-api-error-toast"
        message={presenter.apiError.message || t("presentation.presenter.handshakeError", "Presenter handshake failed.")}
        onDismiss={presenter.clearApiError} durationMs={0}
      />
    );
  }
  if (!presenter.isPopupBlockedToastVisible) return null;
  return (
    <CowokToast
      tone="warning" className="ff-toast-warning" testId="presenter-popup-blocked-toast"
      message={t("presentation.presenter.popupBlockedToast", "Trình duyệt đang chặn popup. Đã kích hoạt Inline Split Presenter.")}
      actionLabel={t("presentation.presenter.reopenWindow", "Mở lại Presenter Window")}
      actionTestId="btn-reopen-presenter-toast" onAction={presenter.reopenPresenterWindow}
      onDismiss={presenter.dismissPopupBlockedToast} durationMs={0}
    />
  );
}

function useStagePresenterSession(opts: {
  p: StageProps; isDeckTab: boolean; deckId: string; activeSlideId: string; cmsSpeakerNotes: string; isHistoricalReadOnly: boolean;
}) {
  const { t } = useTranslation();
  const { p, isDeckTab, deckId, activeSlideId, cmsSpeakerNotes, isHistoricalReadOnly } = opts;
  const { vp } = p;
  const deckBlackoutRef = useRef<((mode: PresenterBlackoutMode) => void) | null>(null);
  const onRegisterDeckBlackout = useCallback((fn: ((mode: PresenterBlackoutMode) => void) | null) => { deckBlackoutRef.current = fn; }, []);
  const speakerNotesBySlideId = useMemo(() => buildSpeakerNotesMap(vp.slides, isDeckTab, activeSlideId, cmsSpeakerNotes), [activeSlideId, cmsSpeakerNotes, isDeckTab, vp.slides]);
  const customHtmlBySlideId = useMemo(() => buildCustomHtmlSlideMap(p.activeTab.id, vp.customHtmlByTarget), [p.activeTab.id, vp.customHtmlByTarget]);

  const presenter = usePresenterSessionController({
    projectId: p.projectId, deckId, deckTitle: p.activeTab.title, aspectRatio: vp.aspectRatio,
    slides: vp.slides, activeSlideIndex: vp.activeSlideIndex, speakerNotesBySlideId, customHtmlBySlideId,
    isAudienceFullscreen: vp.isPresenting, isHistoricalReadOnly,
    snapshotTag: resolveSnapshotTagFromTurn(p.historicalPreviewTurn),
    onChangeSlide: vp.setActiveSlideIndex, onPostDeckKeyNav: p.onPostDeckKeyNav,
    onPostDeckBlackout: (mode) => deckBlackoutRef.current?.(mode),
    onToggleAudienceFullscreen: (req) => {
      if (req === "enter") p.onEnterPresentDeck?.();
      else if (req === "exit") p.onExitPresent?.();
      else p.onTogglePresentDeck?.();
    },
  });

  useEffect(() => {
    const guardedBlackout = presenter.guardPresenterMutation(presenter.toggleBlackoutKey);
    p.onRegisterPresenterBindings?.({ isPresenterActive: presenter.isPresenterActive, toggleBlackoutKey: guardedBlackout });
    return () => p.onRegisterPresenterBindings?.(null);
  }, [p.onRegisterPresenterBindings, presenter.guardPresenterMutation, presenter.isPresenterActive, presenter.toggleBlackoutKey]);

  const presenterToastNode = renderPresenterToastNode(presenter, t);
  return { presenter, onRegisterDeckBlackout, presenterToastNode };
}

function StageInlinePresenterDock(opts: { p: StageProps; presenter: ReturnType<typeof usePresenterSessionController> }) {
  const { p, presenter } = opts;
  if (!presenter.isInlineFallbackOpen) return null;
  const guard = presenter.guardPresenterMutation;
  return (
    <PresenterInlineSplitFallback
      deckTitle={p.activeTab.title} slides={presenter.presenterSlides} activeSlideIndex={p.vp.activeSlideIndex}
      currentSlideHtml={presenter.currentSlideHtml} nextSlideHtml={presenter.nextSlideHtml}
      blackoutMode={presenter.blackoutMode} isAudienceFullscreen={p.vp.isPresenting}
      transportMode={presenter.transportMode} syncLatencyMs={presenter.syncLatencyMs}
      isHistoricalReadOnly={presenter.isHistoricalReadOnly} snapshotTag={presenter.snapshotTag}
      elapsedMs={presenter.elapsedMs} isTimerRunning={presenter.timerState.isRunning}
      onPrevSlide={guard(() => presenter.navigateSlide(p.vp.activeSlideIndex - 1))}
      onNextSlide={guard(() => presenter.navigateSlide(p.vp.activeSlideIndex + 1))}
      onToggleBlackout={guard((m: "black" | "white") => presenter.toggleBlackoutKey(m))}
      onClearBlackout={guard(() => presenter.setBlackoutMode("none"))}
      onToggleFullscreen={guard(() => p.onTogglePresentDeck?.())} onTimerCommand={guard(presenter.dispatchTimer)}
      onReopenWindow={presenter.reopenPresenterWindow} onClose={presenter.closePresenterSession}
    />
  );
}

async function applyHtmlWithCmsUpdate(opts: {
  nextHtml: string;
  isDeckTab: boolean;
  isDeckResolving: boolean;
  isHistoricalReadOnly: boolean;
  structureLens: string;
  cmsController: ReturnType<typeof useStageCmsBindings>["cmsController"];
  editor: ReturnType<typeof useInlineHtmlEditorActions>;
}): Promise<string | null | void> {
  if ((opts.isDeckTab && opts.isDeckResolving) || opts.isHistoricalReadOnly) return null;
  if (!opts.isDeckTab || opts.structureLens !== "slide-cms") {
    opts.editor.handleApplyHtml(opts.nextHtml);
    return opts.nextHtml;
  }
  if (!(await opts.cmsController.flushAllPendingNow())) return null;
  const outcome = await opts.cmsController.applyInlineCustomHtml(opts.nextHtml, true);
  if (!outcome.ok || typeof outcome.appliedHtml !== "string") return null;
  opts.editor.handleApplyHtml(outcome.appliedHtml);
  return outcome.appliedHtml;
}

export function DualPaneHtmlStage(p: StageProps) {
  const { vp, splitContainerRef } = p;
  const isDeckTab = p.activeTab.format === "slide-cms";
  const showSplitter = !vp.isLeftStructureHidden && !vp.isRightPreviewHidden && !vp.isPresenting;
  const { cmsController, postPatchByFfIdRef, onRegisterPatchByFfId, pinnedSlideIds, isDeckResolving, isHistoricalReadOnly } = useStageCmsBindings(p, isDeckTab);
  const { selectedNode, baseScreenData } = resolveBaseScreenDataForStage(p);
  const appCms = useAppScreenCmsState({ projectId: p.projectId, nodes: p.nodes, selectedNodeId: p.selectedNodeId, baseScreenData, isReadOnly: isHistoricalReadOnly, postPatchByFfIdRef, onLockedRuleReceipt: p.onLockedRuleReceipt });
  const [cmsFilterFfIds, setCmsFilterFfIds] = useState<string[]>([]);
  const render2DChartRef = useRef<((opts: { slideUid: string; ffId: string; spec: ChartSpecV12; ariaLabel: string }) => void) | null>(null);
  const onRegisterRender2DChart = useCallback((fn: ((opts: { slideUid: string; ffId: string; spec: ChartSpecV12; ariaLabel: string }) => void) | null) => { render2DChartRef.current = fn; }, []);
  const deckReadOnly = isHistoricalReadOnly || isDeckResolving;
  const copilot = useSlideCopilotController({ enabled: isDeckTab && vp.structureLens === "slide-cms", projectId: p.projectId, deckId: cmsController.deckId, slideUid: cmsController.slideUid, isReadOnly: deckReadOnly, slots: cmsController.slots, flushSlideBeforeDispatch: cmsController.flushPendingBatchNow, getSlideCommitSnapshot: cmsController.getSlideCommitSnapshot, reloadServerRevision: cmsController.reloadServerRevision, applyCopilotBatchCommit: cmsController.applyCopilotBatchCommit });
  const model = useDualPanePreviewModel(p, isDeckTab, selectedNode, isDeckTab ? baseScreenData : appCms.mergedScreenData);
  const editor = useInlineHtmlEditorActions(p, model);
  const paneProps = buildLiveRightPaneProps({ p, model, editor, isDeckResolving: isDeckTab && isDeckResolving });
  const { presenter, onRegisterDeckBlackout, presenterToastNode } = useStagePresenterSession({
    p, isDeckTab, deckId: cmsController.deckId, activeSlideId: model.activeSlide.id,
    cmsSpeakerNotes: cmsController.speakerNotes, isHistoricalReadOnly: model.isHistoricalReadOnly,
  });
  usePresenterInlineCollapse(presenter.isInlineFallbackOpen, vp.isLeftStructureHidden, vp.setIsLeftStructureHidden);
  const handleApplyHtmlWithCms = (nextHtml: string) => applyHtmlWithCmsUpdate({
    nextHtml, isDeckTab, isDeckResolving, isHistoricalReadOnly: model.isHistoricalReadOnly,
    structureLens: vp.structureLens, cmsController, editor,
  });
  return (
    <div ref={splitContainerRef} className="canvas-dual-pane-stage" style={{ flex: 1, display: "flex", width: "100%", height: "100%", overflow: "hidden", position: "relative" }}>
      <StageLeftStructureSection p={p} isDeckTab={isDeckTab} cmsController={cmsController} appCms={appCms} copilot={copilot} isDeckResolving={isDeckResolving} cmsFilterFfIds={cmsFilterFfIds} onClearCmsFilter={() => setCmsFilterFfIds([])} onRenderChartStage={(ffId, spec, ariaLabel) => render2DChartRef.current?.({ slideUid: cmsController.slideUid, ffId, spec, ariaLabel })} />
      {showSplitter && <StageSplitter isDragging={vp.isDraggingInternalSplit} onDragStart={() => vp.setIsDraggingInternalSplit(true)} />}
      {!vp.isRightPreviewHidden && (
        <DualPaneRightStage p={p} isDeckTab={isDeckTab} selectedNode={selectedNode} baseScreenData={baseScreenData} cmsController={cmsController} appCms={appCms} copilot={copilot} onRegisterPatchByFfId={onRegisterPatchByFfId} onRegisterRender2DChart={onRegisterRender2DChart} pinnedSlideIds={pinnedSlideIds} isDeckResolving={isDeckResolving} onOpenRegionCmsEdit={(ids) => { vp.setIsLeftStructureHidden(false); vp.setStructureLens("slide-cms"); vp.setIsCmsDrawerOpen(true); setCmsFilterFfIds(ids); }} paneProps={paneProps} isHistoricalReadOnly={model.isHistoricalReadOnly} slideCode={model.activeSlide.code} onApplyHtmlWithCms={handleApplyHtmlWithCms} blackoutMode={presenter.blackoutMode} isPresenterActive={presenter.isPresenterActive} onOpenPresenter={presenter.openPresenterView} onRegisterDeckBlackout={onRegisterDeckBlackout} presenterToastNode={presenterToastNode} />
      )}
      <StageInlinePresenterDock p={p} presenter={presenter} />
    </div>
  );
}
