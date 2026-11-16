// @story 04.1, 05.1
// @implements AC-03, AC-04, AC-06, [EC-STORYBOARD-001] Dirty Inline Editor Guard, 4-Path Launchpad & Sample Data Studio
import React, { useCallback, useEffect, useRef, useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ConflictState, WorkbenchNode } from "../store/flow-workbench-types";
import { normalizeFixtureRef, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import { resolveScreenPreviewData } from "./companion-canvas-data";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";
import { CompanionTabBar } from "./CompanionTabBar";
import { CompanionStructureLeftPane } from "./CompanionStructureLeftPane";
import { CompanionLiveHtmlRightPane } from "./CompanionLiveHtmlRightPane";
import { ProjectLaunchpadModal } from "./ProjectLaunchpadModal";
import { SampleDataStudioModal, type SampleDataPreset } from "./SampleDataStudioModal";
import {
  useInternalSplitResizer,
  useTabNavigationState,
  useDualPaneViewportState,
} from "../hooks/useCompanionCanvasState";

const ENTRY_PATH_BY_LAUNCHPAD = {
  prd: "prompt_prd",
  templates: "starter_template",
  design: "design_import",
  git: "git_codebase",
} as const;

const MAX_LAUNCHPAD_SLUG_LEN = 48;
const MAX_LAUNCHPAD_NAME_LEN = 80;
const MAX_LAUNCHPAD_BRIEF_LEN = 2000;
const SLUG_TIMESTAMP_RADIX = 36;

async function submitLaunchpadProject(
  pathType: "prd" | "templates" | "design" | "git",
  data: Record<string, unknown>,
  activeProjectId?: string
): Promise<void> {
  const rawTitle = String(data.prdFileName || data.templateId || data.designUrl || data.gitRepoUrl || `cowok-${pathType}`);
  const slug = rawTitle.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, MAX_LAUNCHPAD_SLUG_LEN) || `cowok-${pathType}`;
  const brief = String(data.prdText || data.clipboardSnippet || data.designUrl || data.gitRepoUrl || `Initialized from ${pathType}`);
  try {
    await apiHarnessDaemon.createProjectFromLaunchpad({
      slug: `${slug}-${Date.now().toString(SLUG_TIMESTAMP_RADIX)}`,
      name: rawTitle.slice(0, MAX_LAUNCHPAD_NAME_LEN) || "New Launchpad Workspace",
      product_brief: brief.slice(0, MAX_LAUNCHPAD_BRIEF_LEN),
      provenance: "scratch",
      entryPath: ENTRY_PATH_BY_LAUNCHPAD[pathType],
      templateId: typeof data.templateId === "string" ? data.templateId : undefined,
      designSourceUrl: typeof data.designUrl === "string" && data.designUrl ? data.designUrl : undefined,
      gitRepoUrl: typeof data.gitRepoUrl === "string" && data.gitRepoUrl ? data.gitRepoUrl : undefined,
      gitBranch: typeof data.gitBranch === "string" && data.gitBranch ? data.gitBranch : undefined,
    });
  } catch {
    if (activeProjectId) void activeProjectId;
  }
}

