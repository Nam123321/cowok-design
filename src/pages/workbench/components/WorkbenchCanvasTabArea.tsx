// @story 04.1, 04.4
// @implements AC-01, AC-02, AC-06, AC-07, AC-08
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { FlowWorkbenchState, createNewFlowWorkbenchStore } from "../../../features/flow/store/flow-workbench-store";
import { CompanionCanvasTab, CanvasTabItem } from "../../../features/flow/components/CompanionCanvasTab";
import { DagCanvas2D } from "../../../features/flow/components/DagCanvas2D";
import { ContextualNodeFloatingChat } from "../../../features/flow/components/ContextualNodeFloatingChat";
import { ErrorBoundary } from "../../../components/ui/ErrorBoundary";
import { findOrphanNodes } from "../../../features/flow/utils/graph-analysis";
import { useWorkbenchGraphHandlers } from "../hooks/useWorkbenchGraphHandlers";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";

export const WorkbenchSplitterDivider: React.FC<{
  isDragging: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
}> = ({ isDragging, onMouseDown }) => (
  <div
    id="splitterChatCanvas" role="separator" aria-orientation="vertical" onMouseDown={onMouseDown}
    style={{ width: "4px", cursor: "col-resize", backgroundColor: isDragging ? "var(--action-primary)" : "var(--border-default)", zIndex: 10, flexShrink: 0, transition: isDragging ? "none" : "background-color 0.15s ease" }}
  />
);

export const WorkbenchCanvasTabArea: React.FC<{
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
  activeTabId: string;
  projectId?: string;
  sessionId?: string;
  onTabChange: (id: string, format: CanvasTabItem["format"]) => void;
  graphHandlers: ReturnType<typeof useWorkbenchGraphHandlers>;
  onCommentOnNode: (nodeKey: string) => void;
  onOpenLeftChat?: () => void;
  isLeftChatCollapsed?: boolean;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({
  state,
  store,
  activeTabId,
  projectId = DEFAULT_FLOW_PROJECT_ID,
  sessionId = "session-1",
  onTabChange,
  graphHandlers,
  onCommentOnNode,
  onOpenLeftChat,
  isLeftChatCollapsed = false,
  onOpenSettings,
}) => {
  const { t } = useTranslation();
  const [isDeploying, setIsDeploying] = useState(false);
  const handleDeployDraft = async () => {
    if (isDeploying) return;
    setIsDeploying(true);
    try { await store.syncDeployDraft(projectId); } finally { setIsDeploying(false); }
  };

  const selectedNodes = state.nodes.filter(
    (n) => n.id === state.selectedNodeId || (state.selectedNodeIds && state.selectedNodeIds.includes(n.id))
  );
  const orphanCount = findOrphanNodes(
    state.nodes.map((n) => ({ id: n.id })),
    state.transitions.map((tr) => ({ source: tr.sourceNodeId, target: tr.targetNodeId }))
  ).length;

  return (
    <main style={{ flex: 1, minWidth: "550px", height: "100%", overflow: "hidden", position: "relative", backgroundColor: "var(--bg-canvas)" }}>
      <CompanionCanvasTab
        activeTabId={activeTabId} onTabChange={onTabChange} selectedNodeId={state.selectedNodeId}
        nodes={state.nodes}
        isDraftDirty={state.isDraftDirty} orphanCount={orphanCount} conflictState={state.conflictState}
        onResolveConflict={(c) => { void store.resolveConflict(c); }}
        onDeployDraft={() => { void handleDeployDraft(); }}
        onDiscardDraft={() => store.discardDraft()} onAttachRegionToChat={onCommentOnNode}
      >
        <ErrorBoundary fallbackTitle={t("errors.dagCanvasRender", "Lỗi hiển thị Canvas 2D DAG")} onReset={() => store.discardDraft()}>
          <DagCanvas2D
            state={state} projectId={projectId} sessionId={sessionId}
            onUpdatePosition={graphHandlers.handleUpdatePosition} onSetViewport={graphHandlers.handleSetViewport}
            onSelectNode={(id, multi) => store.selectNode(id, multi)} onSelectTransition={(id) => store.selectTransition(id)}
            onAddNode={graphHandlers.handleAddNode} onAutoLayout={() => store.autoLayoutNodes()}
            onSelectSubState={(id, sub) => store.setNodeSubState(id, sub)} onDuplicateNode={(id) => store.duplicateNode(id)}
            onRenameNode={(id, title) => store.renameNode(id, title)} onDeleteNode={(id) => store.deleteNode(id)}
            onAddTransition={graphHandlers.handleAddTransition} onUpdateTransition={(id, u) => store.updateTransition(id, u)}
            onReverseTransition={graphHandlers.handleReverseTransition} onDeleteTransition={(id) => store.deleteTransition(id)}
            onUpdateNodeNote={(id, note) => store.updateNodeNote(id, note)} onToggleMinimap={() => store.toggleMinimap()}
            onUndo={() => store.undo()} onRedo={() => store.redo()} onCommentOnNode={onCommentOnNode}
          />
        </ErrorBoundary>
      </CompanionCanvasTab>
      {isLeftChatCollapsed && selectedNodes.length > 0 && (
        <ContextualNodeFloatingChat
          projectId={projectId}
          sessionId={sessionId}
          selectedNodes={selectedNodes}
          onDeselectNode={(id) => store.selectNode(id, true)}
          onClearSelection={() => store.selectNode(null)}
          onOpenLeftChat={onOpenLeftChat}
          onOpenSettings={onOpenSettings}
          onFocusNode={(id) => {
            const target = state.nodes.find((n) => n.id === id);
            if (target) store.setViewport({ panX: -target.posX + 300, panY: -target.posY + 200, zoom: 1 });
          }}
        />
      )}
      {graphHandlers.graphError && (
        <div
          role="alert"
          data-testid="graph-error-toast"
          style={{
            position: "absolute", right: "16px", bottom: "16px", zIndex: 60, maxWidth: "380px", padding: "8px 12px",
            borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--state-error)",
            boxShadow: "var(--shadow-lg)", color: "var(--state-error)", fontSize: "12px",
            display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px",
          }}
        >
          <span>{graphHandlers.graphError}</span>
          <button
            type="button" onClick={graphHandlers.clearGraphError} aria-label={t("common.close", "Đóng")}
            className="cowok-btn cowok-btn-ghost"
            style={{ padding: "0 4px", fontSize: "12px", color: "var(--text-secondary)", cursor: "pointer" }}
          >
            ✕
          </button>
        </div>
      )}
    </main>
  );
};
