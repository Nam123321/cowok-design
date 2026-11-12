// @story 04.1, 04.3
// @implements AC-03 (MainSidebar 3-Level Sub-Sessions & Composite Scope Key)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ChatSession, NodeSubSessionItem } from "../types";

export interface SidebarSessionRowProps {
  session: ChatSession;
  keyPrefix: string;
  activeSessionId: string;
  projectId?: string;
  nodes?: Array<{ id: string; screenKey: string; title: string }>;
  activeSubSessionKey?: string | null;
  onSelectSession: (id: string) => void;
  onSelectNodeSubSession?: (
    compositeKey: string,
    projectId: string,
    sessionId: string,
    nodeId: string
  ) => void;
  onTogglePinSession?: (sessionId: string) => void;
  onRenameSession?: (sessionId: string, newTitle: string) => void;
  onDeleteSession?: (sessionId: string) => void;
}

const HOVER_ACTION_BTN_STYLE: React.CSSProperties = {
  width: "18px",
  height: "18px",
  border: "none",
  background: "transparent",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  padding: 0,
};

const SessionRowActions: React.FC<{
  session: ChatSession;
  onStartEdit: () => void;
  onTogglePinSession?: (id: string) => void;
  onDeleteSession?: (id: string) => void;
}> = ({ session, onStartEdit, onTogglePinSession, onDeleteSession }) => {
  const { t } = useTranslation();
  const pinLabel = t("workbench.pinSession", "Ghim / Bỏ ghim hội thoại");
  const renameLabel = t("workbench.renameItem", "Đổi tên");
  const delLabel = t("workbench.deleteItem", "Xóa");
  return (
    <div
      style={{ display: "flex", alignItems: "center", gap: "2px", flexShrink: 0 }}
      onClick={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        onClick={() => onTogglePinSession?.(session.id)}
        title={pinLabel}
        aria-label={pinLabel}
        style={{
          ...HOVER_ACTION_BTN_STYLE,
          color: session.isPinned ? "var(--action-primary)" : "var(--text-muted)",
        }}
      >
        <BrandIcon name="pin" size={11} color="currentColor" />
      </button>
      <button
        type="button"
        onClick={onStartEdit}
        title={renameLabel}
        aria-label={renameLabel}
        style={{ ...HOVER_ACTION_BTN_STYLE, color: "var(--text-muted)" }}
      >
        <BrandIcon name="edit" size={11} color="currentColor" />
      </button>
      <button
        type="button"
        onClick={() => onDeleteSession?.(session.id)}
        title={delLabel}
        aria-label={delLabel}
        style={{ ...HOVER_ACTION_BTN_STYLE, color: "var(--state-danger)" }}
      >
        <BrandIcon name="trash" size={11} color="currentColor" />
      </button>
    </div>
  );
};

export const SidebarSessionRow: React.FC<SidebarSessionRowProps> = ({
  session,
  keyPrefix,
  activeSessionId,
  projectId = "default-project",
  nodes = [],
  activeSubSessionKey,
  onSelectSession,
  onSelectNodeSubSession,
  onTogglePinSession,
  onRenameSession,
  onDeleteSession,
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isSubTreeOpen, setIsSubTreeOpen] = useState(true);
  const [editingTitle, setEditingTitle] = useState(session.title);

  const isActive = session.id === activeSessionId;
  const commitRename = () => {
    onRenameSession?.(session.id, editingTitle);
    setIsEditing(false);
  };

  // Derive sub-sessions from session.subSessions OR from the project's nodes
  const resolvedSubSessions: NodeSubSessionItem[] =
    session.subSessions && session.subSessions.length > 0
      ? session.subSessions
      : nodes.map((n) => ({
          nodeId: n.id,
          nodeTitle: n.title,
          screenKey: n.screenKey,
        }));

  const rowStyle: React.CSSProperties = {
    padding: "4px 8px",
    borderRadius: "var(--radius-xs)",
    fontSize: "12px",
    color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
    backgroundColor: isActive
      ? "var(--bg-selected)"
      : isHovered
      ? "var(--bg-subtle)"
      : "transparent",
    fontWeight: isActive ? 600 : 400,
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "6px",
    marginBottom: "1px",
    borderLeft: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
    transition: "all var(--duration-fast)",
  };

  return (
    <div key={`${keyPrefix}-${session.id}`} style={{ display: "flex", flexDirection: "column" }}>
      <div
        role="button"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        onClick={() => onSelectSession(session.id)}
        style={rowStyle}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "5px", overflow: "hidden", flex: 1 }}>
          {resolvedSubSessions.length > 0 && (
            <span
              onClick={(e) => {
                e.stopPropagation();
                setIsSubTreeOpen((prev) => !prev);
              }}
              style={{
                fontSize: "10px",
                color: "var(--text-muted)",
                display: "inline-flex",
                alignItems: "center",
                cursor: "pointer",
              }}
            >
              <BrandIcon name={isSubTreeOpen ? "chevron-down" : "chevron-right"} size={10} color="currentColor" />
            </span>
          )}
          {isEditing ? (
            <input
              type="text"
              value={editingTitle}
              autoFocus
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setEditingTitle(e.target.value)}
              onBlur={commitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitRename();
                else if (e.key === "Escape") setIsEditing(false);
              }}
              style={{
                width: "100%",
                height: "20px",
                fontSize: "11.5px",
                padding: "0 4px",
                borderRadius: "3px",
                border: "1px solid var(--action-primary)",
                backgroundColor: "var(--bg-surface)",
                color: "var(--text-primary)",
              }}
            />
          ) : (
            <span
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                flex: 1,
              }}
            >
              {session.title}
            </span>
          )}
        </div>

        {isHovered && !isEditing ? (
          <SessionRowActions
            session={session}
            onStartEdit={() => {
              setEditingTitle(session.title);
              setIsEditing(true);
            }}
            onTogglePinSession={onTogglePinSession}
            onDeleteSession={onDeleteSession}
          />
        ) : (
          <span
            style={{
              fontSize: "10px",
              color: "var(--text-muted)",
              fontFamily: "var(--font-mono)",
              flexShrink: 0,
            }}
          >
            {session.updatedAt}
          </span>
        )}
      </div>

      {/* Level 3: Node Sub-Sessions (Virtual Tree) */}
      {isActive && isSubTreeOpen && resolvedSubSessions.length > 0 && (
        <div
          className="sidebar-sub-sessions-tree"
          style={{
            paddingLeft: "14px",
            marginLeft: "10px",
            marginTop: "1px",
            marginBottom: "3px",
            borderLeft: "1px dashed var(--border-subtle)",
            display: "flex",
            flexDirection: "column",
            gap: "1px",
          }}
        >
          {resolvedSubSessions.map((sub) => {
            const compositeKey = `${projectId}::${session.id}::${sub.nodeId}`;
            const isSubActive = activeSubSessionKey === compositeKey;

            return (
              <div
                key={compositeKey}
                data-testid={`subsession-row-${sub.nodeId}`}
                title={`Composite Scope: ${compositeKey}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelectNodeSubSession?.(compositeKey, projectId, session.id, sub.nodeId);
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "2px 6px",
                  borderRadius: "var(--radius-xs)",
                  fontSize: "11px",
                  color: isSubActive ? "var(--text-primary)" : "var(--text-secondary)",
                  backgroundColor: isSubActive ? "var(--bg-selected)" : "transparent",
                  fontWeight: isSubActive ? 600 : 400,
                  cursor: "pointer",
                  transition: "background-color var(--duration-fast)",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "4px", overflow: "hidden" }}>
                  <span style={{ color: "var(--action-primary)", fontSize: "9px" }}>🎯</span>
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {sub.screenKey} · {sub.nodeTitle}
                  </span>
                </div>
                {sub.status === "running" && (
                  <span style={{ fontSize: "9px", color: "var(--action-primary)" }}>⚡</span>
                )}
                {sub.messageCount ? (
                  <span
                    style={{
                      fontSize: "9px",
                      color: "var(--text-muted)",
                      fontFamily: "var(--font-mono)",
                    }}
                  >
                    [{sub.messageCount}]
                  </span>
                ) : null}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
