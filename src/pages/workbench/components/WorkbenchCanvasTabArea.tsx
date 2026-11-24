// @story 04.1, 04.4, 04.5
// @implements AC-01, AC-02, AC-06, AC-07, AC-08
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { FlowWorkbenchState, createNewFlowWorkbenchStore } from "../../../features/flow/store/flow-workbench-store";
import { CompanionCanvasTab, CanvasTabItem } from "../../../features/flow/components/CompanionCanvasTab";
import { DagCanvas2D, mapSynthesizedPlanToWorkbenchGraph } from "../../../features/flow/components/DagCanvas2D";
import type { SynthesizedFlowPlan } from "../../../lib/validations/flow-synthesize";
import { ContextualNodeFloatingChat } from "../../../features/flow/components/ContextualNodeFloatingChat";
import type { useNodeProposalController } from "../../../features/proposals/hooks/useNodeProposalController";
import { ErrorBoundary } from "../../../components/ui/ErrorBoundary";
import { findOrphanNodes } from "../../../features/flow/utils/graph-analysis";
import { VisualClarificationModal } from "../../../features/sdi/components/VisualClarificationModal";
import {
  LockedRuleChangeReceiptCard,
  StageSkinToggleBar,
  SteeringChipsBar,
  useSketchPolishController,
} from "../../../features/sketch-polish";
import type { LiveDirectionOption } from "../../../lib/validations/sketch-polish";
import { useWorkbenchGraphHandlers } from "../hooks/useWorkbenchGraphHandlers";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";

const DEFAULT_LIVE_DIRECTIONS: readonly LiveDirectionOption[] = [
  {
    id: "dir-editorial-sketch",
    label: "Direction 1 · Monochrome Sketch",
    summary: "Warm sketch paper hierarchy first before locking brand colors.",
    accentHex: "#2563EB",
    surfaceHex: "#FAF9F6",
    skillSlug: "sketch-monochrome",
  },
  {
    id: "dir-polish-workbench",
    label: "Direction 2 · Polished Workbench",
    summary: "High-density Bento workspace with WCAG AA contrast guard.",
    accentHex: "#0F766E",
    surfaceHex: "#F8FAFC",
    skillSlug: "impeccable-polish",
  },
];

const FOCUS_OFFSET_X = 300;
const FOCUS_OFFSET_Y = 200;

export const WorkbenchSplitterDivider: React.FC<{ isDragging: boolean; onMouseDown: (e: React.MouseEvent) => void }> = ({ isDragging, onMouseDown }) => (
  <div id="splitterChatCanvas" role="separator" aria-orientation="vertical" onMouseDown={onMouseDown} style={{ width: "4px", cursor: "col-resize", backgroundColor: isDragging ? "var(--action-primary)" : "var(--border-default)", zIndex: 10, flexShrink: 0, transition: isDragging ? "none" : "background-color 0.15s ease" }} />
);

const GraphErrorToast: React.FC<{ message: string; onClose: () => void }> = ({ message, onClose }) => {
  const { t } = useTranslation();
  return (
    <div role="alert" data-testid="graph-error-toast" style={{ position: "absolute", right: "16px", bottom: "16px", zIndex: 60, maxWidth: "380px", padding: "8px 12px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--state-error)", boxShadow: "var(--shadow-lg)", color: "var(--state-error)", fontSize: "12px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px" }}>
      <span>{message}</span>
      <button type="button" onClick={onClose} aria-label={t("common.close", "Đóng")} className="cowok-btn cowok-btn-ghost" style={{ padding: "0 4px", fontSize: "12px", color: "var(--text-secondary)", cursor: "pointer" }}>×</button>
    </div>
  );
};

