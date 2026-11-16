// @story 04.1
// @implements AC-03, AC-06
import React, { useRef } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ConflictState, WorkbenchNode } from "../store/flow-workbench-types";
import { normalizeFixtureRef, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import { resolveScreenPreviewData } from "./companion-canvas-data";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";
import { CompanionTabBar } from "./CompanionTabBar";
import { CompanionStructureLeftPane } from "./CompanionStructureLeftPane";
import { CompanionLiveHtmlRightPane } from "./CompanionLiveHtmlRightPane";
import {
  useInternalSplitResizer,
  useTabNavigationState,
  useDualPaneViewportState,
} from "../hooks/useCompanionCanvasState";

export interface CanvasTabItem {
  id: string;
  title: string;
  iconName: string;
  format: "dag" | "slide-cms" | "md" | "assets" | "code";
  defaultLens?: "userflow" | "slide-cms";
  filePath?: string;
  isClosable?: boolean;
}

export interface CompanionCanvasTabProps {
  projectId?: string;
  activeTabId?: string;
  onTabChange?: (tabId: string, format: CanvasTabItem["format"]) => void;
  selectedNodeId?: string | null;
  nodes?: WorkbenchNode[];
  isDraftDirty?: boolean;
  orphanCount?: number;
  conflictState?: ConflictState | null;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
  onAttachRegionToChat?: (chipLabel: string) => void;
  onQuoteText?: (text: string) => void;
  onSelectNodeSubState?: (nodeId: string, subState: FixtureVariant) => void;
  onUpdateNodeViewport?: (nodeId: string, vp: ViewportPreset) => void;
  children: React.ReactNode;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const SPLITTER_Z_INDEX = 15;

const NonHtmlTabView: React.FC<{ activeTab: CanvasTabItem; inlineCodeText: string }> = ({
  activeTab,
  inlineCodeText,
}) => {
  const { t } = useTranslation();
  if (activeTab.format === "md") {
    return (
      <div data-quotable="true" style={{ flex: 1, padding: "24px", boxSizing: "border-box", overflowY: "auto", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}>
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          # {activeTab.title} — {t("canvas.mdDocTitleSuffix", "Tài liệu Đặc tả Thiết kế Hệ thống (SSOT)")}
        </h3>
        <p style={{ color: "var(--text-secondary)", fontSize: "13px", lineHeight: "1.6" }}>
          {t("canvas.mdDocQuoteHintPrefix", "Bạn có thể bôi đen bất kỳ đoạn văn bản nào trong tài liệu Markdown này để trích dẫn")}{" "}
          <code>@[Quote]</code> {t("canvas.mdDocQuoteHintSuffix", "trực tiếp vào khung trò chuyện cowok Agent.")}
        </p>
      </div>
    );
  }
  if (activeTab.format === "assets") {
    return (
      <div style={{ flex: 1, padding: "24px", boxSizing: "border-box", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", gap: "12px" }}>
        <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>{activeTab.filePath || activeTab.title}</div>
        <div style={{ padding: "24px", borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)" }}>
          <code>[{t("canvas.assetPreviewPrefix", "Asset Preview")}: {activeTab.title}]</code>
        </div>
      </div>
    );
  }
  return (
    <div style={{ flex: 1, padding: "20px", boxSizing: "border-box", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: "12px", overflowY: "auto" }}>
      <div style={{ color: "var(--text-muted)", marginBottom: "8px" }}>// {activeTab.filePath || activeTab.title}</div>
      <pre style={{ margin: 0, lineHeight: "1.55" }}>{inlineCodeText}</pre>
    </div>
  );
};

interface StageProps extends CompanionCanvasTabProps {
  activeTab: CanvasTabItem;
  vp: ReturnType<typeof useDualPaneViewportState>;
  splitContainerRef: React.RefObject<HTMLDivElement | null>;
}

const StageLeftStructureSection: React.FC<{ p: StageProps }> = ({ p }) => {
  const { vp } = p;
  if (vp.isLeftStructureHidden || vp.isPresenting) return null;
  return (
    <CompanionStructureLeftPane
      widthPercent={vp.leftPanePercent} isRightPreviewHidden={vp.isRightPreviewHidden} structureLens={vp.structureLens}
      onSetLens={vp.setStructureLens} onToggleRightPreviewHidden={() => vp.setIsRightPreviewHidden((prev) => !prev)}
      isDraftDirty={Boolean(p.isDraftDirty)} orphanCount={p.orphanCount} conflictState={p.conflictState} onDeployDraft={p.onDeployDraft}
      onDiscardDraft={p.onDiscardDraft} onResolveConflict={p.onResolveConflict} slides={vp.slides}
      activeSlideIndex={vp.activeSlideIndex} isCmsDrawerOpen={vp.isCmsDrawerOpen}
      onSelectSlide={(idx) => { vp.setActiveSlideIndex(idx); vp.setIsCmsDrawerOpen(true); }}
      onCloseCmsDrawer={() => vp.setIsCmsDrawerOpen(false)}
      onUpdateActiveSlide={(patch) => vp.setSlides((prev) => prev.map((item, i) => (i === vp.activeSlideIndex ? { ...item, ...patch } : item)))}
    >
      {p.children}
    </CompanionStructureLeftPane>
  );
};

const StageSplitter: React.FC<{ isDragging: boolean; onDragStart: () => void }> = ({ isDragging, onDragStart }) => (
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

const DualPaneHtmlStage: React.FC<StageProps> = (p) => {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  const { activeTab, vp, splitContainerRef } = p;
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
  const customRawHtml = vp.customHtmlByTarget[targetKey];
  const showSplitter = !vp.isLeftStructureHidden && !vp.isRightPreviewHidden && !vp.isPresenting;

  const seedLabels = {
    lang: locale,
    kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"),
    kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"),
    kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"),
    actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
  };
  const nextSeedHtml = buildPreviewDocumentHtml({
    structureLens: vp.structureLens,
    activeSlide,
    activeScreenData,
    formattedAmount: typeof (vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount) === "number"
      ? formatCurrency(Number(vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount), "USD")
      : String(vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount),
    labels: seedLabels,
  });
  const activeSubState = normalizeFixtureRef(selectedNode?.activeSubState ?? vp.activeSubState);

  const handleToggleInlineCode = () => {
    if (vp.isInlineCodeOpen && vp.dirtyBuffer) {
      vp.setShowDirtyGuardPrompt(true);
      return;
    }
    const seed = customRawHtml || nextSeedHtml;
    vp.setHtmlCodeText(seed);
    vp.setBaselineHtmlText(seed);
    vp.setShowDirtyGuardPrompt(false);
    vp.setIsInlineCodeOpen((prev) => !prev);
  };

  return (
    <div ref={splitContainerRef} className="canvas-dual-pane-stage" style={{ flex: 1, display: "flex", width: "100%", height: "100%", overflow: "hidden", position: "relative" }}>
      <StageLeftStructureSection p={p} />
      {showSplitter && <StageSplitter isDragging={vp.isDraggingInternalSplit} onDragStart={() => vp.setIsDraggingInternalSplit(true)} />}
      {!vp.isRightPreviewHidden && (
        <CompanionLiveHtmlRightPane
          projectId={p.projectId}
          activeTab={activeTab} structureLens={vp.structureLens} selectedNodeId={p.selectedNodeId}
          selectedNodeKey={selectedNode?.screenKey || undefined} activeSlide={activeSlide} activeScreenData={activeScreenData}
          activeSubState={activeSubState}
          onSelectSubState={(v) => {
            vp.setActiveSubState(v);
            if (p.selectedNodeId) p.onSelectNodeSubState?.(p.selectedNodeId, v);
          }}
          pointerMode={vp.pointerMode} onSetPointerMode={vp.setPointerMode}
          isInlineCodeOpen={vp.isInlineCodeOpen} dirtyBuffer={vp.dirtyBuffer} showDirtyGuardPrompt={vp.showDirtyGuardPrompt}
          onToggleInlineCode={handleToggleInlineCode}
          onConfirmDiscardDirty={() => {
            vp.setHtmlCodeText(vp.baselineHtmlText);
            vp.setShowDirtyGuardPrompt(false);
            vp.setIsInlineCodeOpen(false);
          }}
          onCancelCloseEditor={() => vp.setShowDirtyGuardPrompt(false)}
          inlineCodeText={vp.htmlCodeText} onChangeInlineCode={vp.setHtmlCodeText}
          viewportPreset={vp.viewportPreset}
          onSetViewportPreset={(nextVp) => {
            vp.setViewportPreset(nextVp);
            if (p.selectedNodeId) p.onUpdateNodeViewport?.(p.selectedNodeId, nextVp);
          }}
          aspectRatio={vp.aspectRatio}
          onSetAspectRatio={vp.setAspectRatio} isPresenting={vp.isPresenting} onTogglePresenting={() => vp.setIsPresenting((prev) => !prev)}
          isLeftStructureHidden={vp.isLeftStructureHidden} onToggleLeftStructureHidden={() => vp.setIsLeftStructureHidden((prev) => !prev)}
          highlightedRegionId={vp.highlightedRegionId} customRawHtml={customRawHtml}
          onApplyHtml={(html) => {
            vp.setBaselineHtmlText(html);
            vp.setShowDirtyGuardPrompt(false);
            vp.setCustomHtmlForTarget(targetKey, html);
            vp.setIsInlineCodeOpen(false);
          }}
          onQuoteText={p.onQuoteText}
          onRegionClick={(id, label) => {
            vp.setHighlightedRegionId(id);
            if (vp.pointerMode !== "interact") {
              const formatted = label.startsWith("@[") ? label : `[${label}]`;
              p.onAttachRegionToChat?.(formatted);
            }
          }}
        />
      )}
    </div>
  );
};

export const CompanionCanvasTab: React.FC<CompanionCanvasTabProps> = (props) => {
  const { t } = useTranslation();
  const splitContainerRef = useRef<HTMLDivElement>(null);
  const nav = useTabNavigationState(props.activeTabId, props.onTabChange);
  const vp = useDualPaneViewportState(nav.activeTab);
  useInternalSplitResizer(vp.isDraggingInternalSplit, vp.setIsDraggingInternalSplit, splitContainerRef, vp.setLeftPanePercent);
  const isHtmlDesignTab = nav.activeTab.format === "dag" || nav.activeTab.format === "slide-cms";

  return (
    <div className="companion-canvas-tab-container" aria-label={t("canvas.companionContainer", "Companion Canvas Studio")} style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column", overflow: "hidden", backgroundColor: "var(--bg-canvas)" }}>
      <CompanionTabBar tabs={nav.tabs} activeTabId={nav.activeTab.id} isNewTabMenuOpen={nav.isNewTabMenuOpen} onToggleNewTabMenu={() => nav.setIsNewTabMenuOpen((prev) => !prev)} onTabSelect={nav.handleTabSelect} onCloseTab={nav.handleClose} onCreateTab={nav.handleCreateTab} />
      {isHtmlDesignTab ? (
        <DualPaneHtmlStage {...props} activeTab={nav.activeTab} vp={vp} splitContainerRef={splitContainerRef} />
      ) : (
        <NonHtmlTabView activeTab={nav.activeTab} inlineCodeText={vp.inlineCodeText} />
      )}
    </div>
  );
};
