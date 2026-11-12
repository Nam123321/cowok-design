// @story 04.1
// @implements AC-01, AC-02, AC-06, AC-07, AC-08
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { FlowWorkbenchState, createNewFlowWorkbenchStore } from "../../../features/flow/store/flow-workbench-store";
import { MainSidebar, ChatSession } from "./MainSidebar";
import { AgentChatPanel } from "./AgentChatPanel";
import { CompanionCanvasTab, CanvasTabItem } from "../../../features/flow/components/CompanionCanvasTab";
import { DagCanvas2D } from "../../../features/flow/components/DagCanvas2D";
import { ContextualNodeFloatingChat } from "../../../features/flow/components/ContextualNodeFloatingChat";
import { ErrorBoundary } from "../../../components/ui/ErrorBoundary";
import { useWorkbenchSplitter } from "../hooks/useWorkbenchSplitter";
import { useWorkbenchAttachments } from "../hooks/useWorkbenchAttachments";
import { useWorkbenchGraphHandlers } from "../hooks/useWorkbenchGraphHandlers";
import { ThemeMode } from "./WorkbenchTopNav";
import { DEFAULT_FLOW_PROJECT_ID, WorkbenchProject } from "../constants";

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
}) => {
  const [isDeploying, setIsDeploying] = useState(false);
  const handleDeployDraft = async () => {
    if (isDeploying) return;
    setIsDeploying(true);
    try { await store.syncDeployDraft(projectId); } finally { setIsDeploying(false); }
  };

  const selectedNodes = state.nodes.filter(
    (n) => n.id === state.selectedNodeId || (state.selectedNodeIds && state.selectedNodeIds.includes(n.id))
  );

  return (
    <main style={{ flex: 1, minWidth: "550px", height: "100%", overflow: "hidden", position: "relative", backgroundColor: "var(--bg-canvas)" }}>
      <CompanionCanvasTab
        activeTabId={activeTabId} onTabChange={onTabChange} selectedNodeId={state.selectedNodeId}
        isDraftDirty={state.isDraftDirty} conflictState={state.conflictState}
        onResolveConflict={(c) => { void store.resolveConflict(c); }}
        onDeployDraft={() => { void handleDeployDraft(); }}
        onDiscardDraft={() => store.discardDraft()} onAttachRegionToChat={onCommentOnNode}
      >
        <ErrorBoundary fallbackTitle="Lỗi hiển thị Canvas 2D DAG" onReset={() => store.discardDraft()}>
          <DagCanvas2D
            state={state} onUpdatePosition={graphHandlers.handleUpdatePosition} onSetViewport={graphHandlers.handleSetViewport}
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
      {selectedNodes.length > 0 && (
        <ContextualNodeFloatingChat
          projectId={projectId}
          sessionId={sessionId}
          selectedNodes={selectedNodes}
          onDeselectNode={(id) => {
            store.selectNode(state.selectedNodeId === id ? null : state.selectedNodeId);
          }}
          onClearSelection={() => store.selectNode(null)}
          onOpenLeftChat={onOpenLeftChat}
          onFocusNode={(id) => {
            const target = state.nodes.find((n) => n.id === id);
            if (target) {
              store.setViewport({ panX: -target.posX + 300, panY: -target.posY + 200, zoom: 1 });
            }
          }}
        />
      )}
    </main>
  );
};

export interface WorkbenchBodyLayoutProps {
  projectName: string;
  projectId?: string;
  projects: WorkbenchProject[];
  sessions: ChatSession[];
  activeSessionId: string;
  onSelectSession: (id: string) => void;
  onOpenNewConversationModal: () => void;
  onOpenSettings: (tab?: "general" | "models" | "mcp" | "automations") => void;
  currentLocale: string;
  themeMode: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  isSidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  onToggleProjectFolder: (projId: string) => void;
  onCreateSessionInProject?: (projId: string) => void;
  onTogglePinSession?: (sessionId: string) => void;
  onRenameSession?: (sessionId: string, newTitle: string) => void;
  onDeleteSession?: (sessionId: string) => void;
  onRenameProject?: (projId: string, newName: string) => void;
  onDeleteProject?: (projId: string) => void;
  onLinkProjectFolder?: (projId: string) => void;
  activeTabId: string;
  onTabChange: (id: string, format: CanvasTabItem["format"]) => void;
  splitter: ReturnType<typeof useWorkbenchSplitter>;
  attachments: ReturnType<typeof useWorkbenchAttachments>;
  graphHandlers: ReturnType<typeof useWorkbenchGraphHandlers>;
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
}

