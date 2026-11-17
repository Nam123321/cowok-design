// @story 04.1, 05.1, 05.3
// @implements AC-03, AC-04, AC-06 Dual Pane HTML Stage
import React from "react";
import { submitInlineHtmlProposal } from "../../proposals/utils/proposal-events";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ChangeProposalResponse, DecideTradeoffInput } from "../../../lib/validations/change-proposal";
import { normalizeFixtureRef, type FixtureVariant } from "../../../lib/validations/screen-fixture";
import type { ChatArtifactSnapshotResponse } from "../../../lib/validations/version-snapshot";
import {
  guardReadOnlyAction,
  isHistoricalPreviewReadOnly,
  parseTurnManifestFiles,
  resolveHistoricalReadOnlyStageBindings,
} from "../../snapshots/utils/snapshot-engine";
import { DesignerTalkOverlay } from "../../proposals/components/DesignerTalkOverlay";
import { PROPOSAL_DRAWER_WIDTH_PX } from "../../proposals/components/CanvasProposalControls";
import { resolveScreenPreviewData } from "./companion-canvas-data";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";
import { CompanionStructureLeftPane } from "./CompanionStructureLeftPane";
import { CompanionLiveHtmlRightPane } from "./CompanionLiveHtmlRightPane";
import type { CanvasTabItem, CompanionCanvasTabProps } from "./CompanionCanvasTab";
import { useDualPaneViewportState } from "../hooks/useCompanionCanvasState";

const SPLITTER_Z_INDEX = 15;
const EMPTY_ELEMENT_RECTS: Record<string, never> = {};

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
}

function StageLeftStructureSection({ p }: { p: StageProps }) {
  const { vp } = p;
  if (vp.isLeftStructureHidden || vp.isPresenting) return null;
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(p.historicalPreviewTurn?.id);
  const safeBindings = resolveHistoricalReadOnlyStageBindings(p, isHistoricalReadOnly);
  return (
    <CompanionStructureLeftPane
      widthPercent={vp.leftPanePercent}
      isRightPreviewHidden={vp.isRightPreviewHidden}
      structureLens={vp.structureLens}
      onSetLens={vp.setStructureLens}
      onToggleRightPreviewHidden={() => vp.setIsRightPreviewHidden((prev) => !prev)}
      isDraftDirty={Boolean(safeBindings.isDraftDirty)}
      orphanCount={p.orphanCount}
      conflictState={safeBindings.conflictState ?? null}
      onDeployDraft={safeBindings.onDeployDraft}
      onDiscardDraft={safeBindings.onDiscardDraft}
      onResolveConflict={safeBindings.onResolveConflict}
      slides={vp.slides}
      activeSlideIndex={vp.activeSlideIndex}
      isCmsDrawerOpen={vp.isCmsDrawerOpen}
      onSelectSlide={(idx) => {
        vp.setActiveSlideIndex(idx);
        vp.setIsCmsDrawerOpen(true);
      }}
      onCloseCmsDrawer={() => vp.setIsCmsDrawerOpen(false)}
      onUpdateActiveSlide={guardReadOnlyAction(isHistoricalReadOnly, (patch) =>
        vp.setSlides((prev) => prev.map((item, i) => (i === vp.activeSlideIndex ? { ...item, ...patch } : item)))
      )}
    >
      {p.children}
    </CompanionStructureLeftPane>
  );
}

function StageSplitter({ isDragging, onDragStart }: { isDragging: boolean; onDragStart: () => void }) {
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onMouseDown={(e) => {
        e.preventDefault();
        onDragStart();
      }}
      style={{
        width: "4px",
        minWidth: "4px",
        height: "100%",
        cursor: "col-resize",
        backgroundColor: isDragging ? "var(--action-primary)" : "var(--border-default)",
        zIndex: SPLITTER_Z_INDEX,
      }}
    />
  );
}

