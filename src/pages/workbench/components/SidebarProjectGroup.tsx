// @story 04.1
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { WorkbenchProject } from "../constants";
import type { ChatSession } from "../types";
import { SidebarSessionRow } from "./SidebarSessionRow";

interface SidebarProjectGroupProps {
  projectId: string;
  projectName: string;
  projects: WorkbenchProject[];
  sessions: ChatSession[];
  activeSessionId: string;
  nodes?: Array<{ id: string; screenKey: string; title: string }>;
  activeSubSessionKey?: string | null;
  onSelectSession: (id: string) => void;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
  onOpenNewConversationModal: () => void;
  onToggleProjectFolder?: (projectId: string) => void;
  onCreateSessionInProject?: (projectId: string) => void;
  onTogglePinSession?: (sessionId: string) => void;
  onRenameSession?: (sessionId: string, newTitle: string) => void;
  onDeleteSession?: (sessionId: string) => void;
  onRenameProject?: (projectId: string, newName: string) => void;
  onDeleteProject?: (projectId: string) => void;
  onLinkProjectFolder?: (projectId: string) => void;
}

const MINI_BTN_STYLE: React.CSSProperties = {
  width: "18px", height: "18px", border: "none", background: "transparent", cursor: "pointer",
  display: "inline-flex", alignItems: "center", justifyContent: "center", padding: 0,
};

const ProjectRowHoverActions: React.FC<{
  proj: WorkbenchProject;
  canDelete: boolean;
  onStartEdit: () => void;
  onOpenNewConversationModal: () => void;
  onCreateSessionInProject?: (id: string) => void;
  onLinkProjectFolder?: (id: string) => void;
  onDeleteProject?: (id: string) => void;
}> = (p) => {
  const { t } = useTranslation();
  const newLabel = t("workbench.newSessionInProject", "Tạo phiên hội thoại mới trong dự án này");
  const renameLabel = t("workbench.renameItem", "Đổi tên");
  const linkLabel = t("workbench.linkLocalFolder", "Liên kết thư mục Git cục bộ");
  const delLabel = t("workbench.deleteItem", "Xóa");
  return (
    <>
      <button type="button" onClick={() => (p.onCreateSessionInProject ? p.onCreateSessionInProject(p.proj.id) : p.onOpenNewConversationModal())} title={newLabel} aria-label={newLabel} style={{ ...MINI_BTN_STYLE, color: "var(--action-primary)" }}>
        <BrandIcon name="plus" size={11} color="currentColor" />
      </button>
      <button type="button" onClick={p.onStartEdit} title={renameLabel} aria-label={renameLabel} style={{ ...MINI_BTN_STYLE, color: "var(--text-muted)" }}>
        <BrandIcon name="edit" size={11} color="currentColor" />
      </button>
      {p.proj.provenance === "scratch" && p.onLinkProjectFolder && (
        <button type="button" onClick={() => p.onLinkProjectFolder?.(p.proj.id)} title={linkLabel} aria-label={linkLabel} style={{ ...MINI_BTN_STYLE, color: "var(--agent-accent)" }}>
          <BrandIcon name="git-branch" size={11} color="currentColor" />
        </button>
      )}
      {p.canDelete && p.onDeleteProject && (
        <button type="button" onClick={() => p.onDeleteProject?.(p.proj.id)} title={delLabel} aria-label={delLabel} style={{ ...MINI_BTN_STYLE, color: "var(--state-danger)" }}>
          <BrandIcon name="trash" size={11} color="currentColor" />
        </button>
      )}
    </>
  );
};

