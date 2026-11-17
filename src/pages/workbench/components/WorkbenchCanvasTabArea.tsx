// @story 04.1, 04.4, 04.5
// @implements AC-01, AC-02, AC-06, AC-07, AC-08
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { FlowWorkbenchState, createNewFlowWorkbenchStore } from "../../../features/flow/store/flow-workbench-store";
import { CompanionCanvasTab, CanvasTabItem } from "../../../features/flow/components/CompanionCanvasTab";
import { DagCanvas2D } from "../../../features/flow/components/DagCanvas2D";
import { ContextualNodeFloatingChat } from "../../../features/flow/components/ContextualNodeFloatingChat";
import type { useNodeProposalController } from "../../../features/proposals/hooks/useNodeProposalController";
import { ErrorBoundary } from "../../../components/ui/ErrorBoundary";
import { findOrphanNodes } from "../../../features/flow/utils/graph-analysis";
import { useWorkbenchGraphHandlers } from "../hooks/useWorkbenchGraphHandlers";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";

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
  projectId: string;
  sessionId: string;
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
  onOpenLeftChat?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({ visible, projectId, sessionId, state, store, onOpenLeftChat, onOpenSettings }) => {
  const selectedNodes = state.nodes.filter((n) => n.id === state.selectedNodeId || Boolean(state.selectedNodeIds?.includes(n.id)));
  if (!visible || selectedNodes.length === 0) return null;
  return (
    <ContextualNodeFloatingChat
      projectId={projectId} sessionId={sessionId} selectedNodes={selectedNodes}
      onDeselectNode={(id) => store.selectNode(id, true)} onClearSelection={() => store.selectNode(null)}
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
  sessionId?: string;
  onTabChange: (id: string, format: CanvasTabItem["format"]) => void;
  graphHandlers: ReturnType<typeof useWorkbenchGraphHandlers>;
  onCommentOnNode: (nodeKey: string) => void;
  onAddQuoteChip?: (text: string) => void;
  onOpenLeftChat?: () => void;
  isLeftChatCollapsed?: boolean;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  proposalController?: ReturnType<typeof useNodeProposalController>;
}

export const WorkbenchCanvasTabArea: React.FC<WorkbenchCanvasTabAreaProps> = ({
  state, store, activeTabId, projectId = DEFAULT_FLOW_PROJECT_ID, sessionId = "session-1",
  onTabChange, graphHandlers, onCommentOnNode, onAddQuoteChip, onOpenLeftChat, isLeftChatCollapsed = false, onOpenSettings, proposalController,
}) => {
  const { t } = useTranslation();
  const [isDeploying, setIsDeploying] = useState(false);
  const handleDeployDraft = async () => { if (!isDeploying) { setIsDeploying(true); try { await store.syncDeployDraft(projectId); } finally { setIsDeploying(false); } } };
  const orphanCount = findOrphanNodes(state.nodes.map((n) => ({ id: n.id })), state.transitions.map((tr) => ({ source: tr.sourceNodeId, target: tr.targetNodeId }))).length;

  return (
    <main style={{ flex: 1, minWidth: "550px", height: "100%", overflow: "hidden", position: "relative", backgroundColor: "var(--bg-canvas)" }}>
      <CompanionCanvasTab projectId={projectId} activeTabId={activeTabId} onTabChange={onTabChange} selectedNodeId={state.selectedNodeId} onSelectNode={(id) => store.selectNode(id, false)} nodes={state.nodes} isDraftDirty={state.isDraftDirty} orphanCount={orphanCount} conflictState={state.conflictState} onResolveConflict={(c) => { void store.resolveConflict(c); }} onDeployDraft={() => { void handleDeployDraft(); }} onDiscardDraft={() => store.discardDraft()} onAttachRegionToChat={onCommentOnNode} onQuoteText={onAddQuoteChip} onSelectNodeSubState={(nid, sub) => store.setNodeSubState(nid, sub)} proposalController={proposalController}>
        <ErrorBoundary fallbackTitle={t("errors.dagCanvasRender", "Lỗi hiển thị Canvas 2D DAG")} onReset={() => store.discardDraft()}>
          <DagCanvas2D state={state} projectId={projectId} sessionId={sessionId} onUpdatePosition={graphHandlers.handleUpdatePosition} onSetViewport={graphHandlers.handleSetViewport} onSelectNode={(id, multi) => store.selectNode(id, multi)} onSelectTransition={(id) => store.selectTransition(id)} onAddNode={graphHandlers.handleAddNode} onAutoLayout={() => store.autoLayoutNodes()} onSelectSubState={(id, sub) => store.setNodeSubState(id, sub)} onDuplicateNode={(id) => store.duplicateNode(id)} onRenameNode={(id, title) => store.renameNode(id, title)} onDeleteNode={(id) => store.deleteNode(id)} onAddTransition={graphHandlers.handleAddTransition} onUpdateTransition={(id, u) => store.updateTransition(id, u)} onReverseTransition={graphHandlers.handleReverseTransition} onDeleteTransition={(id) => store.deleteTransition(id)} onUpdateNodeNote={(id, note) => store.updateNodeNote(id, note)} onToggleMinimap={() => store.toggleMinimap()} onUndo={() => store.undo()} onRedo={() => store.redo()} onCommentOnNode={onCommentOnNode} />
        </ErrorBoundary>
      </CompanionCanvasTab>
      <FloatingChatOverlay visible={isLeftChatCollapsed} projectId={projectId} sessionId={sessionId} state={state} store={store} onOpenLeftChat={onOpenLeftChat} onOpenSettings={onOpenSettings} />
      {graphHandlers.graphError && <GraphErrorToast message={graphHandlers.graphError} onClose={graphHandlers.clearGraphError} />}
    </main>
  );
};
