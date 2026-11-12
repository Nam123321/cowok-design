// @story 04.1
// @implements AC-03, AC-06
import React, { useRef } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ConflictState } from "../store/flow-workbench-types";
import { SCREEN_PREVIEW_MAP } from "./companion-canvas-data";
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
  activeTabId?: string;
  onTabChange?: (tabId: string, format: CanvasTabItem["format"]) => void;
  selectedNodeId?: string | null;
  isDraftDirty?: boolean;
  conflictState?: ConflictState | null;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
  onAttachRegionToChat?: (chipLabel: string) => void;
  children: React.ReactNode;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const SPLITTER_Z_INDEX = 15;

const NonHtmlTabView: React.FC<{ activeTab: CanvasTabItem; inlineCodeText: string }> = ({
  activeTab,
  inlineCodeText,
}) => {
  if (activeTab.format === "md") {
    return (
      <div data-quotable="true" style={{ flex: 1, padding: "24px", boxSizing: "border-box", overflowY: "auto", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}>
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          # {activeTab.title} — Tài liệu Đặc tả Thiết kế Hệ thống (SSOT)
        </h3>
        <p style={{ color: "var(--text-secondary)", fontSize: "13px", lineHeight: "1.6" }}>
          Bạn có thể bôi đen bất kỳ đoạn văn bản nào trong tài liệu Markdown này để trích dẫn{" "}
          <code>@[Quote]</code> trực tiếp vào khung trò chuyện cowok Agent.
        </p>
      </div>
    );
  }
  if (activeTab.format === "assets") {
    return (
      <div style={{ flex: 1, padding: "24px", boxSizing: "border-box", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", gap: "12px" }}>
        <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>{activeTab.filePath || activeTab.title}</div>
        <div style={{ padding: "24px", borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)" }}>
          <code>[Asset Preview: {activeTab.title}]</code>
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

const DualPaneHtmlStage: React.FC<StageProps> = (p) => {
  const { activeTab, vp, splitContainerRef } = p;
  const activeScreenData = SCREEN_PREVIEW_MAP[p.selectedNodeId || "node-scr-02"] || SCREEN_PREVIEW_MAP["node-scr-02"];
  const activeSlide = vp.slides[vp.activeSlideIndex] || vp.slides[0];
  const showSplitter = !vp.isLeftStructureHidden && !vp.isRightPreviewHidden && !vp.isPresenting;

  return (
    <div ref={splitContainerRef} className="canvas-dual-pane-stage" style={{ flex: 1, display: "flex", width: "100%", height: "100%", overflow: "hidden", position: "relative" }}>
      {!vp.isLeftStructureHidden && !vp.isPresenting && (
        <CompanionStructureLeftPane
          widthPercent={vp.leftPanePercent} isRightPreviewHidden={vp.isRightPreviewHidden} structureLens={vp.structureLens}
          onSetLens={vp.setStructureLens} onToggleRightPreviewHidden={() => vp.setIsRightPreviewHidden((prev) => !prev)}
          isDraftDirty={Boolean(p.isDraftDirty)} conflictState={p.conflictState} onDeployDraft={p.onDeployDraft}
          onDiscardDraft={p.onDiscardDraft} onResolveConflict={p.onResolveConflict} slides={vp.slides}
          activeSlideIndex={vp.activeSlideIndex} isCmsDrawerOpen={vp.isCmsDrawerOpen}
          onSelectSlide={(idx) => { vp.setActiveSlideIndex(idx); vp.setIsCmsDrawerOpen(true); }}
          onCloseCmsDrawer={() => vp.setIsCmsDrawerOpen(false)}
          onUpdateActiveSlide={(patch) => vp.setSlides((prev) => prev.map((item, i) => (i === vp.activeSlideIndex ? { ...item, ...patch } : item)))}
        >
          {p.children}
        </CompanionStructureLeftPane>
      )}
      {showSplitter && (
        <div role="separator" aria-orientation="vertical" onMouseDown={(e) => { e.preventDefault(); vp.setIsDraggingInternalSplit(true); }} style={{ width: "4px", minWidth: "4px", height: "100%", cursor: "col-resize", backgroundColor: vp.isDraggingInternalSplit ? "var(--action-primary)" : "var(--border-default)", zIndex: SPLITTER_Z_INDEX }} />
      )}
      {!vp.isRightPreviewHidden && (
        <CompanionLiveHtmlRightPane
          activeTab={activeTab} structureLens={vp.structureLens} selectedNodeId={p.selectedNodeId}
          activeSlide={activeSlide} activeScreenData={activeScreenData} pointerMode={vp.pointerMode}
          onSetPointerMode={vp.setPointerMode} isInlineCodeOpen={vp.isInlineCodeOpen}
          onToggleInlineCode={() => vp.setIsInlineCodeOpen((prev) => !prev)} inlineCodeText={vp.inlineCodeText}
          onChangeInlineCode={vp.setInlineCodeText} viewportPreset={vp.viewportPreset}
          onSetViewportPreset={vp.setViewportPreset} aspectRatio={vp.aspectRatio} onSetAspectRatio={vp.setAspectRatio}
          isPresenting={vp.isPresenting} onTogglePresenting={() => vp.setIsPresenting((prev) => !prev)}
          isLeftStructureHidden={vp.isLeftStructureHidden} onToggleLeftStructureHidden={() => vp.setIsLeftStructureHidden((prev) => !prev)}
          highlightedRegionId={vp.highlightedRegionId}
          onRegionClick={(id, label) => { vp.setHighlightedRegionId(id); if (vp.pointerMode !== "interact") p.onAttachRegionToChat?.(`[${label}]`); }}
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