const SingleProjectFolderBlock: React.FC<{
  proj: WorkbenchProject;
  isCurrentActiveProj: boolean;
  canDelete: boolean;
  sessions: ChatSession[];
  activeSessionId: string;
  nodes?: Array<{ id: string; screenKey: string; title: string }>;
  activeSubSessionKey?: string | null;
  onSelectSession: (id: string) => void;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
  onOpenNewConversationModal: () => void;
  onToggleProjectFolder?: (id: string) => void;
  onCreateSessionInProject?: (id: string) => void;
  onTogglePinSession?: (id: string) => void;
  onRenameSession?: (id: string, title: string) => void;
  onDeleteSession?: (id: string) => void;
  onRenameProject?: (id: string, name: string) => void;
  onDeleteProject?: (id: string) => void;
  onLinkProjectFolder?: (id: string) => void;
}> = (p) => {
  const [hovered, setHovered] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draftName, setDraftName] = useState(p.proj.name);
  const isExpanded = p.proj.isExpanded !== false;
  const commit = () => { p.onRenameProject?.(p.proj.id, draftName); setEditing(false); };

  return (
    <div style={{ marginBottom: "4px" }}>
      <div role="button" onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)} onClick={() => p.onToggleProjectFolder?.(p.proj.id)} style={{ padding: "5px 8px", borderRadius: "var(--radius-xs)", fontSize: "12px", color: p.isCurrentActiveProj ? "var(--text-primary)" : "var(--text-secondary)", backgroundColor: p.isCurrentActiveProj || hovered ? "var(--bg-subtle)" : "transparent", fontWeight: p.isCurrentActiveProj ? 600 : 500, cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "4px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px", overflow: "hidden", flex: 1 }}>
          <BrandIcon name="folder" size={12} color={p.isCurrentActiveProj ? "var(--action-primary)" : "currentColor"} />
          {editing ? (
            <input type="text" value={draftName} autoFocus onClick={(e) => e.stopPropagation()} onChange={(e) => setDraftName(e.target.value)} onBlur={commit} onKeyDown={(e) => { if (e.key === "Enter") commit(); else if (e.key === "Escape") setEditing(false); }} style={{ width: "100%", height: "20px", fontSize: "11.5px", padding: "0 4px", borderRadius: "3px", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }} />
          ) : (
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.proj.name}</span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "3px", flexShrink: 0 }} onClick={(e) => e.stopPropagation()}>
          {hovered ? (
            <ProjectRowHoverActions proj={p.proj} canDelete={p.canDelete} onStartEdit={() => { setDraftName(p.proj.name); setEditing(true); }} onOpenNewConversationModal={p.onOpenNewConversationModal} onCreateSessionInProject={p.onCreateSessionInProject} onLinkProjectFolder={p.onLinkProjectFolder} onDeleteProject={p.onDeleteProject} />
          ) : (
            <span style={{ fontSize: "9.5px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", marginRight: "2px" }}>{p.proj.provenance === "scratch" ? "scratch" : "git"}</span>
          )}
          <BrandIcon name={isExpanded ? "chevron-down" : "chevron-right"} size={10} color="var(--text-muted)" />
        </div>
      </div>
      {isExpanded && (
        <div style={{ paddingLeft: "10px", marginLeft: "10px", marginTop: "2px", borderLeft: "1px solid var(--border-subtle)" }}>
          {p.sessions.map((s) => (
            <SidebarSessionRow
              key={`proj-${s.id}`}
              session={s}
              keyPrefix="proj"
              projectId={p.proj.id}
              nodes={p.nodes}
              activeSubSessionKey={p.activeSubSessionKey}
              activeSessionId={p.activeSessionId}
              onSelectSession={p.onSelectSession}
              onSelectNodeSubSession={p.onSelectNodeSubSession}
              onTogglePinSession={p.onTogglePinSession}
              onRenameSession={p.onRenameSession}
              onDeleteSession={p.onDeleteSession}
            />
          ))}
        </div>
      )}
    </div>
  );
};

