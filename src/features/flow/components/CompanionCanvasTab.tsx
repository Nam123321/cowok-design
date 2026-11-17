// @story 04.1, 05.1
// @implements AC-03, AC-04, AC-06, [EC-STORYBOARD-001] Dirty Inline Editor Guard, 4-Path Launchpad & Sample Data Studio
import React, { useCallback, useEffect, useRef, useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ConflictState, WorkbenchNode } from "../store/flow-workbench-types";
import type { FixtureVariant, ViewportPreset } from "../../../lib/validations/screen-fixture";
import type { ChatArtifactSnapshotResponse } from "../../../lib/validations/version-snapshot";
import {
  CanvasSnapshotDrawerAndModalHost,
  HistoricalPreviewBanner,
  VersionPublishMenuButton,
  useSnapshotWorkspaceController,
} from "../../snapshots/components/VersionHistoryDrawer";
import {
  guardReadOnlyAction,
  isHistoricalPreviewReadOnly,
  resolveHistoricalReadOnlyStageBindings,
} from "../../snapshots/utils/snapshot-engine";
import {
  CanvasProposalDrawerHost,
} from "../../proposals/components/CanvasProposalControls";
import { useNodeProposalController } from "../../proposals/hooks/useNodeProposalController";
import { CompanionTabBar } from "./CompanionTabBar";
import { NonHtmlTabView } from "./CompanionNonHtmlTabView";
import { DualPaneHtmlStage } from "./CompanionDualPaneStage";
import { ProjectLaunchpadModal } from "./ProjectLaunchpadModal";
import { SampleDataStudioModal, type SampleDataPreset } from "./SampleDataStudioModal";
import {
  ReadOnlyCanvasContext,
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
  const variantMap: Record<SampleDataPreset, FixtureVariant> = {
    HAPPY: "DEFAULT",
    EMPTY: "EMPTY",
    LONG_TEXT: "DEFAULT",
    VALIDATION_ERROR: "ERROR",
  };
  const variant = variantMap[preset] || "DEFAULT";
  window.dispatchEvent(new CustomEvent("cowok:sample-fixture-applied", {
    detail: { projectId, nodeId, preset, variant, fixtureJson: json },
  }));
  if (!projectId || !nodeId) return;
  try {
    await apiScreenFixture.saveFixture(projectId, nodeId, {
      variant,
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
  proposalController?: ReturnType<typeof useNodeProposalController>;
  children: React.ReactNode;
}

const WALKTHROUGH_INTERVAL_MS = 2500;
const AMBIGUITY_LOCK_THRESHOLD = 0.45;

const CompanionLockBanners: React.FC<{ ambiguityScore?: number; conflictState?: ConflictState | null }> = ({ ambiguityScore, conflictState }) => {
  const { t } = useTranslation();
  const isAmbiguityLocked = ambiguityScore !== undefined && ambiguityScore >= AMBIGUITY_LOCK_THRESHOLD;
  if (!isAmbiguityLocked && !conflictState) return null;

  return (
    <>
      {isAmbiguityLocked && (
        <div data-testid="ambiguity-execution-lock" style={{ padding: "5px 14px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", borderBottom: "1px solid var(--state-warning)", fontSize: "11px", fontWeight: 600, display: "flex", alignItems: "center", justifyContent: "space-between", zIndex: 20 }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="lock" size={13} strokeWidth={1.85} color="currentColor" />
            <span>{t("sdi.ambiguityLockFull", "Khóa Thực Thi: Điểm mơ hồ {{score}} ≥ 0.45. Cần làm rõ ý định trước khi sinh mã.").replace("{{score}}", String(ambiguityScore))}</span>
          </div>
        </div>
      )}
      {Boolean(conflictState) && (
        <div data-testid="dag-conflict-lock" style={{ padding: "5px 14px", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", borderBottom: "1px solid var(--state-danger)", fontSize: "11px", fontWeight: 600, display: "flex", alignItems: "center", gap: "6px", zIndex: 20 }}>
          <BrandIcon name="warning" size={13} strokeWidth={1.85} color="currentColor" />
          <span>{t("flow.dagConflictLockMessage", "Cảnh báo Xung Đột DAG Chưa Giải Quyết: Các thao tác triển khai bị khóa tạm thời.")}</span>
        </div>
      )}
    </>
  );
};

const CompanionDirtyGuardModal: React.FC<{ isOpen: boolean; onCancel: () => void; onDiscard: () => void; onSave: () => void }> = ({ isOpen, onCancel, onDiscard, onSave }) => {
  const { t } = useTranslation();
  if (!isOpen) return null;
  return (
    <div role="dialog" aria-modal="true" data-testid="dirty-buffer-guard-modal" style={{ position: "absolute", inset: 0, zIndex: 60, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px" }}>
      <div style={{ width: "100%", maxWidth: "420px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", padding: "18px 20px", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", gap: "12px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <BrandIcon name="warning" size={18} strokeWidth={1.85} color="var(--state-warning)" />
          <h3 style={{ margin: 0, fontSize: "14px", fontWeight: 600, color: "var(--text-primary)" }}>{t("canvas.dirtyGuardTitle", "Thay Đổi Chưa Lưu Trong Trình Sửa Mã")}</h3>
        </div>
        <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)", lineHeight: "1.5" }}>{t("canvas.dirtyGuardBody", "Bạn có thay đổi chưa lưu trong Inline </> Editor. Bạn muốn Lưu bản vá (Save Patch) hay Hủy bỏ thay đổi (Discard Changes) trước khi chuyển đổi?")}</p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
          <button type="button" data-testid="dirty-guard-cancel-btn" onClick={onCancel} style={{ padding: "6px 12px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>{t("common.cancel", "Hủy Bỏ")}</button>
          <button type="button" data-testid="dirty-guard-discard-btn" onClick={onDiscard} style={{ padding: "6px 12px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)", color: "var(--state-danger)", cursor: "pointer" }}>{t("canvas.discardChangesBtn", "Hủy Thay Đổi (Discard)")}</button>
          <button type="button" data-testid="dirty-guard-save-btn" onClick={onSave} style={{ padding: "6px 14px", fontSize: "11px", fontWeight: 600, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>{t("canvas.savePatchBtn", "Lưu Bản Vá (Save Patch)")}</button>
        </div>
      </div>
    </div>
  );
};

function useCanvasGuardedActions(
  vp: ReturnType<typeof useDualPaneViewportState>,
  nav: ReturnType<typeof useTabNavigationState>,
  props: Pick<CompanionCanvasTabProps, "selectedNodeId" | "onSelectNode" | "onSelectNodeSubState">
) {
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);

  const runWithDirtyGuard = useCallback((action: () => void) => {
    if (vp.dirtyBuffer) {
      setPendingAction(() => action);
      vp.setShowDirtyGuardPrompt(true);
    } else {
      action();
    }
  }, [vp]);

  const flushPending = useCallback((savePatch: boolean) => {
    if (savePatch) vp.setBaselineHtmlText(vp.htmlCodeText);
    else vp.setHtmlCodeText(vp.baselineHtmlText);
    vp.setShowDirtyGuardPrompt(false);
    vp.setIsInlineCodeOpen(false);
    if (pendingAction) {
      pendingAction();
      setPendingAction(null);
    }
  }, [pendingAction, vp]);

  return {
    handleGuardedTabSelect: useCallback((tabId: string, format: CanvasTabItem["format"]) => runWithDirtyGuard(() => nav.handleTabSelect(tabId, format)), [nav, runWithDirtyGuard]),
    handleGuardedNodeSelect: useCallback((nodeId: string) => runWithDirtyGuard(() => props.onSelectNode?.(nodeId)), [props, runWithDirtyGuard]),
    handleGuardedSubStateSelect: useCallback((subState: FixtureVariant) => runWithDirtyGuard(() => {
      vp.setActiveSubState(subState);
      if (props.selectedNodeId) props.onSelectNodeSubState?.(props.selectedNodeId, subState);
    }), [props, runWithDirtyGuard, vp]),
    handleGuardedModeSelect: useCallback((mode: "interact" | "click-dom" | "drag-box") => runWithDirtyGuard(() => vp.setPointerMode(mode)), [runWithDirtyGuard, vp]),
    handleConfirmSavePatch: useCallback(() => flushPending(true), [flushPending]),
    handleConfirmDiscardChanges: useCallback(() => flushPending(false), [flushPending]),
    handleCancelDirtyGuard: useCallback(() => { vp.setShowDirtyGuardPrompt(false); setPendingAction(null); }, [vp]),
  };
}

function useWalkthroughController(
  nodes: CompanionCanvasTabProps["nodes"],
  onSelectNode: CompanionCanvasTabProps["onSelectNode"],
  setIsPresenting: React.Dispatch<React.SetStateAction<boolean>>
) {
  const [isActive, setIsActive] = useState(false);
  const [autoPlay, setAutoPlay] = useState(true);
  const [viewMode, setViewModeState] = useState<"dual" | "full">("dual");
  const [stepIndex, setStepIndex] = useState(0);
  const total = nodes?.length ?? 0;

  const jumpTo = useCallback((nextIdx: number) => {
    if (!nodes || nodes.length === 0) return;
    const bounded = ((nextIdx % nodes.length) + nodes.length) % nodes.length;
    setStepIndex(bounded);
    const target = nodes[bounded];
    if (target && onSelectNode) onSelectNode(target.id);
  }, [nodes, onSelectNode]);

  useEffect(() => {
    if (!isActive || !autoPlay || total === 0) return;
    const timer = setInterval(() => {
      setStepIndex((prev) => {
        const next = (prev + 1) % total;
        const target = nodes?.[next];
        if (target && onSelectNode) onSelectNode(target.id);
        return next;
      });
    }, WALKTHROUGH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [autoPlay, isActive, nodes, onSelectNode, total]);

  useEffect(() => {
    if (!isActive) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        setIsActive(false);
        setIsPresenting(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isActive, setIsPresenting]);

  const toggleWalkthrough = useCallback(() => {
    setIsActive((prev) => {
      const next = !prev;
      if (next) {
        setAutoPlay(true);
        setViewModeState("dual");
        setIsPresenting(false);
        jumpTo(0);
      } else {
        setIsPresenting(false);
      }
      return next;
    });
  }, [jumpTo, setIsPresenting]);

  const setViewMode = useCallback((mode: "dual" | "full") => {
    setViewModeState(mode);
    setIsPresenting(mode === "full");
  }, [setIsPresenting]);

  const exitWalkthrough = useCallback(() => {
    setIsActive(false);
    setIsPresenting(false);
  }, [setIsPresenting]);

  const { locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const currentNode = total > 0 ? nodes?.[stepIndex % total] : undefined;
  const stepLabel = total > 0
    ? `${currentNode?.screenKey || currentNode?.id || "SCR"} (${formatNumber((stepIndex % total) + 1)}/${formatNumber(total)})`
    : undefined;

  return {
    isActive, autoPlay, viewMode, stepLabel,
    toggleAutoPlay: () => setAutoPlay((v) => !v),
    prevStep: () => { setAutoPlay(false); jumpTo(stepIndex - 1); },
    nextStep: () => { setAutoPlay(false); jumpTo(stepIndex + 1); },
    toggleWalkthrough, setViewMode, exitWalkthrough,
  };
}

const CompanionOverlaysHost: React.FC<{
  props: CompanionCanvasTabProps;
  effectiveInspectorFormat: CanvasTabItem["format"];
  snapshotCtrl: ReturnType<typeof useSnapshotWorkspaceController>;
  proposalCtrl: ReturnType<typeof useNodeProposalController>;
  nodeLabelsById: Record<string, string>;
  vp: ReturnType<typeof useDualPaneViewportState>;
  isLaunchpadOpen: boolean;
  setIsLaunchpadOpen: (v: boolean) => void;
  isSampleDataOpen: boolean;
  setIsSampleDataOpen: (v: boolean) => void;
  isHistoricalReadOnly: boolean;
}> = (p) => (
  <>
    <CanvasSnapshotDrawerAndModalHost projectId={p.props.projectId} artifactFormat={p.effectiveInspectorFormat} controller={p.snapshotCtrl} />
    <CanvasProposalDrawerHost
      controller={p.proposalCtrl}
      nodeLabelsById={p.nodeLabelsById}
      isReadOnly={p.isHistoricalReadOnly}
      onOpenInlineEditor={guardReadOnlyAction(p.isHistoricalReadOnly, () => p.vp.setIsInlineCodeOpen(true))}
    />
    <ProjectLaunchpadModal
      isOpen={p.isLaunchpadOpen}
      onClose={() => p.setIsLaunchpadOpen(false)}
      onLaunchPath={guardReadOnlyAction(p.isHistoricalReadOnly, (pathType, data) => { void submitLaunchpadProject(pathType, data, p.props.projectId); })}
    />
    <SampleDataStudioModal
      isOpen={p.isSampleDataOpen}
      onClose={() => p.setIsSampleDataOpen(false)}
      onApplyFixture={guardReadOnlyAction(p.isHistoricalReadOnly, (preset: SampleDataPreset, json: string) => { void applyStudioSampleFixture(p.props.projectId, p.props.selectedNodeId, preset, json); })}
    />
  </>
);

function useCompanionWorkspaceShell(props: CompanionCanvasTabProps) {
  const splitContainerRef = useRef<HTMLDivElement>(null);
  const nav = useTabNavigationState(props.activeTabId, props.onTabChange);
  const vp = useDualPaneViewportState(nav.activeTab);
  useInternalSplitResizer(vp.isDraggingInternalSplit, vp.setIsDraggingInternalSplit, splitContainerRef, vp.setLeftPanePercent);
  const [isLaunchpadOpen, setIsLaunchpadOpen] = useState(false);
  const [isSampleDataOpen, setIsSampleDataOpen] = useState(false);
  const [historicalPreviewTurn, setHistoricalPreviewTurn] = useState<ChatArtifactSnapshotResponse | null>(null);
  const isHistoricalReadOnly = isHistoricalPreviewReadOnly(historicalPreviewTurn?.id);
  const isHtmlDesignTab = nav.activeTab.format === "dag" || nav.activeTab.format === "slide-cms";
  const selectedNodeObj = props.nodes?.find((n) => n.id === props.selectedNodeId);
  const nodeLabelsById = Object.fromEntries((props.nodes ?? []).map((n) => [n.id, n.screenKey || n.id]));
  const internalProposalCtrl = useNodeProposalController({
    projectId: props.projectId, selectedNodeId: props.selectedNodeId, selectedNodeKey: selectedNodeObj?.screenKey || null,
    isModalOpen: isLaunchpadOpen || isSampleDataOpen || vp.showDirtyGuardPrompt, isInlineCodeOpen: vp.isInlineCodeOpen, disabled: Boolean(props.proposalController),
  });
  const proposalCtrl = props.proposalController ?? internalProposalCtrl;
  const snapshotCtrl = useSnapshotWorkspaceController(props.projectId, setHistoricalPreviewTurn);
  const effectiveInspectorFormat = isHtmlDesignTab && vp.structureLens === "slide-cms" ? "slide-cms" : nav.activeTab.format;
  const walkthrough = useWalkthroughController(props.nodes, props.onSelectNode, vp.setIsPresenting);
  useEffect(() => {
    const onOpenLaunchpad = () => setIsLaunchpadOpen(true);
    window.addEventListener("cowok:open-launchpad", onOpenLaunchpad);
    return () => window.removeEventListener("cowok:open-launchpad", onOpenLaunchpad);
  }, []);
  const guards = useCanvasGuardedActions(vp, nav, props);
  const stageMutators = resolveHistoricalReadOnlyStageBindings(props, isHistoricalReadOnly);
  return {
    splitContainerRef, nav, vp, isLaunchpadOpen, setIsLaunchpadOpen, isSampleDataOpen, setIsSampleDataOpen,
    walkthrough, historicalPreviewTurn, isHistoricalReadOnly,
    isHtmlDesignTab, nodeLabelsById, proposalCtrl, snapshotCtrl, effectiveInspectorFormat, guards, stageMutators,
  };
}

export function CompanionCanvasTab(props: CompanionCanvasTabProps) {
  const s = useCompanionWorkspaceShell(props);
  const versionMenuNode = (
    <VersionPublishMenuButton snapshotsCount={s.snapshotCtrl.snapshots.length} onOpenHistory={() => s.snapshotCtrl.setIsDrawerOpen((p) => !p)} onOpenExportSection={(sec) => { void s.snapshotCtrl.exp.handleOpenExport(sec); }} />
  );
  const isWalkthroughFullscreen = s.walkthrough.isActive;
  const containerStyle: React.CSSProperties = isWalkthroughFullscreen
    ? {
        position: "fixed",
        inset: 0,
        width: "100vw",
        height: "100vh",
        zIndex: 45,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        backgroundColor: "var(--bg-canvas)",
      }
    : {
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        backgroundColor: "var(--bg-canvas)",
        position: "relative",
      };
  return (
    <div
      className={`companion-canvas-tab-container${isWalkthroughFullscreen ? " walkthrough-fullscreen-active" : ""}`}
      data-walkthrough-fullscreen={isWalkthroughFullscreen ? "true" : "false"}
      style={containerStyle}
    >
      <CompanionLockBanners ambiguityScore={props.ambiguityScore} conflictState={s.stageMutators.conflictState} />
      <CompanionDirtyGuardModal isOpen={s.vp.showDirtyGuardPrompt} onCancel={s.guards.handleCancelDirtyGuard} onDiscard={s.guards.handleConfirmDiscardChanges} onSave={s.guards.handleConfirmSavePatch} />
      {!isWalkthroughFullscreen && (
        <CompanionTabBar tabs={s.nav.tabs} activeTabId={s.nav.activeTab.id} isNewTabMenuOpen={s.nav.isNewTabMenuOpen} onToggleNewTabMenu={() => s.nav.setIsNewTabMenuOpen((prev) => !prev)} onTabSelect={s.guards.handleGuardedTabSelect} onCloseTab={s.nav.handleClose} onCreateTab={s.nav.handleCreateTab} />
      )}
      <HistoricalPreviewBanner activePreviewTurn={s.snapshotCtrl.turn.activePreviewTurn} onExit={s.snapshotCtrl.turn.handlePreviewTurn} />
      {s.isHtmlDesignTab ? (
        <ReadOnlyCanvasContext.Provider value={s.isHistoricalReadOnly}>
          <DualPaneHtmlStage
            {...props} {...s.stageMutators} activeTab={s.nav.activeTab} vp={s.vp} splitContainerRef={s.splitContainerRef}
            isLaunchpadOpen={s.isLaunchpadOpen} onOpenLaunchpad={guardReadOnlyAction(s.isHistoricalReadOnly, () => s.setIsLaunchpadOpen(true))} onCloseLaunchpad={() => s.setIsLaunchpadOpen(false)}
            isSampleDataOpen={s.isSampleDataOpen} onOpenSampleData={guardReadOnlyAction(s.isHistoricalReadOnly, () => s.setIsSampleDataOpen(true))} onCloseSampleData={() => s.setIsSampleDataOpen(false)}
            isWalkthroughActive={s.walkthrough.isActive} onToggleWalkthrough={s.walkthrough.toggleWalkthrough}
            walkthroughViewMode={s.walkthrough.viewMode} onSetWalkthroughViewMode={s.walkthrough.setViewMode}
            walkthroughStepLabel={s.walkthrough.stepLabel} walkthroughAutoPlay={s.walkthrough.autoPlay}
            onToggleWalkthroughAutoPlay={s.walkthrough.toggleAutoPlay} onWalkthroughPrev={s.walkthrough.prevStep}
            onWalkthroughNext={s.walkthrough.nextStep} onExitWalkthrough={s.walkthrough.exitWalkthrough}
            snapshotsCount={s.snapshotCtrl.snapshots.length}
            onOpenVersionHistory={() => s.snapshotCtrl.setIsDrawerOpen((p) => !p)}
            onOpenExportSection={(sec) => { void s.snapshotCtrl.exp.handleOpenExport(sec); }}
            isDesignerTalkOpen={s.proposalCtrl.isDesignerTalkOpen} onCloseDesignerTalk={() => s.proposalCtrl.setIsDesignerTalkOpen(false)} isProposalDrawerOpen={s.proposalCtrl.isDrawerOpen}
            isModalOpen={s.isLaunchpadOpen || s.isSampleDataOpen || s.vp.showDirtyGuardPrompt} activeProposal={s.proposalCtrl.activeProposal}
            onDecideTradeoff={(tid, input) => (s.isHistoricalReadOnly ? Promise.resolve() : s.proposalCtrl.decideTradeoff(tid, input))}
            onGuardedSubStateSelect={s.guards.handleGuardedSubStateSelect} onGuardedModeSelect={s.guards.handleGuardedModeSelect}
            historicalPreviewTurn={s.historicalPreviewTurn} versionMenuNode={versionMenuNode}
          />
        </ReadOnlyCanvasContext.Provider>
      ) : (
        <div style={{ position: "relative", flex: 1, display: "flex", overflow: "hidden" }}>
          <NonHtmlTabView activeTab={s.nav.activeTab} inlineCodeText={s.vp.inlineCodeText} />
          <div style={{ position: "absolute", top: "10px", right: "14px", zIndex: 10 }}>{versionMenuNode}</div>
        </div>
      )}
      <CompanionOverlaysHost
        props={props} effectiveInspectorFormat={s.effectiveInspectorFormat} snapshotCtrl={s.snapshotCtrl} proposalCtrl={s.proposalCtrl}
        nodeLabelsById={s.nodeLabelsById} vp={s.vp} isLaunchpadOpen={s.isLaunchpadOpen} setIsLaunchpadOpen={s.setIsLaunchpadOpen}
        isSampleDataOpen={s.isSampleDataOpen} setIsSampleDataOpen={s.setIsSampleDataOpen} isHistoricalReadOnly={s.isHistoricalReadOnly}
      />
    </div>
  );
}
