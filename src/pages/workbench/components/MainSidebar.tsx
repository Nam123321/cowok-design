// @story 04.1
// cowok-design — Canonical Main Sidebar & Seamless Session Tree (DESIGN.md v4.3-OS)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { DEFAULT_FLOW_PROJECT_ID, type WorkbenchProject } from "../constants";
import type { ProjectDto } from "../../../lib/validations/harness-daemon";
import type { ChatSession } from "../types";
import { SidebarCollapsedRail } from "./SidebarCollapsedRail";
import { SidebarProjectGroup } from "./SidebarProjectGroup";
import { SidebarSessionRow } from "./SidebarSessionRow";
import { SidebarTrashJournalDrawer } from "./SidebarTrashJournalDrawer";
import type { ThemeMode } from "./WorkbenchTopNav";

export type { ChatSession };

export interface MainSidebarProps {
  projectName: string;
  projectId?: string;
  projects: WorkbenchProject[];
  trashedProjects?: ProjectDto[];
  restoringProjectId?: string | null;
  sessions: ChatSession[];
  activeSessionId: string;
  nodes?: Array<{ id: string; screenKey: string; title: string }>;
  activeSubSessionKey?: string | null;
  onSelectSession: (id: string) => void;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
  onOpenNewConversationModal: () => void;
  onOpenSettings: (tab?: "general" | "models" | "mcp" | "automations") => void;
  currentLocale: string;
  themeMode: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  onToggleProjectFolder?: (projectId: string) => void;
  onCreateSessionInProject?: (projectId: string) => void;
  onTogglePinSession?: (sessionId: string) => void;
  onRenameSession?: (sessionId: string, newTitle: string) => void;
  onDeleteSession?: (sessionId: string) => void;
  onRenameProject?: (projectId: string, newName: string) => void;
  onDeleteProject?: (projectId: string) => void;
  onRestoreProject?: (projectId: string) => void;
  onLinkProjectFolder?: (projectId: string) => void;
}