const ProjectFilterPopover: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ position: "absolute", top: "26px", right: "6px", width: "200px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-strong)", borderRadius: "var(--radius-sm)", boxShadow: "var(--shadow-md)", padding: "6px", zIndex: 40, fontSize: "11.5px", color: "var(--text-primary)" }} onClick={(e) => e.stopPropagation()}>
      <div style={{ padding: "5px 8px", display: "flex", justifyContent: "space-between" }}>
        <span>{t("workbench.groupByLabel", "Group By")}</span>
        <span style={{ color: "var(--text-muted)" }}>{t("workbench.groupByProject", "Project ›")}</span>
      </div>
      <div style={{ padding: "5px 8px", display: "flex", justifyContent: "space-between" }}>
        <span>{t("workbench.sortConversationsLabel", "Sort Conversations")}</span>
        <span style={{ color: "var(--text-muted)" }}>{t("workbench.sortLastUpdated", "Last Updated ›")}</span>
      </div>
      <div style={{ height: "1px", backgroundColor: "var(--border-subtle)", margin: "4px 0" }} />
      <div role="button" onClick={onClose} style={{ padding: "5px 8px", cursor: "pointer", color: "var(--action-primary)", fontWeight: 500 }}>
        {t("workbench.markAllRead", "Mark All as Read")}
      </div>
    </div>
  );
};

export const SidebarProjectGroup: React.FC<SidebarProjectGroupProps> = (p) => {
  const { t } = useTranslation();
  const [isGroupOpen, setIsGroupOpen] = useState(true);
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const filterLabel = t("workbench.filterProjects", "Lọc và sắp xếp dự án");
  const newProjLabel = t("workbench.newProjectOrLink", "Tạo dự án mới hoặc Liên kết thư mục Git");

  return (
    <div style={{ marginBottom: "12px", position: "relative" }}>
      <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", padding: "4px 8px", display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer", userSelect: "none" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "5px" }} onClick={() => setIsGroupOpen((v) => !v)}>
          <span>{t("workbench.projects", "Projects")}</span>
          <BrandIcon name={isGroupOpen ? "chevron-down" : "chevron-right"} size={10} color="currentColor" />
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <button type="button" onClick={(e) => { e.stopPropagation(); setIsFilterOpen((v) => !v); }} title={filterLabel} aria-label={filterLabel} className="cowok-btn-ghost" style={{ border: "none", background: isFilterOpen ? "var(--bg-subtle)" : "transparent", cursor: "pointer", padding: "3px", borderRadius: "4px", color: "var(--text-muted)" }}>
            <BrandIcon name="filter" size={11} color="currentColor" />
          </button>
          <button type="button" onClick={(e) => { e.stopPropagation(); p.onOpenNewConversationModal(); }} title={newProjLabel} aria-label={newProjLabel} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "3px", borderRadius: "4px", color: "var(--text-muted)" }}>
            <BrandIcon name="folder-plus" size={12} color="currentColor" />
          </button>
        </div>
      </div>
      {isFilterOpen && <ProjectFilterPopover onClose={() => setIsFilterOpen(false)} />}
      {isGroupOpen && (
        <div style={{ marginTop: "2px" }}>
          {p.projects.map((proj) => (
            <SingleProjectFolderBlock
              key={proj.id} proj={proj} isCurrentActiveProj={proj.id === p.projectId || proj.name === p.projectName}
              canDelete={p.projects.length > 1} sessions={p.sessions.filter((s) => s.projectId === proj.id)}
              activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession}
              nodes={p.nodes} activeSubSessionKey={p.activeSubSessionKey} onSelectNodeSubSession={p.onSelectNodeSubSession}
              onOpenNewConversationModal={p.onOpenNewConversationModal} onToggleProjectFolder={p.onToggleProjectFolder}
              onCreateSessionInProject={p.onCreateSessionInProject} onTogglePinSession={p.onTogglePinSession}
              onRenameSession={p.onRenameSession} onDeleteSession={p.onDeleteSession}
              onRenameProject={p.onRenameProject} onDeleteProject={p.onDeleteProject} onLinkProjectFolder={p.onLinkProjectFolder}
            />
          ))}
        </div>
      )}
    </div>
  );
};