async function applyStudioSampleFixture(
  projectId: string | undefined,
  nodeId: string | null | undefined,
  preset: SampleDataPreset,
  json: string
): Promise<void> {
  if (!projectId || !nodeId) return;
  try {
    await apiScreenFixture.saveFixture(projectId, nodeId, {
      variant: "DEFAULT",
      payloadJson: json,
      presetKey: preset,
    });
  } catch {
    /* keep local state responsive if daemon offline */
  }
}

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
  onSelectNode?: (nodeId: string) => void;
  nodes?: WorkbenchNode[];
  isDraftDirty?: boolean;
  orphanCount?: number;
  conflictState?: ConflictState | null;
  ambiguityScore?: number;
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
      <div
        data-quotable="true"
        style={{
          flex: 1,
          padding: "24px",
          boxSizing: "border-box",
          overflowY: "auto",
          backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)",
        }}
      >
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          # {activeTab.title} — {t("canvas.mdDocTitleSuffix", "Tài liệu Đặc tả Thiết kế Hệ thống (SSOT)")}
        </h3>
        <p style={{ color: "var(--text-secondary)", fontSize: "13px", lineHeight: "1.6" }}>
          {t(
            "canvas.mdDocQuoteHintPrefix",
            "Bạn có thể bôi đen bất kỳ đoạn văn bản nào trong tài liệu Markdown này để trích dẫn"
          )}{" "}
          <code>@[Quote]</code>{" "}
          {t("canvas.mdDocQuoteHintSuffix", "trực tiếp vào khung trò chuyện cowok Agent.")}
        </p>
      </div>
    );
  }
  if (activeTab.format === "assets") {
    return (
      <div
        style={{
          flex: 1,
          padding: "24px",
          boxSizing: "border-box",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "var(--bg-canvas)",
          color: "var(--text-primary)",
          gap: "12px",
        }}
      >
        <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {activeTab.filePath || activeTab.title}
        </div>
        <div
          style={{
            padding: "24px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border-default)",
            backgroundColor: "var(--bg-surface)",
          }}
        >
          <code>
            [{t("canvas.assetPreviewPrefix", "Asset Preview")}: {activeTab.title}]
          </code>
        </div>
      </div>
    );
  }
  return (
    <div
      style={{
        flex: 1,
        padding: "20px",
        boxSizing: "border-box",
        backgroundColor: "var(--bg-canvas)",
        color: "var(--text-primary)",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        overflowY: "auto",
      }}
    >
      <div style={{ color: "var(--text-muted)", marginBottom: "8px" }}>// {activeTab.filePath || activeTab.title}</div>
      <pre
        data-testid="non-html-code-pre"
        style={{
          margin: 0,
          lineHeight: "1.55",
          overflowX: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {inlineCodeText}
      </pre>
    </div>
  );
};

interface StageProps extends CompanionCanvasTabProps {
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
  onGuardedNodeSelect: (nodeId: string) => void;
  onGuardedSubStateSelect: (subState: FixtureVariant) => void;
  onGuardedModeSelect: (mode: "interact" | "click-dom" | "drag-box") => void;
}

const StageLeftStructureSection: React.FC<{ p: StageProps }> = ({ p }) => {
  const { vp } = p;
  if (vp.isLeftStructureHidden || vp.isPresenting) return null;
  return (
    <CompanionStructureLeftPane
      widthPercent={vp.leftPanePercent}
      isRightPreviewHidden={vp.isRightPreviewHidden}
      structureLens={vp.structureLens}
      onSetLens={vp.setStructureLens}
      onToggleRightPreviewHidden={() => vp.setIsRightPreviewHidden((prev) => !prev)}
      isDraftDirty={Boolean(p.isDraftDirty)}
      orphanCount={p.orphanCount}
      conflictState={p.conflictState}
      onDeployDraft={p.onDeployDraft}
      onDiscardDraft={p.onDiscardDraft}
      onResolveConflict={p.onResolveConflict}
      slides={vp.slides}
      activeSlideIndex={vp.activeSlideIndex}
      isCmsDrawerOpen={vp.isCmsDrawerOpen}
      onSelectSlide={(idx) => {
        vp.setActiveSlideIndex(idx);
        vp.setIsCmsDrawerOpen(true);
      }}
      onCloseCmsDrawer={() => vp.setIsCmsDrawerOpen(false)}
      onUpdateActiveSlide={(patch) =>
        vp.setSlides((prev) => prev.map((item, i) => (i === vp.activeSlideIndex ? { ...item, ...patch } : item)))
      }
    >
      {p.children}
    </CompanionStructureLeftPane>
  );
};

const StageSplitter: React.FC<{ isDragging: boolean; onDragStart: () => void }> = ({
  isDragging,
  onDragStart,
}) => (
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
    formattedAmount:
      typeof (vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount) === "number"
        ? formatCurrency(
            Number(vp.structureLens === "slide-cms" ? activeSlide.kpi1 : activeScreenData.amount),
            "USD"
          )
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
    <div
      ref={splitContainerRef}
      className="canvas-dual-pane-stage"
      style={{
        flex: 1,
        display: "flex",
        width: "100%",
        height: "100%",
        overflow: "hidden",
        position: "relative",
      }}
    >
      <StageLeftStructureSection p={p} />
      {showSplitter && (
        <StageSplitter
          isDragging={vp.isDraggingInternalSplit}
          onDragStart={() => vp.setIsDraggingInternalSplit(true)}
        />
      )}
      {!vp.isRightPreviewHidden && (
        <CompanionLiveHtmlRightPane
          projectId={p.projectId}
          activeTab={activeTab}
          structureLens={vp.structureLens}
          selectedNodeId={p.selectedNodeId}
          selectedNodeKey={selectedNode?.screenKey || undefined}
          activeSlide={activeSlide}
          activeScreenData={activeScreenData}
          activeSubState={activeSubState}
          onSelectSubState={(v) => {
            p.onGuardedSubStateSelect(v);
          }}
          pointerMode={vp.pointerMode}
          onSetPointerMode={(m) => {
            p.onGuardedModeSelect(m);
          }}
          isInlineCodeOpen={vp.isInlineCodeOpen}
          dirtyBuffer={vp.dirtyBuffer}
          showDirtyGuardPrompt={vp.showDirtyGuardPrompt}
          onToggleInlineCode={handleToggleInlineCode}
          onConfirmDiscardDirty={() => {
            vp.setHtmlCodeText(vp.baselineHtmlText);
            vp.setShowDirtyGuardPrompt(false);
            vp.setIsInlineCodeOpen(false);
          }}
          onCancelCloseEditor={() => vp.setShowDirtyGuardPrompt(false)}
          inlineCodeText={vp.htmlCodeText}
          onChangeInlineCode={vp.setHtmlCodeText}
          viewportPreset={vp.viewportPreset}
          onSetViewportPreset={(nextVp) => {
            vp.setViewportPreset(nextVp);
            if (p.selectedNodeId) p.onUpdateNodeViewport?.(p.selectedNodeId, nextVp);
          }}
          aspectRatio={vp.aspectRatio}
          onSetAspectRatio={vp.setAspectRatio}
          isPresenting={vp.isPresenting}
          onTogglePresenting={() => vp.setIsPresenting((prev) => !prev)}
          isSampleDataOpen={p.isSampleDataOpen}
          onToggleSampleData={p.isSampleDataOpen ? p.onCloseSampleData : p.onOpenSampleData}
          isWalkthroughActive={p.isWalkthroughActive}
          onToggleWalkthrough={p.onToggleWalkthrough}
          isLeftStructureHidden={vp.isLeftStructureHidden}
          onToggleLeftStructureHidden={() => vp.setIsLeftStructureHidden((prev) => !prev)}
          highlightedRegionId={vp.highlightedRegionId}
          customRawHtml={customRawHtml}
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

  // Modal states
  const [isLaunchpadOpen, setIsLaunchpadOpen] = useState(false);
  const [isSampleDataOpen, setIsSampleDataOpen] = useState(false);
  const [isWalkthroughActive, setIsWalkthroughActive] = useState(false);
  const [walkthroughIndex, setWalkthroughIndex] = useState(0);

  // [EC-STORYBOARD-001] Pending Guarded Action for Dirty Buffer Guard
  const [pendingGuardedAction, setPendingGuardedAction] = useState<(() => void) | null>(null);

  const isHtmlDesignTab = nav.activeTab.format === "dag" || nav.activeTab.format === "slide-cms";

  // Customer Walkthrough automated timeline progression
  useEffect(() => {
    if (!isWalkthroughActive || !props.nodes || props.nodes.length === 0) return;
    const timer = setInterval(() => {
      setWalkthroughIndex((prev) => {
        const next = (prev + 1) % props.nodes!.length;
        const targetNode = props.nodes![next];
        if (targetNode && props.onSelectNode) {
          props.onSelectNode(targetNode.id);
        }
        return next;
      });
    }, 2500);
    return () => clearInterval(timer);
  }, [isWalkthroughActive, props.nodes, props.onSelectNode]);

  // Intercept action with [EC-STORYBOARD-001] Dirty Guard
  const runWithDirtyGuard = useCallback(
    (action: () => void) => {
      if (vp.dirtyBuffer) {
        setPendingGuardedAction(() => action);
        vp.setShowDirtyGuardPrompt(true);
      } else {
        action();
      }
    },
    [vp]
  );

  const handleGuardedTabSelect = useCallback(
    (tabId: string, format: CanvasTabItem["format"]) => {
      runWithDirtyGuard(() => nav.handleTabSelect(tabId, format));
    },
    [nav, runWithDirtyGuard]
  );

  const handleGuardedNodeSelect = useCallback(
    (nodeId: string) => {
      runWithDirtyGuard(() => props.onSelectNode?.(nodeId));
    },
    [props, runWithDirtyGuard]
  );

  const handleGuardedSubStateSelect = useCallback(
    (subState: FixtureVariant) => {
      runWithDirtyGuard(() => {
        vp.setActiveSubState(subState);
        if (props.selectedNodeId) {
          props.onSelectNodeSubState?.(props.selectedNodeId, subState);
        }
      });
    },
    [props, runWithDirtyGuard, vp]
  );

  const handleGuardedModeSelect = useCallback(
    (mode: "interact" | "click-dom" | "drag-box") => {
      runWithDirtyGuard(() => vp.setPointerMode(mode));
    },
    [runWithDirtyGuard, vp]
  );

  const handleConfirmSavePatch = useCallback(() => {
    vp.setBaselineHtmlText(vp.htmlCodeText);
    vp.setShowDirtyGuardPrompt(false);
    vp.setIsInlineCodeOpen(false);
    if (pendingGuardedAction) {
      pendingGuardedAction();
      setPendingGuardedAction(null);
    }
  }, [pendingGuardedAction, vp]);

  const handleConfirmDiscardChanges = useCallback(() => {
    vp.setHtmlCodeText(vp.baselineHtmlText);
    vp.setShowDirtyGuardPrompt(false);
    vp.setIsInlineCodeOpen(false);
    if (pendingGuardedAction) {
      pendingGuardedAction();
      setPendingGuardedAction(null);
    }
  }, [pendingGuardedAction, vp]);

  const handleCancelDirtyGuard = useCallback(() => {
    vp.setShowDirtyGuardPrompt(false);
    setPendingGuardedAction(null);
  }, [vp]);

  const isAmbiguityLocked = props.ambiguityScore !== undefined && props.ambiguityScore >= 0.45;
  const isConflictLocked = Boolean(props.conflictState);

  return (
    <div
      className="companion-canvas-tab-container"
      aria-label={t("canvas.companionContainer", "Companion Canvas Studio")}
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        backgroundColor: "var(--bg-canvas)",
        position: "relative",
      }}
    >
      {/* Top Banner: [AC-03] Ambiguity Execution Lock */}
      {isAmbiguityLocked && (
        <div
          data-testid="ambiguity-execution-lock"
          style={{
            padding: "5px 14px",
            backgroundColor: "var(--state-warning-soft)",
            color: "var(--state-warning)",
            borderBottom: "1px solid var(--state-warning)",
            fontSize: "11px",
            fontWeight: 600,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            zIndex: 20,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="lock" size={13} strokeWidth={1.85} color="currentColor" />
            <span>
              {t("sdi.ambiguityLockFull", "Khóa Thực Thi: Điểm mơ hồ {{score}} ≥ 0.45. Cần làm rõ ý định trước khi sinh mã.").replace("{{score}}", String(props.ambiguityScore))}
            </span>
          </div>
        </div>
      )}

      {/* Top Banner: [AC-04] DAG Conflict Lock */}
      {isConflictLocked && (
        <div
          data-testid="dag-conflict-lock"
          style={{
            padding: "5px 14px",
            backgroundColor: "var(--state-danger-soft)",
            color: "var(--state-danger)",
            borderBottom: "1px solid var(--state-danger)",
            fontSize: "11px",
            fontWeight: 600,
            display: "flex",
            alignItems: "center",
            gap: "6px",
            zIndex: 20,
          }}
        >
          <BrandIcon name="warning" size={13} strokeWidth={1.85} color="currentColor" />
          <span>{t("flow.dagConflictLockMessage", "Cảnh báo Xung Đột DAG Chưa Giải Quyết: Các thao tác triển khai bị khóa tạm thời.")}</span>
        </div>
      )}

      {/* [EC-STORYBOARD-001] Global Dirty Editor Guard Modal */}
      {vp.showDirtyGuardPrompt && (
        <div
          role="dialog"
          aria-modal="true"
          data-testid="dirty-buffer-guard-modal"
          style={{
            position: "absolute",
            inset: 0,
            zIndex: 60,
            backgroundColor: "var(--bg-overlay)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            padding: "16px",
          }}
        >
          <div
            style={{
              width: "100%",
              maxWidth: "420px",
              backgroundColor: "var(--bg-surface)",
              border: "1px solid var(--border-default)",
              borderRadius: "var(--radius-lg)",
              padding: "18px 20px",
              boxShadow: "var(--shadow-lg)",
              display: "flex",
              flexDirection: "column",
              gap: "12px",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <BrandIcon name="warning" size={18} strokeWidth={1.85} color="var(--state-warning)" />
              <h3 style={{ margin: 0, fontSize: "14px", fontWeight: 600, color: "var(--text-primary)" }}>
                {t("canvas.dirtyGuardTitle", "Thay Đổi Chưa Lưu Trong Trình Sửa Mã")}
              </h3>
            </div>
            <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)", lineHeight: "1.5" }}>
              {t(
                "canvas.dirtyGuardBody",
                "Bạn có thay đổi chưa lưu trong Inline </> Editor. Bạn muốn Lưu bản vá (Save Patch) hay Hủy bỏ thay đổi (Discard Changes) trước khi chuyển đổi?"
              )}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
              <button
                type="button"
                data-testid="dirty-guard-cancel-btn"
                onClick={handleCancelDirtyGuard}
                style={{
                  padding: "6px 12px",
                  fontSize: "11px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "transparent",
                  color: "var(--text-secondary)",
                  cursor: "pointer",
                }}
              >
                {t("common.cancel", "Hủy Bỏ")}
              </button>
              <button
                type="button"
                data-testid="dirty-guard-discard-btn"
                onClick={handleConfirmDiscardChanges}
                style={{
                  padding: "6px 12px",
                  fontSize: "11px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "var(--bg-subtle)",
                  color: "var(--state-danger)",
                  cursor: "pointer",
                }}
              >
                {t("canvas.discardChangesBtn", "Hủy Thay Đổi (Discard)")}
              </button>
              <button
                type="button"
                data-testid="dirty-guard-save-btn"
                onClick={handleConfirmSavePatch}
                style={{
                  padding: "6px 14px",
                  fontSize: "11px",
                  fontWeight: 600,
                  borderRadius: "var(--radius-xs)",
                  border: "none",
                  backgroundColor: "var(--action-primary)",
                  color: "var(--action-primary-fg)",
                  cursor: "pointer",
                }}
              >
                {t("canvas.savePatchBtn", "Lưu Bản Vá (Save Patch)")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Tab Bar */}
      <CompanionTabBar
        tabs={nav.tabs}
        activeTabId={nav.activeTab.id}
        isNewTabMenuOpen={nav.isNewTabMenuOpen}
        onToggleNewTabMenu={() => nav.setIsNewTabMenuOpen((prev) => !prev)}
        onTabSelect={handleGuardedTabSelect}
        onCloseTab={nav.handleClose}
        onCreateTab={nav.handleCreateTab}
      />

      {isHtmlDesignTab ? (
        <DualPaneHtmlStage
          {...props}
          activeTab={nav.activeTab}
          vp={vp}
          splitContainerRef={splitContainerRef}
          isLaunchpadOpen={isLaunchpadOpen}
          onOpenLaunchpad={() => setIsLaunchpadOpen(true)}
          onCloseLaunchpad={() => setIsLaunchpadOpen(false)}
          isSampleDataOpen={isSampleDataOpen}
          onOpenSampleData={() => setIsSampleDataOpen(true)}
          onCloseSampleData={() => setIsSampleDataOpen(false)}
          isWalkthroughActive={isWalkthroughActive}
          onToggleWalkthrough={() => setIsWalkthroughActive((v) => !v)}
          onGuardedNodeSelect={handleGuardedNodeSelect}
          onGuardedSubStateSelect={handleGuardedSubStateSelect}
          onGuardedModeSelect={handleGuardedModeSelect}
        />
      ) : (
        <NonHtmlTabView activeTab={nav.activeTab} inlineCodeText={vp.inlineCodeText} />
      )}

      {/* Modals */}
      <ProjectLaunchpadModal
        isOpen={isLaunchpadOpen}
        onClose={() => setIsLaunchpadOpen(false)}
        onLaunchPath={(pathType, data) => {
          void submitLaunchpadProject(pathType, data, props.projectId);
        }}
      />

      <SampleDataStudioModal
        isOpen={isSampleDataOpen}
        onClose={() => setIsSampleDataOpen(false)}
        onApplyFixture={(preset: SampleDataPreset, json: string) => {
          void applyStudioSampleFixture(props.projectId, props.selectedNodeId, preset, json);
        }}
      />
    </div>
  );
};