function useDualPanePreviewModel(p: StageProps) {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  const { activeTab, vp } = p;
  const selectedNode = p.nodes?.find((n) => n.id === p.selectedNodeId) || null;
  const activeScreenData = resolveScreenPreviewData(selectedNode, p.selectedNodeId, {
    kpiRoute: t("canvas.previewSynthKpiRoute", "Route"),
    kpiState: t("canvas.previewSynthKpiState", "Node Status"),
    kpiType: t("canvas.previewSynthKpiType", "Node Type"),
    actionHint: t("canvas.previewSynthActionHint", "Synthesized live preview from WorkbenchNode metadata."),
    ctaOpen: t("canvas.previewSynthCtaOpen", "Open"),
  });
  const activeSlide = vp.slides[vp.activeSlideIndex] || vp.slides[0];
  const lensTag = vp.structureLens === "slide-cms" ? activeSlide.id : "screen";
  const targetKey = `${activeTab.id}::${vp.structureLens}::${p.selectedNodeId || "default"}::${lensTag}`;
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(p.historicalPreviewTurn?.id);
  const historicalHtmlFile = p.historicalPreviewTurn
    ? parseTurnManifestFiles(p.historicalPreviewTurn.manifestJson).find((f) => f.path.endsWith(".html"))
    : undefined;
  const customRawHtml = isHistoricalReadOnly ? historicalHtmlFile?.content : vp.customHtmlByTarget[targetKey];
  const rawKpi = vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount;
  const nextSeedHtml = buildPreviewDocumentHtml({
    structureLens: vp.structureLens,
    activeSlide,
    activeScreenData,
    formattedAmount: typeof rawKpi === "number" ? formatCurrency(Number(rawKpi), "USD") : String(rawKpi),
    labels: {
      lang: locale,
      kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"),
      kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"),
      kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"),
      actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
    },
  });
  return { selectedNode, activeScreenData, activeSlide, targetKey, isHistoricalReadOnly, customRawHtml, nextSeedHtml };
}

