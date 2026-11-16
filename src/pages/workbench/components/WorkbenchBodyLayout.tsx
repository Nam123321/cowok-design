// @story 04.1, 04.4
// @implements AC-01, AC-02, AC-04, AC-06, AC-07, AC-08
import React from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { useTranslation } from "../../../hooks/useTranslation";
import { FlowWorkbenchState, createNewFlowWorkbenchStore } from "../../../features/flow/store/flow-workbench-store";
import { sharedChatStore } from "../../../features/flow/store/shared-chat-sync";
import { MainSidebar, ChatSession } from "./MainSidebar";
import { AgentChatPanel } from "./AgentChatPanel";
import { CanvasTabItem } from "../../../features/flow/components/CompanionCanvasTab";
import { useWorkbenchSplitter } from "../hooks/useWorkbenchSplitter";
import { useWorkbenchAttachments } from "../hooks/useWorkbenchAttachments";
import { useWorkbenchGraphHandlers } from "../hooks/useWorkbenchGraphHandlers";
import { ThemeMode } from "./WorkbenchTopNav";
import { DEFAULT_FLOW_PROJECT_ID, WorkbenchProject } from "../constants";
import { WorkbenchCanvasTabArea, WorkbenchSplitterDivider } from "./WorkbenchCanvasTabArea";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

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

  const handleSelectNodeSubSession = (compositeKey: string, projId: string, sessId: string, nodeId: string) => {
    p.onSelectSession(sessId);
    p.store.selectNode(nodeId);
    sharedChatStore.markScopeCleared(compositeKey);
    if (UUID_RE.test(projId)) {
      void apiFlowExecution
        .markNodeSessionRead(projId, { sessionId: sessId, nodeId })
        .then(() => sharedChatStore.bumpSubSessionsVersion())
        .catch(() => { /* ignore when offline */ });
    }
    const target = p.state.nodes.find((n) => n.id === nodeId);
    if (target) {
      p.store.setViewport({ panX: -target.posX + 300, panY: -target.posY + 200, zoom: 1 });
    }
  };

  const activeSubSessionKey = p.state.selectedNodeId
    ? `${resolvedProjectId}::${p.activeSessionId}::${p.state.selectedNodeId}`
    : null;

  const selectedScopeNodes = p.state.nodes.filter(
    (n) => n.id === p.state.selectedNodeId || (p.state.selectedNodeIds && p.state.selectedNodeIds.includes(n.id))
  );

  return (
    <div className="workbench-body-layout" aria-label={t("workbench.appName", "cowok-design")} style={{ display: "flex", flex: 1, overflowX: "auto", overflowY: "hidden", position: "relative", scrollBehavior: "smooth", width: "100%", height: "100%" }}>
      <MainSidebar
        projectName={p.projectName} projectId={resolvedProjectId} projects={p.projects} sessions={p.sessions}
        activeSessionId={p.activeSessionId} nodes={p.state.nodes} activeSubSessionKey={activeSubSessionKey}
        onSelectSession={p.onSelectSession} onSelectNodeSubSession={handleSelectNodeSubSession}
        onOpenNewConversationModal={p.onOpenNewConversationModal} onOpenSettings={() => p.onOpenSettings()}
        currentLocale={p.currentLocale} themeMode={p.themeMode} onThemeChange={p.onThemeChange}
        isCollapsed={p.isSidebarCollapsed} onToggleCollapse={p.onToggleSidebar}
        onToggleProjectFolder={p.onToggleProjectFolder} onCreateSessionInProject={p.onCreateSessionInProject}
        onTogglePinSession={p.onTogglePinSession} onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession}
        onRenameProject={p.onRenameProject} onDeleteProject={p.onDeleteProject} onLinkProjectFolder={p.onLinkProjectFolder}
      />
      <div className="dual-main-panel" style={{ display: "flex", flex: 1, minWidth: "650px", overflow: "hidden", position: "relative" }}>
        <AgentChatPanel
          projectId={resolvedProjectId} activeSessionId={p.activeSessionId} selectedNodes={selectedScopeNodes}
          onDeselectNode={(id) => p.store.selectNode(id, true)}
          onFocusNode={(id) => {
            const target = p.state.nodes.find((n) => n.id === id);
            if (target) p.store.setViewport({ panX: -target.posX + 300, panY: -target.posY + 200, zoom: 1 });
          }}
          width={p.splitter.chatWidth} isShortened={p.splitter.isChatShortened}
          onToggleShorten={p.splitter.handleToggleShortenChat} attachmentChips={p.attachments.attachmentChips}
          onRemoveAttachmentChip={p.attachments.handleRemoveAttachmentChip} onOpenSettings={p.onOpenSettings}
        />
        <WorkbenchSplitterDivider isDragging={p.splitter.isDraggingSplitter} onMouseDown={p.splitter.handleSplitterMouseDown} />
        <WorkbenchCanvasTabArea
          state={p.state} store={p.store} projectId={resolvedProjectId} sessionId={p.activeSessionId}
          activeTabId={p.activeTabId} onTabChange={p.onTabChange} graphHandlers={p.graphHandlers}
          onCommentOnNode={p.attachments.handleCommentOnNode} onAddQuoteChip={p.attachments.handleAddQuoteChip}
          isLeftChatCollapsed={p.splitter.isChatShortened}
          onOpenSettings={p.onOpenSettings}
          onOpenLeftChat={p.splitter.isChatShortened ? p.splitter.handleToggleShortenChat : undefined}
        />
      </div>
      {p.attachments.quoteTooltip?.visible && (
        <div
          className="quote-floating-pill" data-testid="quote-floating-pill"
          style={{
            position: "fixed", left: `${p.attachments.quoteTooltip.x}px`, top: `${p.attachments.quoteTooltip.y}px`,
            zIndex: 100, display: "flex", alignItems: "center", gap: "4px", padding: "4px 10px",
            borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--action-primary)",
            boxShadow: "var(--shadow-lg)", color: "var(--text-primary)", fontSize: "11px", fontWeight: 600, cursor: "pointer", userSelect: "none",
          }}
          onClick={() => { if (p.attachments.quoteTooltip) p.attachments.handleAddQuoteChip(p.attachments.quoteTooltip.text); }}
        >
          <span>{t("chat.quoteToChat", "Quote vào Chat")}</span>
        </div>
      )}
    </div>
  );
};
