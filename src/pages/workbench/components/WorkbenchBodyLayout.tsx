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
import { useNodeProposalController } from "../../../features/proposals/hooks/useNodeProposalController";
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
  onCreateSessionInProject?: (projId: string, title?: string, entryKind?: "fe-dag" | "presentation") => void;
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
  isModalOpen?: boolean;
  onOpenVisualQc?: () => void;
  onOpenTournament?: () => void;
  onOpenCertification?: () => void;
  activeTournamentRunCount?: number;
  state: FlowWorkbenchState;
  store: ReturnType<typeof createNewFlowWorkbenchStore>;
}

const VIEWPORT_FOCUS_OFFSET_X = 300;
const VIEWPORT_FOCUS_OFFSET_Y = 200;
const QUOTE_PILL_Z_INDEX = 100;
const FONT_WEIGHT_SEMIBOLD = 600;

function useWorkbenchBodyController(p: WorkbenchBodyLayoutProps) {
  const resolvedProjectId = p.projectId || DEFAULT_FLOW_PROJECT_ID;
  const focusNode = (nodeId: string) => {
    const target = p.state.nodes.find((n) => n.id === nodeId);
    if (target) p.store.setViewport({ panX: -target.posX + VIEWPORT_FOCUS_OFFSET_X, panY: -target.posY + VIEWPORT_FOCUS_OFFSET_Y, zoom: 1 });
  };
  const handleSelectNodeSubSession = (compositeKey: string, projId: string, sessId: string, nodeId: string) => {
    p.onSelectSession(sessId);
    p.store.selectNode(nodeId);
    sharedChatStore.markScopeCleared(compositeKey);
    if (UUID_RE.test(projId)) {
      void apiFlowExecution.markNodeSessionRead(projId, { sessionId: sessId, nodeId }).then(() => sharedChatStore.bumpSubSessionsVersion()).catch(() => {});
    }
    focusNode(nodeId);
  };
  const selectedScopeNodes = p.state.nodes.filter((n) => n.id === p.state.selectedNodeId || (p.state.selectedNodeIds && p.state.selectedNodeIds.includes(n.id)));
  const primaryNode = selectedScopeNodes.find((n) => n.id === p.state.selectedNodeId) || selectedScopeNodes[0] || null;
  const proposalCtrl = useNodeProposalController({
    projectId: resolvedProjectId, selectedNodeId: p.state.selectedNodeId,
    selectedNodeKey: primaryNode?.screenKey || null, isModalOpen: Boolean(p.isModalOpen), isInlineCodeOpen: false,
  });
  const isChatShortened = p.splitter.isChatShortened;
  const toggleShortenChat = p.splitter.handleToggleShortenChat;
  React.useEffect(() => {
    const onSlideStudioEntered = () => { if (!isChatShortened) toggleShortenChat(); };
    window.addEventListener("cowok:slide-studio-entered", onSlideStudioEntered);
    return () => window.removeEventListener("cowok:slide-studio-entered", onSlideStudioEntered);
  }, [isChatShortened, toggleShortenChat]);
  return { resolvedProjectId, focusNode, handleSelectNodeSubSession, selectedScopeNodes, primaryNode, proposalCtrl };
}

const QuoteFloatingPill: React.FC<{ quoteTooltip: WorkbenchBodyLayoutProps["attachments"]["quoteTooltip"]; onQuote: (text: string) => void }> = ({ quoteTooltip, onQuote }) => {
  const { t } = useTranslation();
  if (!quoteTooltip?.visible) return null;
  return (
    <div
      className="quote-floating-pill" data-testid="quote-floating-pill"
      onClick={() => onQuote(quoteTooltip.text)}
      style={{ position: "fixed", left: `${quoteTooltip.x}px`, top: `${quoteTooltip.y}px`, zIndex: QUOTE_PILL_Z_INDEX, display: "flex", alignItems: "center", gap: "4px", padding: "4px 10px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--action-primary)", boxShadow: "var(--shadow-lg)", color: "var(--text-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", userSelect: "none" }}
    >
      <span>{t("chat.quoteToChat", "Quote vào Chat")}</span>
    </div>
  );
};