const FloatingChatOverlay: React.FC<{
  visible: boolean;
  activeTabId: string;
  projectId: string;
  sessionId: string;
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
  attachmentChips?: string[];
  attachmentSnippets?: Record<string, string>;
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onClearAttachments?: () => void;
  onOpenLeftChat?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({
  visible,
  activeTabId,
  projectId,
  sessionId,
  state,
  store,
  attachmentChips = [],
  attachmentSnippets,
  onRemoveAttachmentChip,
  onAttachFiles,
  onClearAttachments,
  onOpenLeftChat,
  onOpenSettings,
}) => {
  const isDeckFocus = activeTabId.includes("slide") || activeTabId.includes("deck");
  const selectedNodes = isDeckFocus ? [] : state.nodes.filter((n) => n.id === state.selectedNodeId || Boolean(state.selectedNodeIds?.includes(n.id)));
  if (!visible || (selectedNodes.length === 0 && attachmentChips.length === 0)) return null;
  return (
    <ContextualNodeFloatingChat
      projectId={projectId} sessionId={sessionId} selectedNodes={selectedNodes}
      attachmentChips={attachmentChips} attachmentSnippets={attachmentSnippets}
      onRemoveAttachmentChip={onRemoveAttachmentChip} onAttachFiles={onAttachFiles}
      onClearAttachments={onClearAttachments}
      onDeselectNode={(id) => store.selectNode(id, true)}
      onClearSelection={() => {
        if (selectedNodes.length > 0) store.selectNode(null);
        else onClearAttachments?.();
      }}
      onOpenLeftChat={onOpenLeftChat} onOpenSettings={onOpenSettings}
      onFocusNode={(id) => {
        const target = state.nodes.find((n) => n.id === id);
        if (target) store.setViewport({ panX: -target.posX + FOCUS_OFFSET_X, panY: -target.posY + FOCUS_OFFSET_Y, zoom: 1 });
      }}
    />
  );
};

export interface WorkbenchCanvasTabAreaProps {
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
  activeTabId: string;
  projectId?: string;
  chatProjectId?: string;
  sessionId?: string;
  sessionEntryKind?: "fe-dag" | "presentation";
  onTabChange: (id: string, format: CanvasTabItem["format"]) => void;
  graphHandlers: ReturnType<typeof useWorkbenchGraphHandlers>;
  onCommentOnNode: (nodeKey: string) => void;
  onAddQuoteChip?: (text: string) => void;
  onOpenLeftChat?: () => void;
  isLeftChatCollapsed?: boolean;
  attachmentChips?: string[];
  attachmentSnippets?: Record<string, string>;
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onClearAttachments?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  proposalController?: ReturnType<typeof useNodeProposalController>;
}

const SketchPolishOverlayModals: React.FC<{
  polish: ReturnType<typeof useSketchPolishController>;
}> = ({ polish }) => (
  <>
    <LockedRuleChangeReceiptCard
      receipt={polish.lockedRuleReceipt}
      onConfirmOverride={polish.handleConfirmRuleOverride}
      onDismiss={polish.dismissLockedRuleReceipt}
    />
    <VisualClarificationModal
      isOpen={polish.isClarifyOpen}
      variants={[]}
      liveDirections={polish.clarifySession?.directions ?? [...DEFAULT_LIVE_DIRECTIONS]}
      activeDirectionId={polish.clarifySession?.activeDirectionId ?? DEFAULT_LIVE_DIRECTIONS[0]!.id}
      generationEpoch={polish.clarifySession?.generationEpoch ?? 1}
      goalPrompt={polish.goalPrompt}
      clarifyLocale={polish.clarifyLocale}
      referenceApps={polish.referenceApps}
      themeModes={polish.themeModes}
      selectedPopularComponents={polish.selectedPopularComponents}
      onChangeGoalPrompt={polish.setGoalPrompt}
      onChangeClarifyLocale={polish.setClarifyLocale}
      onChangeReferenceApps={polish.setReferenceApps}
      onToggleThemeMode={polish.toggleThemeMode}
      onTogglePopularComponent={polish.togglePopularComponent}
      onSwitchLiveDirection={(dirId) => { void polish.handleSwitchLiveDirection(dirId); }}
      onConfirm={polish.handleConfirmStyleDirection}
      onClose={polish.handleCancelStyleModal}
    />
  </>
);

const TOOLBAR_STRIP_Z_INDEX = 35;
const STEERING_POPOVER_Z_INDEX = 40;
const ICON_SIZE_COMPACT = 12;
const ICON_SIZE_CHEVRON = 11;
const ICON_STROKE_MONO = 1.85;

const COMPACT_TOOLBAR_STRIP_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "row",
  flexWrap: "nowrap",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  height: "34px",
  padding: "0 10px",
  boxSizing: "border-box",
  position: "relative",
  overflow: "visible",
  zIndex: TOOLBAR_STRIP_Z_INDEX,
  backgroundColor: "var(--bg-surface)",
  borderBottom: "1px solid var(--border-default)",
};

