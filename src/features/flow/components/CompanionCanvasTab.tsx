// @story 04.1, 05.1
// @implements AC-03, AC-04, AC-06, AC-09, [EC-STORYBOARD-001] Dirty Inline Editor Guard, 4-Path Launchpad & Sample Data Studio
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import { CanvasProposalDrawerHost } from "../../proposals/components/CanvasProposalControls";
import { useNodeProposalController } from "../../proposals/hooks/useNodeProposalController";
import { applyLaunchpadMaterialToProject, applyStudioSampleFixture, submitLaunchpadProject } from "../utils/companion-canvas-actions";
import { useWalkthroughController } from "../hooks/useWalkthroughController";
import { useDeckPresentationController } from "../hooks/useDeckPresentationController";
import { CompanionTabBar } from "./CompanionTabBar";
import { NonHtmlTabView } from "./CompanionNonHtmlTabView";
import { DualPaneHtmlStage } from "./CompanionDualPaneStage";
import { ProjectLaunchpadModal, type LaunchpadMaterialContext } from "./ProjectLaunchpadModal";
import { SampleDataStudioModal, type SampleDataPreset } from "./SampleDataStudioModal";
import {
  ReadOnlyCanvasContext,
  useInternalSplitResizer,
  useTabNavigationState,
  useDualPaneViewportState,
} from "../hooks/useCompanionCanvasState";

const AMBIGUITY_LOCK_THRESHOLD = 0.45;
const LOCK_BANNER_ICON_SIZE = 13;
const LOCK_BANNER_Z_INDEX = 20;
const MODAL_ICON_SIZE = 18;
const DIRTY_MODAL_Z_INDEX = 60;
const WALKTHROUGH_FULLSCREEN_Z_INDEX = 45;
const NON_HTML_MENU_Z_INDEX = 10;
const ICON_STROKE_WEIGHT = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;

export interface CanvasTabItem {
  id: string;
  title: string;
  iconName: string;
  format: "dag" | "slide-cms" | "md" | "assets" | "code";
  defaultLens?: "userflow" | "slide-cms";
  filePath?: string;
  isClosable?: boolean;
}

import type { StageWorkstreamMode } from "../utils/stage-capability";

export interface CompanionCanvasTabProps {
  projectId?: string;
  sessionId?: string;
  sessionEntryKind?: StageWorkstreamMode;
  activeTabId?: string;
  onTabChange?: (tabId: string, format: CanvasTabItem["format"]) => void;
  selectedNodeId?: string | null;
  onSelectNode?: (nodeId: string) => void;
  nodes?: WorkbenchNode[];
  isDraftDirty?: boolean;
  orphanCount?: number;
  conflictState?: ConflictState | null;
  ambiguityScore?: number;
  canUndo?: boolean;
  canRedo?: boolean;
  onUndo?: () => void;
  onRedo?: () => void;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
  onAttachRegionToChat?: (chipLabel: string) => void;
  onQuoteText?: (text: string) => void;
  onSelectNodeSubState?: (nodeId: string, subState: FixtureVariant) => void;
  onUpdateNodeViewport?: (nodeId: string, vp: ViewportPreset) => void;
  proposalController?: ReturnType<typeof useNodeProposalController>;
  stageSkin?: "sketch-paper" | "brand-color";
  sketchPolishBarNode?: React.ReactNode;
  onRegisterStageSkinSwap?: (fn: ((skin: "sketch-paper" | "brand-color", cssVars?: Record<string, string>) => void) | null) => void;
  onRegisterStageApplyHtml?: (fn: ((html: string) => void) | null) => void;
  onLockedRuleReceipt?: (notice: import("../../../lib/sketch-polish-occ-store").LockedRuleReceiptNotice) => void;
  children: React.ReactNode;
}

