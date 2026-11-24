// @story 04.1, 05.1, 05.3, 05.4, 06.3
// @implements AC-03, AC-04, AC-05, AC-06 (Dual-Pane Stage Left/Right Subpanes with Live Iframe elementRects)
import React, { useCallback, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import { normalizeFixtureRef } from "../../../lib/validations/screen-fixture";
import { guardReadOnlyAction, isHistoricalPreviewReadOnly, resolveHistoricalReadOnlyStageBindings } from "../../snapshots/utils/snapshot-engine";
import { DesignerTalkOverlay } from "../../proposals/components/DesignerTalkOverlay";
import { PROPOSAL_DRAWER_WIDTH_PX } from "../../proposals/components/CanvasProposalControls";
import { resolveScreenPreviewData, type ScreenPreviewData } from "./companion-canvas-data";
import { CompanionStructureLeftPane, type CmsStudioControllerAdapter } from "./CompanionStructureLeftPane";
import { CompanionLiveHtmlRightPane } from "./CompanionLiveHtmlRightPane";
import { FloatingSlideCopilot, SlideCopilotToastBanner } from "./FloatingSlideCopilot";
import { useCmsSlideEditorController, type PostPatchByFfIdFn } from "../hooks/useCmsSlideEditorController";
import { useAppScreenCmsState } from "../hooks/useAppScreenCmsState";
import type { ElementRectBox } from "../hooks/useSandboxedIframeBridge";
import { useSlideCopilotController } from "../hooks/useSlideCopilotController";
import type { IframeToHostDeckMessage, WordingSlotDto } from "../../../lib/validations/presentation";
import type { ChartSpecV12 } from "../../../lib/validations/presentation-chart-specs";
import type { StageProps } from "./CompanionDualPaneStage";

function buildAppScreenCmsAdapter(appCms: ReturnType<typeof useAppScreenCmsState>): CmsStudioControllerAdapter {
  return {
    slideUid: appCms.activeScreenKey,
    slots: appCms.slots,
    speakerNotes: appCms.speakerNotes,
    syncState: appCms.hasPendingSave ? "DIRTY_FAST_PATCHED" : "SYNCED",
    overflowAlert: null,
    customHtmlOverride: false,
    isHydrating: false,
    setComposing: () => {},
    applySlideFieldEdit: (edit) => appCms.applyScreenFieldEdit(edit.ffId, edit.nextValue, edit.previousValue, edit.recordUndo ?? true),
    applySpeakerNotesEdit: (notes) => appCms.setSpeakerNotes(notes),
  };
}

export function StageLeftStructureSection(props: {
  p: StageProps;
  isDeckTab: boolean;
  cmsController: ReturnType<typeof useCmsSlideEditorController>;
  appCms: ReturnType<typeof useAppScreenCmsState>;
  copilot: ReturnType<typeof useSlideCopilotController>;
  isDeckResolving: boolean;
  cmsFilterFfIds: string[];
  onClearCmsFilter: () => void;
  onRenderChartStage: (ffId: string, spec: ChartSpecV12, ariaLabel: string) => void;
}) {
  const { p, isDeckTab, cmsController, appCms, copilot } = props;
  const { vp } = p;
  if (vp.isLeftStructureHidden || vp.isPresenting) return null;
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(p.historicalPreviewTurn?.id);
  const isCmsReadOnly = isHistoricalReadOnly || (isDeckTab && props.isDeckResolving);
  const safeBindings = resolveHistoricalReadOnlyStageBindings(p, isHistoricalReadOnly);
  const slides = isDeckTab ? vp.slides : appCms.screenCards;
  const activeSlideIndex = isDeckTab ? vp.activeSlideIndex : appCms.activeCardIndex;
  const handleSelectSlide = (idx: number) => {
    props.onClearCmsFilter();
    if (isDeckTab) vp.setActiveSlideIndex(idx);
    else if (appCms.screenCards[idx] && p.nodes?.some((n) => n.id === appCms.screenCards[idx].id)) p.onSelectNode?.(appCms.screenCards[idx].id);
    vp.setIsCmsDrawerOpen(true);
  };
  return (
    <CompanionStructureLeftPane
      widthPercent={vp.leftPanePercent} isRightPreviewHidden={vp.isRightPreviewHidden} structureLens={vp.structureLens}
      isDeckTab={isDeckTab}
      onSetLens={vp.setStructureLens} onToggleRightPreviewHidden={() => vp.setIsRightPreviewHidden((prev) => !prev)}
      isDraftDirty={Boolean(safeBindings.isDraftDirty)} orphanCount={p.orphanCount} conflictState={safeBindings.conflictState ?? null}
      onDeployDraft={safeBindings.onDeployDraft} onDiscardDraft={safeBindings.onDiscardDraft} onResolveConflict={safeBindings.onResolveConflict}
      slides={slides} activeSlideIndex={activeSlideIndex} isCmsDrawerOpen={vp.isCmsDrawerOpen} isPresenting={vp.isPresenting}
      isReadOnly={isCmsReadOnly} cmsController={isDeckTab ? cmsController : buildAppScreenCmsAdapter(appCms)}
      cmsFilterFfIds={props.cmsFilterFfIds} onClearCmsFilter={props.onClearCmsFilter} onSelectSlide={handleSelectSlide}
      onCloseCmsDrawer={() => { props.onClearCmsFilter(); vp.setIsCmsDrawerOpen(false); }}
      onUpdateActiveSlide={guardReadOnlyAction(isCmsReadOnly, (patch) => { if (isDeckTab) vp.setSlides((prev) => prev.map((it, i) => (i === vp.activeSlideIndex ? { ...it, ...patch } : it))); })}
      isWalkthroughActive={p.isWalkthroughActive} walkthroughStepLabel={p.walkthroughStepLabel}
      copilotLeasesBySlide={isDeckTab ? copilot.leasesBySlide : undefined} chartErrorByFfId={isDeckTab ? copilot.chartErrorByFfId : undefined}
      onValidateChartSpec={isDeckTab ? copilot.validateChartSpec : undefined} onRenderChartStage={isDeckTab ? props.onRenderChartStage : undefined}
      onRetryChartWithCopilot={isDeckTab ? copilot.retryChartWithCopilot : undefined}
      onCancelCopilot={isDeckTab ? () => { void copilot.cancelCopilot(); } : undefined}
      onOpenCopilot={isDeckTab ? (ffId) => copilot.openCopilot({ targetFfId: ffId }) : undefined}
    >
      {p.children}
    </CompanionStructureLeftPane>
  );
}

function buildRightStageCmsBindings(opts: {
  p: StageProps; isDeckTab: boolean; cmsController: ReturnType<typeof useCmsSlideEditorController>;
  appCms: ReturnType<typeof useAppScreenCmsState>; isReadOnly: boolean;
}) {
  const { p, isDeckTab, cmsController, appCms, isReadOnly } = opts;
  if (isDeckTab) {
    return {
      cmsActions: {
        revision: cmsController.revision, syncState: cmsController.syncState,
        hasPendingSave: cmsController.hasAnyDirtySlides || cmsController.syncState !== "SYNCED",
        canUndo: cmsController.canUndo, canRedo: cmsController.canRedo, isReadOnly,
        onUndo: cmsController.undo, onRedo: cmsController.redo, onSave: () => { void cmsController.flushAllPendingNow(); },
      },
      cmsRecovery: {
        recoveredDraft: cmsController.recoveredDraft, syncState: cmsController.syncState, lastFlushError: cmsController.lastFlushError,
        customHtmlOverride: cmsController.customHtmlOverride, onRestoreDraft: () => { cmsController.restoreRecoveredDraft(); p.vp.setIsCmsDrawerOpen(true); },
        onDismissDraft: cmsController.dismissRecoveredDraft, onRetryFlush: () => { void cmsController.flushPendingBatchNow(); },
        onReloadServerRevision: cmsController.reloadServerRevision, onResyncCustomHtml: cmsController.resyncCustomHtmlOverride,
      },
    };
  }
  const hasAppDirty = appCms.hasPendingSave || Boolean(p.isDraftDirty);
  return {
    cmsActions: {
      revision: appCms.revision, syncState: hasAppDirty ? ("DIRTY_FAST_PATCHED" as const) : ("SYNCED" as const),
      hasPendingSave: hasAppDirty, canUndo: appCms.canUndo || Boolean(p.canUndo), canRedo: appCms.canRedo || Boolean(p.canRedo), isReadOnly,
      onUndo: () => (appCms.canUndo ? appCms.undo() : p.onUndo?.()),
      onRedo: () => (appCms.canRedo ? appCms.redo() : p.onRedo?.()),
      onSave: () => { appCms.markSaved(); p.onDeployDraft?.(); },
    },
    cmsRecovery: undefined,
  };
}

function matchAndApplyDeckInlineEdit(cmsController: ReturnType<typeof useCmsSlideEditorController>, ffId: string, nextValue: string, previousValue: string) {
  const exact = cmsController.slots.find((s: WordingSlotDto) => s.ffId === ffId);
  const isHeadline = ffId === "ff-slide-headline" || ffId.endsWith("-headline");
  const isSubtitle = ffId === "ff-slide-subtitle" || ffId.endsWith("-body");
  const fallback = !exact && cmsController.slots.length > 0
    ? cmsController.slots.find((s: WordingSlotDto) => (isHeadline && (s.slotKind === "heading" || s.ffId.endsWith("-headline"))) || (isSubtitle && (s.ffId.endsWith("-body") || s.slotKind === "text")))
    : undefined;
  const matched = exact ?? fallback;
  if (cmsController.slots.length > 0 && !matched) return;
  const fieldKey = isHeadline ? "headline" : isSubtitle ? "subtitle" : "slot";
  const slotKind = matched?.slotKind ?? (isHeadline ? "heading" : "text");
  cmsController.applySlideFieldEdit({ ffId: matched?.ffId ?? ffId, fieldKey, slotKind, nextValue, previousValue, bulletItems: slotKind === "bullet" ? nextValue.split("\n").filter(Boolean) : undefined, chartsJson: slotKind === "chart_json" ? nextValue : undefined });
}

export function DualPaneRightStage(props: {
  p: StageProps; isDeckTab: boolean; selectedNode: NonNullable<StageProps["nodes"]>[number] | null; baseScreenData: ScreenPreviewData;
  cmsController: ReturnType<typeof useCmsSlideEditorController>; appCms: ReturnType<typeof useAppScreenCmsState>;
  copilot: ReturnType<typeof useSlideCopilotController>; onRegisterPatchByFfId: (fn: PostPatchByFfIdFn | null) => void;
  onRegisterRender2DChart: (fn: ((opts: { slideUid: string; ffId: string; spec: ChartSpecV12; ariaLabel: string }) => void) | null) => void;
  pinnedSlideIds: string[]; isDeckResolving: boolean; onOpenRegionCmsEdit: (coveredFfIds: string[]) => void;
  paneProps: React.ComponentProps<typeof CompanionLiveHtmlRightPane>; isHistoricalReadOnly: boolean; slideCode: string;
  onApplyHtmlWithCms: (nextHtml: string) => Promise<string | null | void>;
  onReloadDeck?: () => void;
  blackoutMode?: "none" | "black" | "white";
  isPresenterActive?: boolean;
  onOpenPresenter?: () => void;
  onRegisterDeckBlackout?: (fn: ((mode: "none" | "black" | "white") => void) | null) => void;
  presenterToastNode?: React.ReactNode;
}) {
  const { p, isDeckTab, cmsController, appCms, copilot } = props;
  const { vp } = p;
  const [liveElementRects, setLiveElementRects] = useState<Record<string, ElementRectBox>>({});
  const copilotReadOnly = props.isHistoricalReadOnly || (isDeckTab && props.isDeckResolving);
  const { cmsActions, cmsRecovery } = buildRightStageCmsBindings({ p, isDeckTab, cmsController, appCms, isReadOnly: copilotReadOnly });
  const handleInlineEdit = useCallback((ffId: string, nextVal: string, prevVal: string) => {
    if (!isDeckTab) { appCms.applyScreenFieldEdit(ffId, nextVal, prevVal); return; }
    matchAndApplyDeckInlineEdit(cmsController, ffId, nextVal, prevVal);
  }, [appCms, cmsController, isDeckTab]);
  const handleTelemetry = useCallback((msg: IframeToHostDeckMessage) => {
    cmsController.handleDeckTelemetryEvent(msg);
    if (msg.type === "STAGE_MAT_CLICK" && copilot.isOpen && !copilot.isCollapsed) copilot.setIsCollapsed(true);
  }, [cmsController, copilot]);
  const { cancelCopilot, submitPrompt, clearAttachedComment, dismissReceiptToast, setDraftPrompt, setIsCollapsed, setIsOpen } = copilot;
  const syncStateLabel = isDeckTab ? cmsController.syncState : (appCms.hasPendingSave ? "DIRTY_FAST_PATCHED" : "SYNCED");
  const handleLinkToDag = useCallback(() => { vp.setIsLeftStructureHidden(false); vp.setStructureLens("userflow"); if (p.selectedNodeId) p.onSelectNode?.(p.selectedNodeId); }, [p, vp]);

  return (
    <div style={{ position: "relative", flex: 1, display: "flex", minWidth: 0, height: "100%", overflow: "hidden" }}>
      <CompanionLiveHtmlRightPane
        {...props.paneProps} onApplyHtml={props.onApplyHtmlWithCms} onRegisterPatchByFfId={props.onRegisterPatchByFfId}
        onRegisterRender2DChart={props.onRegisterRender2DChart} onDeckTelemetryEvent={handleTelemetry}
        pinnedSlideIds={props.pinnedSlideIds} overflowAlertPx={isDeckTab ? cmsController.overflowAlert?.overflowPx : undefined}
        cmsActions={cmsActions} cmsRecovery={cmsRecovery} cmsSlots={isDeckTab ? cmsController.slots : appCms.slots} cmsSyncState={syncStateLabel}
        onCmsInlineFieldEdit={handleInlineEdit} onCmsComposingChange={isDeckTab ? cmsController.setComposing : undefined}
        onOpenRegionCmsEdit={props.onOpenRegionCmsEdit} onLinkToDag={handleLinkToDag} isLeftDagFocused={!isDeckTab && vp.structureLens === "userflow"}
        blackoutMode={props.blackoutMode} isPresenterActive={props.isPresenterActive} onOpenPresenter={props.onOpenPresenter}
        onRegisterDeckBlackout={props.onRegisterDeckBlackout} presenterToastNode={props.presenterToastNode}
        stageSkin={p.stageSkin} sketchPolishBarNode={p.sketchPolishBarNode}
        onRegisterStageSkinSwap={p.onRegisterStageSkinSwap} onRegisterStageApplyHtml={p.onRegisterStageApplyHtml}
        onElementRectsChange={setLiveElementRects}
        copilotToastNode={isDeckTab ? <SlideCopilotToastBanner toast={copilot.receiptToast} onDismiss={dismissReceiptToast} /> : null}
        floatingCopilotNode={isDeckTab ? (
          <FloatingSlideCopilot isOpen={copilot.isOpen} isCollapsed={copilot.isCollapsed} isReadOnly={copilotReadOnly} projectId={p.projectId} deckId={cmsController.deckId !== "local-deck" ? cmsController.deckId : undefined} slideRevision={cmsController.revision} slideCode={props.slideCode} slideUid={cmsController.slideUid} targetFfId={copilot.targetFfId} draftPrompt={copilot.draftPrompt} attachedComment={copilot.attachedComment} activeLease={copilot.activeLease} messages={copilot.messages} activeGlobalSlots={copilot.activeGlobalSlots} maxGlobalSlots={copilot.maxGlobalSlots} onDraftPromptChange={setDraftPrompt} onSendPrompt={(kind) => { void submitPrompt(copilot.draftPrompt, kind); }} onRewriteCommitted={(result) => { setDraftPrompt(""); void cmsController.reloadServerRevision(); if (result.continuationSlides.length > 0) props.onReloadDeck?.(); }} onCancelLease={() => { void cancelCopilot(); }} onCollapse={() => setIsCollapsed(true)} onExpand={() => setIsCollapsed(false)} onClose={() => setIsOpen(false)} onClearAttachedComment={clearAttachedComment} />
        ) : null}
      />
      <DesignerTalkOverlay isOpen={p.isDesignerTalkOpen} tradeoffs={p.activeProposal?.tradeoffs ?? []} elementRects={liveElementRects} drawerWidth={p.isProposalDrawerOpen ? PROPOSAL_DRAWER_WIDTH_PX : 0} isModalOpen={p.isModalOpen} isInlineEditorOpen={vp.isInlineCodeOpen} onClose={p.onCloseDesignerTalk} onDecide={p.onDecideTradeoff} />
    </div>
  );
}

export function resolveBaseScreenDataForStage(p: StageProps): { selectedNode: NonNullable<StageProps["nodes"]>[number] | null; baseScreenData: ScreenPreviewData } {
  const { t } = useTranslation();
  const selectedNode = p.nodes?.find((n) => n.id === p.selectedNodeId) || null;
  const baseScreenData = resolveScreenPreviewData(selectedNode, p.selectedNodeId, {
    kpiRoute: t("canvas.previewSynthKpiRoute", "Route"), kpiState: t("canvas.previewSynthKpiState", "Node Status"),
    kpiType: t("canvas.previewSynthKpiType", "Node Type"), actionHint: t("canvas.previewSynthActionHint", "Synthesized live preview from WorkbenchNode metadata."), ctaOpen: t("canvas.previewSynthCtaOpen", "Open"),
  });
  return { selectedNode, baseScreenData };
}

export function resolveSlideFormattedAmount(rawKpi: string | number, locale: string, formatCurrency: (n: number, c: string) => string): string {
  return typeof rawKpi === "number" ? formatCurrency(Number(rawKpi), "USD") : String(rawKpi);
}

export function useFormattedLocaleHelpers() {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  return { t, locale, formatCurrency, normalizeFixtureRef };
}