const STEERING_POPOVER_PANEL_STYLE: React.CSSProperties = {
  position: "absolute",
  top: "calc(100% + 6px)",
  right: 0,
  minWidth: "240px",
  padding: "8px",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-lg)",
  zIndex: STEERING_POPOVER_Z_INDEX,
};

function useDismissPopover(
  isOpen: boolean,
  ref: React.RefObject<HTMLDivElement | null>,
  onClose: () => void
): void {
  useEffect(() => {
    if (!isOpen) return;
    const onPointerDown = (evt: MouseEvent) => {
      if (ref.current && !ref.current.contains(evt.target as Node)) onClose();
    };
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isOpen, ref, onClose]);
}

const SteeringChipsPopover: React.FC<{
  polish: ReturnType<typeof useSketchPolishController>;
}> = ({ polish }) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  useDismissPopover(isOpen, popoverRef, () => setIsOpen(false));
  const hasActive = Boolean(polish.activeChipId);

  return (
    <div ref={popoverRef} style={{ position: "relative", display: "inline-flex", alignItems: "center", flexShrink: 0 }}>
      <button
        type="button"
        className={`sp-chip-btn${hasActive || isOpen ? " sp-chip-btn--active" : ""}`}
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        data-testid="steering-chips-popover-btn"
        onClick={() => setIsOpen((prev) => !prev)}
        style={{ whiteSpace: "nowrap" }}
      >
        <BrandIcon name="sliders" size={ICON_SIZE_COMPACT} strokeWidth={ICON_STROKE_MONO} color="currentColor" />
        <span>{t("sketchPolish.steeringPopoverBtn", "Tinh chỉnh")}</span>
        <BrandIcon name="chevron-down" size={ICON_SIZE_CHEVRON} strokeWidth={ICON_STROKE_MONO} color="currentColor" />
      </button>
      {isOpen && (
        <div role="dialog" aria-label={t("sketchPolish.steeringChipsLabel", "Tinh chỉnh nhanh")} data-testid="steering-chips-popover-panel" style={STEERING_POPOVER_PANEL_STYLE}>
          <SteeringChipsBar
            activeChipId={polish.activeChipId}
            isApplying={polish.isApplying}
            canUndo={polish.canUndo}
            onSelectChip={polish.handleSelectChip}
            onUndo={polish.handleUndo}
          />
        </div>
      )}
    </div>
  );
};

const SketchPolishToolbarStrip: React.FC<{
  polish: ReturnType<typeof useSketchPolishController>;
}> = ({ polish }) => (
  <div className="sp-toolbar-strip" data-testid="sketch-polish-strip" style={COMPACT_TOOLBAR_STRIP_STYLE}>
    <StageSkinToggleBar
      skin={polish.stageSkin}
      cognitivePhase={polish.cognitivePhase}
      onChangeSkin={polish.handleChangeSkin}
      onSelectPhase={polish.handleSelectPhase}
      onAdvanceToStyle={polish.handleAdvanceToStyle}
      onApproveV1={polish.handleApproveV1}
      onToggleSketchLens={polish.handleToggleSketchLens}
      oklchWarning={polish.oklchWarning}
      lastSwapMs={polish.lastSwapMs}
      onOpenClarify={polish.handleAdvanceToStyle}
    />
    <SteeringChipsPopover polish={polish} />
  </div>
);