const CompanionLockBanners: React.FC<{ ambiguityScore?: number; conflictState?: ConflictState | null }> = ({ ambiguityScore, conflictState }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isAmbiguityLocked = ambiguityScore !== undefined && ambiguityScore >= AMBIGUITY_LOCK_THRESHOLD;
  if (!isAmbiguityLocked && !conflictState) return null;

  return (
    <>
      {isAmbiguityLocked && (
        <div data-testid="ambiguity-execution-lock" style={{ padding: "5px 14px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", borderBottom: "1px solid var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, display: "flex", alignItems: "center", justifyContent: "space-between", zIndex: LOCK_BANNER_Z_INDEX }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="lock" size={LOCK_BANNER_ICON_SIZE} strokeWidth={ICON_STROKE_WEIGHT} color="currentColor" />
            <span>{t("sdi.ambiguityLockFull", "Khóa Thực Thi: Điểm mơ hồ {{score}} ≥ 0.45. Cần làm rõ ý định trước khi sinh mã.").replace("{{score}}", formatNumber(ambiguityScore, 2))}</span>
          </div>
        </div>
      )}
      {Boolean(conflictState) && (
        <div data-testid="dag-conflict-lock" style={{ padding: "5px 14px", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", borderBottom: "1px solid var(--state-danger)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, display: "flex", alignItems: "center", gap: "6px", zIndex: LOCK_BANNER_Z_INDEX }}>
          <BrandIcon name="warning" size={LOCK_BANNER_ICON_SIZE} strokeWidth={ICON_STROKE_WEIGHT} color="currentColor" />
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
    <div role="dialog" aria-modal="true" data-testid="dirty-buffer-guard-modal" style={{ position: "absolute", inset: 0, zIndex: DIRTY_MODAL_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px" }}>
      <div style={{ width: "100%", maxWidth: "420px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", padding: "18px 20px", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", gap: "12px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <BrandIcon name="warning" size={MODAL_ICON_SIZE} strokeWidth={ICON_STROKE_WEIGHT} color="var(--state-warning)" />
          <h3 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{t("canvas.dirtyGuardTitle", "Thay Đổi Chưa Lưu Trong Trình Sửa Mã")}</h3>
        </div>
        <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)", lineHeight: "1.5" }}>{t("canvas.dirtyGuardBody", "Bạn có thay đổi chưa lưu trong Inline </> Editor. Bạn muốn Lưu bản vá (Save Patch) hay Hủy bỏ thay đổi (Discard Changes) trước khi chuyển đổi?")}</p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
          <button type="button" data-testid="dirty-guard-cancel-btn" onClick={onCancel} style={{ padding: "6px 12px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>{t("common.cancel", "Hủy Bỏ")}</button>
          <button type="button" data-testid="dirty-guard-discard-btn" onClick={onDiscard} style={{ padding: "6px 12px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)", color: "var(--state-danger)", cursor: "pointer" }}>{t("canvas.discardChangesBtn", "Hủy Thay Đổi (Discard)")}</button>
          <button type="button" data-testid="dirty-guard-save-btn" onClick={onSave} style={{ padding: "6px 14px", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>{t("canvas.savePatchBtn", "Lưu Bản Vá (Save Patch)")}</button>
        </div>
      </div>
    </div>
  );
};

interface CmsGuardActionsRef {
  flushAll: () => Promise<boolean>;
  discardAll: () => void;
  applyCustomHtml: (rawHtml: string) => Promise<{ ok: boolean; appliedHtml?: string }>;
}

async function commitGuardedSave(opts: {
  vp: ReturnType<typeof useDualPaneViewportState>;
  tabId: string;
  selectedNodeId: string | null | undefined;
  isCmsDirtyRef: React.MutableRefObject<boolean>;
  cmsGuardRef: React.MutableRefObject<CmsGuardActionsRef | null>;
}): Promise<boolean> {
  const { vp, tabId, selectedNodeId, isCmsDirtyRef, cmsGuardRef } = opts;
  if (isCmsDirtyRef.current && cmsGuardRef.current) {
    const flushed = await cmsGuardRef.current.flushAll();
    if (!flushed) return false;
  }
  if (!vp.dirtyBuffer) return true;
  if (vp.structureLens === "slide-cms") {
    const slideId = vp.slides[vp.activeSlideIndex]?.id || vp.slides[0]?.id || "slide-0";
    let committed = vp.htmlCodeText;
    if (cmsGuardRef.current) {
      const outcome = await cmsGuardRef.current.applyCustomHtml(vp.htmlCodeText);
      if (!outcome.ok) return false;
      committed = outcome.appliedHtml ?? vp.htmlCodeText;
    }
    vp.setBaselineHtmlText(committed);
    vp.setHtmlCodeText(committed);
    vp.setCustomHtmlForTarget(`${tabId}::slide-cms::${slideId}`, committed);
    return true;
  }
  vp.setBaselineHtmlText(vp.htmlCodeText);
  vp.setCustomHtmlForTarget(`${tabId}::userflow::${selectedNodeId || "default"}::screen`, vp.htmlCodeText);
  return true;
}

function useCanvasGuardedActions(opts: {
  vp: ReturnType<typeof useDualPaneViewportState>;
  nav: ReturnType<typeof useTabNavigationState>;
  props: Pick<CompanionCanvasTabProps, "selectedNodeId" | "onSelectNode" | "onSelectNodeSubState">;
  isCmsDirtyRef: React.MutableRefObject<boolean>;
  cmsGuardRef: React.MutableRefObject<CmsGuardActionsRef | null>;
}) {
  const { vp, nav, props, isCmsDirtyRef, cmsGuardRef } = opts;
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);
  const runWithDirtyGuard = useCallback((action: () => void) => {
    if (vp.dirtyBuffer || isCmsDirtyRef.current) { setPendingAction(() => action); vp.setShowDirtyGuardPrompt(true); }
    else action();
  }, [isCmsDirtyRef, vp]);
  const flushPending = useCallback(async (savePatch: boolean) => {
    if (savePatch) {
      const ok = await commitGuardedSave({ vp, tabId: nav.activeTab.id, selectedNodeId: props.selectedNodeId, isCmsDirtyRef, cmsGuardRef });
      if (!ok) { vp.setShowDirtyGuardPrompt(false); setPendingAction(null); return; }
    } else {
      vp.setHtmlCodeText(vp.baselineHtmlText);
      if (isCmsDirtyRef.current) cmsGuardRef.current?.discardAll();
    }
    vp.setShowDirtyGuardPrompt(false);
    vp.setIsInlineCodeOpen(false);
    if (pendingAction) { pendingAction(); setPendingAction(null); }
  }, [cmsGuardRef, isCmsDirtyRef, nav.activeTab.id, pendingAction, props.selectedNodeId, vp]);
  return {
    handleGuardedTabSelect: useCallback((tabId: string, format: CanvasTabItem["format"]) => runWithDirtyGuard(() => nav.handleTabSelect(tabId, format)), [nav, runWithDirtyGuard]),
    handleGuardedNodeSelect: useCallback((nodeId: string) => runWithDirtyGuard(() => props.onSelectNode?.(nodeId)), [props, runWithDirtyGuard]),
    handleGuardedSubStateSelect: useCallback((subState: FixtureVariant) => runWithDirtyGuard(() => { vp.setActiveSubState(subState); if (props.selectedNodeId) props.onSelectNodeSubState?.(props.selectedNodeId, subState); }), [props, runWithDirtyGuard, vp]),
    handleGuardedModeSelect: useCallback((mode: "interact" | "click-dom" | "drag-box") => runWithDirtyGuard(() => vp.setPointerMode(mode)), [runWithDirtyGuard, vp]),
    handleConfirmSavePatch: useCallback(() => { void flushPending(true); }, [flushPending]),
    handleConfirmDiscardChanges: useCallback(() => { void flushPending(false); }, [flushPending]),
    handleCancelDirtyGuard: useCallback(() => { vp.setShowDirtyGuardPrompt(false); setPendingAction(null); }, [vp]),
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
  launchpadMaterialContext: LaunchpadMaterialContext | null;
  setLaunchpadMaterialContext: (v: LaunchpadMaterialContext | null) => void;
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
      materialContext={p.launchpadMaterialContext}
      onClose={() => {
        p.setIsLaunchpadOpen(false);
        p.setLaunchpadMaterialContext(null);
      }}
      onLaunchPath={guardReadOnlyAction(p.isHistoricalReadOnly, (pathType, data) => {
        if (p.launchpadMaterialContext) {
          if (pathType === "templates" && data.templateId === "tpl-slide-deck") p.vp.setStructureLens("slide-cms");
          void applyLaunchpadMaterialToProject(p.launchpadMaterialContext.projectId, p.launchpadMaterialContext.entryKind, pathType, data);
        } else {
          void submitLaunchpadProject(pathType, data);
        }
      })}
    />
    <SampleDataStudioModal
      isOpen={p.isSampleDataOpen}
      onClose={() => p.setIsSampleDataOpen(false)}
      onApplyFixture={guardReadOnlyAction(p.isHistoricalReadOnly, (preset: SampleDataPreset, json: string) => { void applyStudioSampleFixture(p.props.projectId, p.props.selectedNodeId, preset, json); })}
    />
  </>
);

function useLaunchpadOpenListener() {
  const [isLaunchpadOpen, setIsLaunchpadOpen] = useState(false);
  const [launchpadMaterialContext, setLaunchpadMaterialContext] = useState<LaunchpadMaterialContext | null>(null);
  useEffect(() => {
    const onOpenLaunchpad = (ev: Event) => {
      const detail = (ev as CustomEvent<{ mode?: string; projectId?: string; projectName?: string; entryKind?: "fe-dag" | "presentation" }>).detail;
      const hasContext = detail?.mode === "post-create-material" && Boolean(detail.projectId && detail.projectName);
      setLaunchpadMaterialContext(
        hasContext ? { projectId: detail!.projectId!, projectName: detail!.projectName!, entryKind: detail?.entryKind ?? "fe-dag" } : null
      );
      setIsLaunchpadOpen(true);
    };
    window.addEventListener("cowok:open-launchpad", onOpenLaunchpad);
    return () => window.removeEventListener("cowok:open-launchpad", onOpenLaunchpad);
  }, []);
  return { isLaunchpadOpen, setIsLaunchpadOpen, launchpadMaterialContext, setLaunchpadMaterialContext };
}

function useCompanionWorkspaceShell(props: CompanionCanvasTabProps) {
  const splitContainerRef = useRef<HTMLDivElement>(null);
  const presentationHostRef = useRef<HTMLDivElement>(null);
  const deckKeyNavRef = useRef<((dir: "prev" | "next" | "goto", targetIndex?: number) => void) | null>(null);
  const presenterBindingsRef = useRef<{ isPresenterActive: boolean; toggleBlackoutKey: (mode: "black" | "white") => void } | null>(null);
  const [isPresenterActiveState, setIsPresenterActiveState] = useState(false);
  const nav = useTabNavigationState(props.activeTabId, props.onTabChange);
  const vp = useDualPaneViewportState(nav.activeTab, props.sessionId, props.sessionEntryKind);
  useInternalSplitResizer(vp.isDraggingInternalSplit, vp.setIsDraggingInternalSplit, splitContainerRef, vp.setLeftPanePercent);
  const { isLaunchpadOpen, setIsLaunchpadOpen, launchpadMaterialContext, setLaunchpadMaterialContext } = useLaunchpadOpenListener();
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
  const deckPresent = useDeckPresentationController({
    slidesCount: vp.slides.length,
    activeSlideIndex: vp.activeSlideIndex,
    onChangeSlide: vp.setActiveSlideIndex,
    onPostDeckKeyNav: (dir, targetIdx) => deckKeyNavRef.current?.(dir, targetIdx),
    onExitWalkthrough: () => {
      if (walkthrough.isActive) walkthrough.exitWalkthrough();
    },
    isPresenting: vp.isPresenting && !walkthrough.isActive,
    setIsPresenting: vp.setIsPresenting,
    containerRef: presentationHostRef,
    isPresenterActive: isPresenterActiveState,
    onToggleBlackoutKey: (mode) => presenterBindingsRef.current?.toggleBlackoutKey(mode),
  });
  const isCmsDirtyRef = useRef<boolean>(false);
  const cmsGuardRef = useRef<CmsGuardActionsRef | null>(null);
  const guards = useCanvasGuardedActions({ vp, nav, props, isCmsDirtyRef, cmsGuardRef });
  const stageMutators = resolveHistoricalReadOnlyStageBindings(props, isHistoricalReadOnly);
  return {
    splitContainerRef, presentationHostRef, deckKeyNavRef, presenterBindingsRef, setIsPresenterActiveState, isCmsDirtyRef, cmsGuardRef, nav, vp, isLaunchpadOpen, setIsLaunchpadOpen,
    launchpadMaterialContext, setLaunchpadMaterialContext, isSampleDataOpen, setIsSampleDataOpen,
    walkthrough, deckPresent, historicalPreviewTurn, isHistoricalReadOnly,
    isHtmlDesignTab, nodeLabelsById, proposalCtrl, snapshotCtrl, effectiveInspectorFormat, guards, stageMutators,
  };
}

function buildCanvasContainerStyle(isWalkthroughFullscreen: boolean): React.CSSProperties {
  const base: React.CSSProperties = {
    display: "flex", flexDirection: "column", overflow: "hidden", backgroundColor: "var(--bg-canvas)",
  };
  if (isWalkthroughFullscreen) {
    return { ...base, position: "fixed", inset: 0, width: "100%", height: "100%", zIndex: WALKTHROUGH_FULLSCREEN_Z_INDEX };
  }
  return { ...base, width: "100%", height: "100%", position: "relative" };
}

function useStableStagePresentationBindings(s: ReturnType<typeof useCompanionWorkspaceShell>) {
  const onPostDeckKeyNav = useCallback(
    (dir: "prev" | "next" | "goto", targetIdx?: number) => s.deckKeyNavRef.current?.(dir, targetIdx),
    [s.deckKeyNavRef]
  );
  const onRegisterDeckKeyNav = useCallback(
    (fn: ((dir: "prev" | "next" | "goto", targetIdx?: number) => void) | null) => {
      s.deckKeyNavRef.current = fn;
    },
    [s.deckKeyNavRef]
  );
  const onRegisterPresenterBindings = useCallback(
    (bindings: Parameters<NonNullable<Parameters<typeof DualPaneHtmlStage>[0]["onRegisterPresenterBindings"]>>[0]) => {
      s.presenterBindingsRef.current = bindings;
      s.setIsPresenterActiveState(Boolean(bindings?.isPresenterActive));
    },
    [s.presenterBindingsRef, s.setIsPresenterActiveState]
  );
  return useMemo(
    () => ({
      onTogglePresentDeck: s.deckPresent.togglePresent, deckAutoPlay: s.deckPresent.autoPlay,
      onToggleDeckAutoPlay: s.deckPresent.toggleAutoPlay, onDeckPrev: s.deckPresent.prevSlide,
      onDeckNext: s.deckPresent.nextSlide, onExitPresent: s.deckPresent.exitPresent,
      onEnterPresentDeck: s.deckPresent.enterPresent, onPostDeckKeyNav, onRegisterDeckKeyNav, onRegisterPresenterBindings,
    }),
    [onPostDeckKeyNav, onRegisterDeckKeyNav, onRegisterPresenterBindings, s.deckPresent]
  );
}

function CompanionCanvasStageBody(p: {
  props: CompanionCanvasTabProps;
  s: ReturnType<typeof useCompanionWorkspaceShell>;
  versionMenuNode: React.ReactNode;
}) {
  const { props, s, versionMenuNode } = p;
  const stageBindings = useStableStagePresentationBindings(s);
  if (!s.isHtmlDesignTab) {
    return (
      <div style={{ position: "relative", flex: 1, display: "flex", overflow: "hidden" }}>
        <NonHtmlTabView activeTab={s.nav.activeTab} inlineCodeText={s.vp.inlineCodeText} />
        <div style={{ position: "absolute", top: "10px", right: "14px", zIndex: NON_HTML_MENU_Z_INDEX }}>{versionMenuNode}</div>
      </div>
    );
  }
  return (
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
        onOpenVersionHistory={() => s.snapshotCtrl.setIsDrawerOpen((prev) => !prev)}
        onOpenExportSection={(sec) => { void s.snapshotCtrl.exp.handleOpenExport(sec); }}
        isDesignerTalkOpen={s.proposalCtrl.isDesignerTalkOpen} onCloseDesignerTalk={() => s.proposalCtrl.setIsDesignerTalkOpen(false)} isProposalDrawerOpen={s.proposalCtrl.isDrawerOpen}
        isModalOpen={s.isLaunchpadOpen || s.isSampleDataOpen || s.vp.showDirtyGuardPrompt} activeProposal={s.proposalCtrl.activeProposal}
        onDecideTradeoff={(tid, input) => (s.isHistoricalReadOnly ? Promise.resolve() : s.proposalCtrl.decideTradeoff(tid, input))}
        onGuardedSubStateSelect={s.guards.handleGuardedSubStateSelect} onGuardedModeSelect={s.guards.handleGuardedModeSelect}
        historicalPreviewTurn={s.historicalPreviewTurn} versionMenuNode={versionMenuNode}
        {...stageBindings}
        onCmsDirtyChange={(dirty) => { s.isCmsDirtyRef.current = dirty; }}
        onRegisterCmsGuardActions={(actions) => { s.cmsGuardRef.current = actions; }}
      />
    </ReadOnlyCanvasContext.Provider>
  );
}

export function CompanionCanvasTab(props: CompanionCanvasTabProps) {
  const s = useCompanionWorkspaceShell(props);
  const versionMenuNode = (
    <VersionPublishMenuButton snapshotsCount={s.snapshotCtrl.snapshots.length} onOpenHistory={() => s.snapshotCtrl.setIsDrawerOpen((p) => !p)} onOpenExportSection={(sec) => { void s.snapshotCtrl.exp.handleOpenExport(sec); }} />
  );
  const isWalkthroughFullscreen = s.walkthrough.isActive;
  const isDeckPresenting = s.deckPresent.isDeckPresenting;
  const isStageFullscreen = isWalkthroughFullscreen || isDeckPresenting;
  return (
    <div
      ref={s.presentationHostRef}
      className={`companion-canvas-tab-container${isWalkthroughFullscreen ? " walkthrough-fullscreen-active" : ""}${isDeckPresenting ? " deck-presentation-fullscreen-active" : ""}`}
      data-walkthrough-fullscreen={isWalkthroughFullscreen ? "true" : "false"}
      data-deck-presenting={isDeckPresenting ? "true" : "false"}
      style={buildCanvasContainerStyle(isStageFullscreen)}
    >
      <CompanionLockBanners ambiguityScore={props.ambiguityScore} conflictState={s.stageMutators.conflictState} />
      <CompanionDirtyGuardModal isOpen={s.vp.showDirtyGuardPrompt} onCancel={s.guards.handleCancelDirtyGuard} onDiscard={s.guards.handleConfirmDiscardChanges} onSave={s.guards.handleConfirmSavePatch} />
      {!isStageFullscreen && (
        <CompanionTabBar tabs={s.nav.tabs} activeTabId={s.nav.activeTab.id} isNewTabMenuOpen={s.nav.isNewTabMenuOpen} onToggleNewTabMenu={() => s.nav.setIsNewTabMenuOpen((prev) => !prev)} onTabSelect={s.guards.handleGuardedTabSelect} onCloseTab={s.nav.handleClose} onCreateTab={s.nav.handleCreateTab} />
      )}
      <HistoricalPreviewBanner activePreviewTurn={s.snapshotCtrl.turn.activePreviewTurn} onExit={s.snapshotCtrl.turn.handlePreviewTurn} />
      <CompanionCanvasStageBody props={props} s={s} versionMenuNode={versionMenuNode} />
      <CompanionOverlaysHost
        props={props} effectiveInspectorFormat={s.effectiveInspectorFormat} snapshotCtrl={s.snapshotCtrl} proposalCtrl={s.proposalCtrl}
        nodeLabelsById={s.nodeLabelsById} vp={s.vp} isLaunchpadOpen={s.isLaunchpadOpen} setIsLaunchpadOpen={s.setIsLaunchpadOpen}
        launchpadMaterialContext={s.launchpadMaterialContext} setLaunchpadMaterialContext={s.setLaunchpadMaterialContext}
        isSampleDataOpen={s.isSampleDataOpen} setIsSampleDataOpen={s.setIsSampleDataOpen} isHistoricalReadOnly={s.isHistoricalReadOnly}
      />
    </div>
  );
}