export const WorkbenchBodyLayout: React.FC<WorkbenchBodyLayoutProps> = (p) => {
  const { t } = useTranslation();
  const resolvedProjectId = p.projectId || DEFAULT_FLOW_PROJECT_ID;

  const handleSelectNodeSubSession = (
    _compositeKey: string,
    _projId: string,
    sessId: string,
    nodeId: string
  ) => {
    p.onSelectSession(sessId);
    p.store.selectNode(nodeId);
    const target = p.state.nodes.find((n) => n.id === nodeId);
    if (target) {
      p.store.setViewport({ panX: -target.posX + 300, panY: -target.posY + 200, zoom: 1 });
    }
  };

  const activeSubSessionKey = p.state.selectedNodeId
    ? `${resolvedProjectId}::${p.activeSessionId}::${p.state.selectedNodeId}`
    : null;

  return (
    <div className="workbench-body-layout" aria-label={t("workbench.appName", "cowok-design")} style={{ display: "flex", flex: 1, overflowX: "auto", overflowY: "hidden", position: "relative", scrollBehavior: "smooth", width: "100%", height: "100%" }}>
      <MainSidebar
        projectName={p.projectName}
        projectId={resolvedProjectId}
        projects={p.projects}
        sessions={p.sessions}
        activeSessionId={p.activeSessionId}
        nodes={p.state.nodes}
        activeSubSessionKey={activeSubSessionKey}
        onSelectSession={p.onSelectSession}
        onSelectNodeSubSession={handleSelectNodeSubSession}
        onOpenNewConversationModal={p.onOpenNewConversationModal}
        onOpenSettings={() => p.onOpenSettings()}
        currentLocale={p.currentLocale}
        themeMode={p.themeMode}
        onThemeChange={p.onThemeChange}
        isCollapsed={p.isSidebarCollapsed}
        onToggleCollapse={p.onToggleSidebar}
        onToggleProjectFolder={p.onToggleProjectFolder}
        onCreateSessionInProject={p.onCreateSessionInProject}
        onTogglePinSession={p.onTogglePinSession}
        onRenameSession={p.onRenameSession}
        onDeleteSession={p.onDeleteSession}
        onRenameProject={p.onRenameProject}
        onDeleteProject={p.onDeleteProject}
        onLinkProjectFolder={p.onLinkProjectFolder}
      />
      <div className="dual-main-panel" style={{ display: "flex", flex: 1, minWidth: "650px", overflow: "hidden", position: "relative" }}>
        <AgentChatPanel
          width={p.splitter.chatWidth} isShortened={p.splitter.isChatShortened}
          onToggleShorten={p.splitter.handleToggleShortenChat} attachmentChips={p.attachments.attachmentChips}
          onRemoveAttachmentChip={p.attachments.handleRemoveAttachmentChip} onOpenSettings={p.onOpenSettings}
        />
        <WorkbenchSplitterDivider isDragging={p.splitter.isDraggingSplitter} onMouseDown={p.splitter.handleSplitterMouseDown} />
        <WorkbenchCanvasTabArea
          state={p.state}
          store={p.store}
          projectId={resolvedProjectId}
          sessionId={p.activeSessionId}
          activeTabId={p.activeTabId}
          onTabChange={p.onTabChange}
          graphHandlers={p.graphHandlers}
          onCommentOnNode={p.attachments.handleCommentOnNode}
          onOpenLeftChat={p.splitter.isChatShortened ? p.splitter.handleToggleShortenChat : undefined}
        />
      </div>
      {p.attachments.quoteTooltip?.visible && (
        <div
          className="quote-floating-pill"
          data-testid="quote-floating-pill"
          style={{
            position: "fixed",
            left: `${p.attachments.quoteTooltip.x}px`,
            top: `${p.attachments.quoteTooltip.y}px`,
            zIndex: 100,
            display: "flex",
            alignItems: "center",
            gap: "4px",
            padding: "4px 10px",
            borderRadius: "var(--radius-pill)",
            backgroundColor: "var(--bg-elevated)",
            border: "1px solid var(--action-primary)",
            boxShadow: "var(--shadow-lg)",
            color: "var(--text-primary)",
            fontSize: "11px",
            fontWeight: 600,
            cursor: "pointer",
            userSelect: "none",
          }}
          onClick={() => {
            if (p.attachments.quoteTooltip) {
              p.attachments.handleAddQuoteChip(p.attachments.quoteTooltip.text);
            }
          }}
        >
          <span>❝ Quote vào Chat</span>
        </div>
      )}
    </div>
  );
};