export const WorkbenchCanvasTabArea: React.FC<WorkbenchCanvasTabAreaProps> = ({
  state, store, activeTabId, projectId = DEFAULT_FLOW_PROJECT_ID, chatProjectId, sessionId = "session-1", sessionEntryKind,
  onTabChange, graphHandlers, onCommentOnNode, onAddQuoteChip, onOpenLeftChat, isLeftChatCollapsed = false,
  attachmentChips, attachmentSnippets, onRemoveAttachmentChip, onAttachFiles, onClearAttachments,
  onOpenSettings, proposalController,
}) => {
  const { t, locale } = useTranslation();
  const [isDeploying, setIsDeploying] = useState(false);
  const handleDeployDraft = async () => { if (!isDeploying) { setIsDeploying(true); try { await store.syncDeployDraft(projectId); } finally { setIsDeploying(false); } } };
  const handleSynthesizedFlow = (plan: SynthesizedFlowPlan) => {
    const mapped = mapSynthesizedPlanToWorkbenchGraph(plan);
    store.hydrate(mapped.nodes, mapped.transitions);
  };
  const orphanCount = findOrphanNodes(state.nodes.map((n) => ({ id: n.id })), state.transitions.map((tr) => ({ source: tr.sourceNodeId, target: tr.targetNodeId }))).length;
  const polish = useSketchPolishController({
    projectId,
    screenId: state.selectedNodeId || state.nodes[0]?.id || "default",
    locale: locale === "vi" ? "vi" : "en",
  });
  const isDeckFocus = activeTabId.includes("slide") || activeTabId.includes("deck");
  const sketchPolishBarNode = isDeckFocus ? null : <SketchPolishToolbarStrip polish={polish} />;

  return (
    <main style={{ flex: 1, minWidth: 0, height: "100%", overflow: "hidden", position: "relative", backgroundColor: "var(--bg-canvas)" }}>
      <CompanionCanvasTab projectId={projectId} sessionId={sessionId} sessionEntryKind={sessionEntryKind} activeTabId={activeTabId} onTabChange={onTabChange} selectedNodeId={state.selectedNodeId} onSelectNode={(id) => store.selectNode(id, false)} nodes={state.nodes} isDraftDirty={state.isDraftDirty} orphanCount={orphanCount} conflictState={state.conflictState} canUndo={(state.undoStack?.length || 0) > 0} canRedo={(state.redoStack?.length || 0) > 0} onUndo={() => store.undo()} onRedo={() => store.redo()} onResolveConflict={(c) => { void store.resolveConflict(c); }} onDeployDraft={() => { void handleDeployDraft(); }} onDiscardDraft={() => store.discardDraft()} onAttachRegionToChat={onCommentOnNode} onQuoteText={onAddQuoteChip} onSelectNodeSubState={(nid, sub) => store.setNodeSubState(nid, sub)} proposalController={proposalController} stageSkin={polish.stageSkin} sketchPolishBarNode={sketchPolishBarNode} onRegisterStageSkinSwap={polish.onRegisterStageSkinSwap} onRegisterStageApplyHtml={polish.onRegisterStageApplyHtml} onLockedRuleReceipt={polish.handleLockedRuleNotice}>
        <ErrorBoundary fallbackTitle={t("errors.dagCanvasRender", "Lỗi hiển thị Canvas 2D DAG")} onReset={() => store.discardDraft()}>
          <DagCanvas2D state={state} projectId={projectId} sessionId={sessionId} onUpdatePosition={graphHandlers.handleUpdatePosition} onSetViewport={graphHandlers.handleSetViewport} onSelectNode={(id, multi) => store.selectNode(id, multi)} onSelectTransition={(id) => store.selectTransition(id)} onAddNode={graphHandlers.handleAddNode} onAutoLayout={() => store.autoLayoutNodes()} onSelectSubState={(id, sub) => store.setNodeSubState(id, sub)} onDuplicateNode={(id) => store.duplicateNode(id)} onRenameNode={(id, title) => store.renameNode(id, title)} onDeleteNode={(id) => store.deleteNode(id)} onAddTransition={graphHandlers.handleAddTransition} onUpdateTransition={(id, u) => store.updateTransition(id, u)} onReverseTransition={graphHandlers.handleReverseTransition} onDeleteTransition={(id) => store.deleteTransition(id)} onUpdateNodeNote={(id, note) => store.updateNodeNote(id, note)} onToggleMinimap={() => store.toggleMinimap()} onUndo={() => store.undo()} onRedo={() => store.redo()} onCommentOnNode={onCommentOnNode} onSynthesizedFlow={handleSynthesizedFlow} />
        </ErrorBoundary>
      </CompanionCanvasTab>
      <SketchPolishOverlayModals polish={polish} />
      <FloatingChatOverlay
        visible={isLeftChatCollapsed}
        activeTabId={activeTabId}
        projectId={chatProjectId ?? ""}
        sessionId={sessionId}
        state={state}
        store={store}
        attachmentChips={attachmentChips}
        attachmentSnippets={attachmentSnippets}
        onRemoveAttachmentChip={onRemoveAttachmentChip}
        onAttachFiles={onAttachFiles}
        onClearAttachments={onClearAttachments}
        onOpenLeftChat={onOpenLeftChat}
        onOpenSettings={onOpenSettings}
      />
      {graphHandlers.graphError && <GraphErrorToast message={graphHandlers.graphError} onClose={graphHandlers.clearGraphError} />}
    </main>
  );
};