const SidebarTopActions: React.FC<{
  onOpenNewConversationModal: () => void;
  onToggleCollapse?: () => void;
  onOpenSettings: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({ onOpenNewConversationModal, onToggleCollapse, onOpenSettings }) => {
  const { t } = useTranslation();
  const collapseLabel = t("workbench.collapseSidebar", "Thu gọn thanh bên");
  return (
    <>
      <div style={{ padding: "8px 10px", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", gap: "6px", backgroundColor: "var(--bg-workspace)" }}>
        <button type="button" onClick={onOpenNewConversationModal} className="btn-new-conversation cowok-btn" style={{ flex: 1, height: "28px", display: "flex", alignItems: "center", justifyContent: "flex-start", padding: "0 10px", gap: "7px", borderRadius: "var(--radius-sm)", fontSize: "12px", fontWeight: 600, cursor: "pointer", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)" }}>
          <BrandIcon name="plus" size={13} color="var(--action-primary)" />
          <span>{t("workbench.newConversation", "New Conversation")}</span>
        </button>
        {onToggleCollapse && (
          <button type="button" onClick={onToggleCollapse} title={collapseLabel} aria-label={collapseLabel} className="cowok-btn cowok-btn-ghost" style={{ width: "28px", height: "28px", minWidth: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)" }}>
            <BrandIcon name="panel-left-close" size={14} color="currentColor" />
          </button>
        )}
      </div>
      <div style={{ padding: "4px 8px 6px 8px", borderBottom: "1px solid var(--border-subtle)" }}>
        <div role="button" onClick={() => onOpenSettings("automations")} title={t("settings.tabAutomations", "Tự động hóa")} style={{ display: "flex", alignItems: "center", gap: "8px", padding: "5px 8px", fontSize: "12px", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
          <BrandIcon name="clock" size={13} color="var(--text-muted)" />
          <span>{t("workbench.automations", "Automations")}</span>
        </div>
        <div role="button" onClick={() => onOpenSettings("mcp")} title={t("settings.tabMcp", "MCP & Tiện ích")} style={{ display: "flex", alignItems: "center", gap: "8px", padding: "5px 8px", fontSize: "12px", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
          <BrandIcon name="customizations" size={13} color="var(--text-muted)" />
          <span>{t("workbench.customizations", "Customizations")}</span>
        </div>
      </div>
    </>
  );
};

const SidebarStandaloneGroup: React.FC<{
  sessions: ChatSession[];
  activeSessionId: string;
  nodes?: Array<{ id: string; screenKey: string; title: string }>;
  activeSubSessionKey?: string | null;
  onSelectSession: (id: string) => void;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
  onOpenNewConversationModal: () => void;
  onTogglePinSession?: (id: string) => void;
  onRenameSession?: (id: string, title: string) => void;
  onDeleteSession?: (id: string) => void;
}> = (p) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const newLabel = t("workbench.newStandaloneConvo", "Tạo hội thoại độc lập mới");
  return (
    <div>
      <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", padding: "4px 8px", display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer", userSelect: "none" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "5px" }} onClick={() => setOpen((v) => !v)}>
          <span>{t("workbench.conversations", "Conversations")}</span>
          <BrandIcon name={open ? "chevron-down" : "chevron-right"} size={10} color="currentColor" />
        </div>
        <button type="button" onClick={(e) => { e.stopPropagation(); p.onOpenNewConversationModal(); }} title={newLabel} aria-label={newLabel} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "2px", color: "var(--text-muted)" }}>
          <BrandIcon name="plus" size={12} color="currentColor" />
        </button>
      </div>
      {open && (
        <div style={{ marginTop: "2px", paddingLeft: "4px" }}>
          {p.sessions.length === 0 ? (
            <div style={{ fontSize: "11px", color: "var(--text-muted)", padding: "4px 8px", fontStyle: "italic" }}>{t("workbench.noStandaloneConversations", "Chưa có hội thoại độc lập")}</div>
          ) : (
            p.sessions.map((s) => (
              <SidebarSessionRow key={`standalone-${s.id}`} session={s} keyPrefix="standalone" activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession} onTogglePinSession={p.onTogglePinSession} onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession} />
            ))
          )}
        </div>
      )}
    </div>
  );
};

const SidebarFooter: React.FC<{
  currentLocale: string;
  onOpenSettings: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({ currentLocale, onOpenSettings }) => {
  const { t } = useTranslation();
  const settingsLabel = t("workbench.settings", "Settings");
  return (
    <div data-testid="sidebar-seamless-footer" style={{ padding: "8px 12px", borderTop: "none", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div style={{ fontSize: "11px", color: "var(--text-muted)", display: "flex", alignItems: "center", gap: "5px", fontFamily: "var(--font-mono)" }}>
        <BrandIcon name="globe" size={12} color="var(--text-muted)" />
        <span>{currentLocale}</span>
      </div>
      <button type="button" onClick={() => onOpenSettings("general")} title={settingsLabel} aria-label={settingsLabel} className="cowok-btn cowok-btn-ghost" style={{ display: "flex", alignItems: "center", gap: "5px", fontSize: "11.5px", color: "var(--text-secondary)", padding: "3px 8px", height: "24px", borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "transparent" }}>
        <BrandIcon name="settings" size={12} color="currentColor" />
        <span style={{ fontWeight: 500 }}>{settingsLabel}</span>
      </button>
    </div>
  );
};

export const MainSidebar: React.FC<MainSidebarProps> = (p) => {
  const { t } = useTranslation();
  const [isPinnedOpen, setIsPinnedOpen] = useState(true);

  if (p.isCollapsed) {
    return <SidebarCollapsedRail onToggleCollapse={p.onToggleCollapse} onOpenNewConversationModal={p.onOpenNewConversationModal} onOpenSettings={p.onOpenSettings} />;
  }

  const pinned = p.sessions.filter((s) => s.isPinned);
  const standalone = p.sessions.filter((s) => !s.projectId || s.projectId === "general" || s.projectId === "standalone");

  return (
    <aside className="workbench-main-sidebar" aria-label={t("workbench.sidebarAria", "Cây thư mục dự án và phiên hội thoại")} style={{ width: "240px", height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-workspace)", borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, fontFamily: "var(--font-sans)", position: "relative" }}>
      <SidebarTopActions onOpenNewConversationModal={p.onOpenNewConversationModal} onToggleCollapse={p.onToggleCollapse} onOpenSettings={p.onOpenSettings} />
      <div style={{ flex: 1, overflowY: "auto", padding: "8px 6px" }}>
        {pinned.length > 0 && (
          <div style={{ marginBottom: "12px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", padding: "3px 8px", display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer", userSelect: "none" }} onClick={() => setIsPinnedOpen((v) => !v)}>
              <span>{t("workbench.pinnedConversations", "Pinned Conversations")}</span>
            </div>
            {isPinnedOpen && (
              <div style={{ marginTop: "2px" }}>
                {pinned.map((s) => (
                  <SidebarSessionRow key={`pinned-${s.id}`} session={s} keyPrefix="pinned" activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession} onTogglePinSession={p.onTogglePinSession} onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession} />
                ))}
              </div>
            )}
          </div>
        )}
        <SidebarProjectGroup
          projectId={p.projectId || DEFAULT_FLOW_PROJECT_ID} projectName={p.projectName} projects={p.projects} sessions={p.sessions}
          activeSessionId={p.activeSessionId} nodes={p.nodes} activeSubSessionKey={p.activeSubSessionKey}
          onSelectSession={p.onSelectSession} onSelectNodeSubSession={p.onSelectNodeSubSession}
          onOpenNewConversationModal={p.onOpenNewConversationModal} onToggleProjectFolder={p.onToggleProjectFolder}
          onCreateSessionInProject={p.onCreateSessionInProject} onTogglePinSession={p.onTogglePinSession}
          onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession} onRenameProject={p.onRenameProject}
          onDeleteProject={p.onDeleteProject} onLinkProjectFolder={p.onLinkProjectFolder}
        />
        <SidebarStandaloneGroup sessions={standalone} activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession} onOpenNewConversationModal={p.onOpenNewConversationModal} onTogglePinSession={p.onTogglePinSession} onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession} />
        <SidebarTrashJournalDrawer trashedProjects={p.trashedProjects} restoringProjectId={p.restoringProjectId} onRestoreProject={p.onRestoreProject} />
      </div>
      <SidebarFooter currentLocale={p.currentLocale} onOpenSettings={p.onOpenSettings} />
    </aside>
  );
};