function DualPaneRightStage({ p }: { p: StageProps }) {
  const { activeTab, vp } = p;
  const {
    selectedNode, activeScreenData, activeSlide, targetKey, isHistoricalReadOnly, customRawHtml, nextSeedHtml,
  } = useDualPanePreviewModel(p);
  const activeSubState = normalizeFixtureRef(selectedNode?.activeSubState ?? vp.activeSubState);
  const handleToggleInlineCode = () => {
    if (isHistoricalReadOnly) return;
    if (vp.isInlineCodeOpen && vp.dirtyBuffer) { vp.setShowDirtyGuardPrompt(true); return; }
    const seed = customRawHtml || nextSeedHtml;
    vp.setHtmlCodeText(seed); vp.setBaselineHtmlText(seed);
    vp.setShowDirtyGuardPrompt(false); vp.setIsInlineCodeOpen((prev) => !prev);
  };
  const handleApplyHtml = (html: string) => {
    if (isHistoricalReadOnly) return;
    const prevHtml = vp.baselineHtmlText;
    vp.setBaselineHtmlText(html); vp.setShowDirtyGuardPrompt(false);
    vp.setCustomHtmlForTarget(targetKey, html); vp.setIsInlineCodeOpen(false);
    submitInlineHtmlProposal({
      projectId: p.projectId, tabTitle: activeTab.title, nodeId: p.selectedNodeId,
      screenKey: selectedNode?.screenKey, updatedHtml: html, previousHtml: prevHtml,
    });
  };
  return (
    <div style={{ position: "relative", flex: 1, display: "flex", minWidth: "280px", height: "100%", overflow: "hidden" }}>
      <CompanionLiveHtmlRightPane
        projectId={p.projectId} activeTab={activeTab} structureLens={vp.structureLens}
        selectedNodeId={p.selectedNodeId} selectedNodeKey={selectedNode?.screenKey || undefined}
        activeSlide={activeSlide} activeScreenData={activeScreenData} activeSubState={activeSubState}
        onSelectSubState={guardReadOnlyAction(isHistoricalReadOnly, p.onGuardedSubStateSelect)}
        pointerMode={vp.pointerMode}
        onSetPointerMode={(m) => (m === "interact" || !isHistoricalReadOnly) && p.onGuardedModeSelect(m)}
        isInlineCodeOpen={vp.isInlineCodeOpen} dirtyBuffer={vp.dirtyBuffer} showDirtyGuardPrompt={vp.showDirtyGuardPrompt}
        onToggleInlineCode={guardReadOnlyAction(isHistoricalReadOnly, handleToggleInlineCode)}
        onConfirmDiscardDirty={() => { vp.setHtmlCodeText(vp.baselineHtmlText); vp.setShowDirtyGuardPrompt(false); vp.setIsInlineCodeOpen(false); }}
        onCancelCloseEditor={() => vp.setShowDirtyGuardPrompt(false)}
        inlineCodeText={vp.htmlCodeText}
        onChangeInlineCode={guardReadOnlyAction(isHistoricalReadOnly, vp.setHtmlCodeText)}
        viewportPreset={vp.viewportPreset}
        onSetViewportPreset={guardReadOnlyAction(isHistoricalReadOnly, (nextVp) => {
          vp.setViewportPreset(nextVp); if (p.selectedNodeId) p.onUpdateNodeViewport?.(p.selectedNodeId, nextVp);
        })}
        aspectRatio={vp.aspectRatio} onSetAspectRatio={vp.setAspectRatio}
        isPresenting={vp.isPresenting} onTogglePresenting={() => vp.setIsPresenting((prev) => !prev)}
        isSampleDataOpen={p.isSampleDataOpen}
        onToggleSampleData={p.isSampleDataOpen ? p.onCloseSampleData : p.onOpenSampleData}
        isWalkthroughActive={p.isWalkthroughActive} onToggleWalkthrough={p.onToggleWalkthrough}
        isLeftStructureHidden={vp.isLeftStructureHidden}
        onToggleLeftStructureHidden={() => vp.setIsLeftStructureHidden((prev) => !prev)}
        versionMenuNode={p.versionMenuNode} isReadOnly={isHistoricalReadOnly}
        highlightedRegionId={vp.highlightedRegionId} customRawHtml={customRawHtml}
        onApplyHtml={handleApplyHtml} onQuoteText={p.onQuoteText}
        onRegionClick={(id, label) => {
          vp.setHighlightedRegionId(id);
          if (vp.pointerMode !== "interact" && !isHistoricalReadOnly) {
            p.onAttachRegionToChat?.(label.startsWith("@[") ? label : `[${label}]`);
          }
        }}
      />
      <DesignerTalkOverlay
        isOpen={p.isDesignerTalkOpen} tradeoffs={p.activeProposal?.tradeoffs ?? []} elementRects={EMPTY_ELEMENT_RECTS}
        drawerWidth={p.isProposalDrawerOpen ? PROPOSAL_DRAWER_WIDTH_PX : 0}
        isModalOpen={p.isModalOpen} isInlineEditorOpen={vp.isInlineCodeOpen}
        onClose={p.onCloseDesignerTalk} onDecide={p.onDecideTradeoff}
      />
    </div>
  );
}

export function DualPaneHtmlStage(p: StageProps) {
  const { vp, splitContainerRef } = p;
  const showSplitter = !vp.isLeftStructureHidden && !vp.isRightPreviewHidden && !vp.isPresenting;
  return (
    <div
      ref={splitContainerRef}
      className="canvas-dual-pane-stage"
      style={{ flex: 1, display: "flex", width: "100%", height: "100%", overflow: "hidden", position: "relative" }}
    >
      <StageLeftStructureSection p={p} />
      {showSplitter && (
        <StageSplitter isDragging={vp.isDraggingInternalSplit} onDragStart={() => vp.setIsDraggingInternalSplit(true)} />
      )}
      {!vp.isRightPreviewHidden && <DualPaneRightStage p={p} />}
    </div>
  );
}
