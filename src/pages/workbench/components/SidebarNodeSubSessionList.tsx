// @story 04.4
// @implements AC-04 (MainSidebar Level-3 V7 node_chat_sessions Sub-Tree)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { NodeSubSessionItem } from "../types";

export interface SidebarNodeSubSessionListProps {
  projectId: string;
  sessionId: string;
  items: NodeSubSessionItem[];
  activeSubSessionKey?: string | null;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
}

const STATUS_BADGE_MAP: Record<string, { labelKey: string; fallback: string; color: string }> = {
  running: { labelKey: "flow.runningBadge", fallback: "RUNNING", color: "var(--action-primary)" },
  paused: { labelKey: "flow.pausedBadge", fallback: "PAUSED", color: "var(--text-secondary)" },
  interrupted: { labelKey: "flow.interruptedBadge", fallback: "INTERRUPTED", color: "var(--state-danger)" },
  queued: { labelKey: "sidebar.subsessions.queued", fallback: "QUEUED", color: "var(--state-warning)" },
};

const SubSessionRowItem: React.FC<{
  sub: NodeSubSessionItem;
  projectId: string;
  sessionId: string;
  isSubActive: boolean;
  onSelectNodeSubSession?: (compositeKey: string, projectId: string, sessionId: string, nodeId: string) => void;
}> = ({ sub, projectId, sessionId, isSubActive, onSelectNodeSubSession }) => {
  const { t } = useTranslation();
  const compositeKey = sub.compositeScopeKey || `${projectId}::${sessionId}::${sub.nodeId}`;
  const badge = sub.status ? STATUS_BADGE_MAP[sub.status] : undefined;
  return (
    <div
      data-testid={`subsession-row-${sub.nodeId}`}
      title={`Composite Scope: ${compositeKey}`}
      onClick={(e) => { e.stopPropagation(); onSelectNodeSubSession?.(compositeKey, projectId, sessionId, sub.nodeId); }}
      style={{
        display: "flex", alignItems: "center", justifyContent: "space-between", gap: "4px", padding: "2px 6px",
        borderRadius: "var(--radius-xs)", fontSize: "11px", color: isSubActive ? "var(--text-primary)" : "var(--text-secondary)",
        backgroundColor: isSubActive ? "var(--bg-selected)" : "transparent", fontWeight: isSubActive ? 600 : 400, cursor: "pointer",
        transition: "background-color var(--duration-fast)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "5px", overflow: "hidden", minWidth: 0 }}>
        <BrandIcon name="crosshair" size={11} color="var(--action-primary)" style={{ flexShrink: 0 }} />
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{sub.screenKey} · {sub.nodeTitle}</span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "4px", flexShrink: 0 }}>
        {badge && <span style={{ fontSize: "8.5px", fontWeight: 700, fontFamily: "var(--font-mono)", color: badge.color }}>{t(badge.labelKey, badge.fallback)}</span>}
        {sub.messageCount ? <span style={{ fontSize: "9px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>[{sub.messageCount}]</span> : null}
      </div>
    </div>
  );
};

export const SidebarNodeSubSessionList: React.FC<SidebarNodeSubSessionListProps> = ({
  projectId, sessionId, items, activeSubSessionKey, onSelectNodeSubSession,
}) => {
  if (items.length === 0) return null;
  return (
    <div
      className="sidebar-sub-sessions-tree" data-testid={`sidebar-subsessions-${sessionId}`}
      style={{ paddingLeft: "14px", marginLeft: "10px", marginTop: "1px", marginBottom: "3px", borderLeft: "1px dashed var(--border-subtle)", display: "flex", flexDirection: "column", gap: "1px" }}
    >
      {items.map((sub) => {
        const compositeKey = sub.compositeScopeKey || `${projectId}::${sessionId}::${sub.nodeId}`;
        return (
          <SubSessionRowItem
            key={compositeKey} sub={sub} projectId={projectId} sessionId={sessionId}
            isSubActive={activeSubSessionKey === compositeKey} onSelectNodeSubSession={onSelectNodeSubSession}
          />
        );
      })}
    </div>
  );
};