export const WorkbenchBodyLayout: React.FC<WorkbenchBodyLayoutProps> = (p) => {
  const { t } = useTranslation();
  const { resolvedProjectId, focusNode, handleSelectNodeSubSession, selectedScopeNodes, primaryNode, proposalCtrl } = useWorkbenchBodyController(p);
  const activeSubSessionKey = p.state.selectedNodeId ? `${resolvedProjectId}::${p.activeSessionId}::${p.state.selectedNodeId}` : null;
  const activeSession = p.sessions.find((s) => s.id === p.activeSessionId);
  const chatProjectId = activeSession && !activeSession.projectId ? undefined : resolvedProjectId;
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
      <div className="dual-main-panel" style={{ display: "flex", flex: 1, minWidth: 0, overflow: "hidden", position: "relative" }}>
        <AgentChatPanel
          projectId={chatProjectId} activeSessionId={p.activeSessionId} selectedNodes={selectedScopeNodes}
          onDeselectNode={(id) => p.store.selectNode(id, true)} onFocusNode={focusNode}
          width={p.splitter.chatWidth} isShortened={p.splitter.isChatShortened}
          onToggleShorten={p.splitter.handleToggleShortenChat} attachmentChips={p.attachments.attachmentChips}
          attachmentSnippets={p.attachments.attachmentSnippets}
          onRemoveAttachmentChip={p.attachments.handleRemoveAttachmentChip} onAttachFiles={p.attachments.handleAddFileChips}
          onClearAttachments={p.attachments.handleClearAttachments} onOpenSettings={p.onOpenSettings}
          proposalControls={{ selectedNodeId: p.state.selectedNodeId, selectedNodeKey: primaryNode?.screenKey || null, proposalsCount: proposalCtrl.proposalsCount, isDesignerTalkOpen: proposalCtrl.isDesignerTalkOpen, isDrawerOpen: proposalCtrl.isDrawerOpen, onToggleDesignerTalk: () => proposalCtrl.setIsDesignerTalkOpen((prev) => !prev), onToggleDrawer: () => proposalCtrl.setIsDrawerOpen((prev) => !prev) }}
        />
        <WorkbenchSplitterDivider isDragging={p.splitter.isDraggingSplitter} onMouseDown={p.splitter.handleSplitterMouseDown} />
        <WorkbenchCanvasTabArea
          state={p.state} store={p.store} projectId={resolvedProjectId} chatProjectId={chatProjectId} sessionId={p.activeSessionId}
          sessionEntryKind={activeSession?.entryKind} activeTabId={p.activeTabId} onTabChange={p.onTabChange}
          graphHandlers={p.graphHandlers} onCommentOnNode={p.attachments.handleCommentOnNode}
          onAddQuoteChip={p.attachments.handleAddQuoteChip} isLeftChatCollapsed={p.splitter.isChatShortened}
          attachmentChips={p.attachments.attachmentChips} attachmentSnippets={p.attachments.attachmentSnippets}
          onRemoveAttachmentChip={p.attachments.handleRemoveAttachmentChip}
          onAttachFiles={p.attachments.handleAddFileChips}
          onClearAttachments={p.attachments.handleClearAttachments}
          onOpenSettings={p.onOpenSettings} onOpenLeftChat={p.splitter.isChatShortened ? p.splitter.handleToggleShortenChat : undefined}
          onOpenVisualQc={p.onOpenVisualQc}
          onOpenTournament={p.onOpenTournament}
          onOpenCertification={p.onOpenCertification}
          activeTournamentRunCount={p.activeTournamentRunCount}
          proposalController={proposalCtrl}
        />
      </div>
      <QuoteFloatingPill quoteTooltip={p.attachments.quoteTooltip} onQuote={p.attachments.handleAddQuoteChip} />
    </div>
  );
};
