// @story 04.2
// @implements AC-03
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectDto } from "../../../lib/validations/harness-daemon";
import {
  triggerRestoreFromSidebar,
  useTrashedProjectsSubscription,
} from "../hooks/useProjectTrashAndBrowse";

export interface SidebarTrashJournalDrawerProps {
  trashedProjects?: ProjectDto[];
  restoringProjectId?: string | null;
  onRestoreProject?: (projectId: string) => void;
}

const TRASH_ICON_SIZE = 12;
const SEMIBOLD_WEIGHT = 600;
const MEDIUM_WEIGHT = 500;

const ROW_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  padding: "5px 6px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-subtle)",
  border: "1px solid var(--border-subtle)",
  marginBottom: "4px",
};

const ROW_TITLE_STYLE: React.CSSProperties = {
  fontSize: "11.5px",
  fontWeight: MEDIUM_WEIGHT,
  color: "var(--text-primary)",
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

const RESTORE_BTN_STYLE: React.CSSProperties = {
  fontSize: "10.5px",
  fontWeight: SEMIBOLD_WEIGHT,
  padding: "3px 7px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--action-primary)",
  flexShrink: 0,
};

const TOGGLE_BTN_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "4px 8px",
  border: "none",
  background: "transparent",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  color: "var(--text-muted)",
  fontSize: "11px",
  fontWeight: SEMIBOLD_WEIGHT,
};

const TrashedProjectRow: React.FC<{
  project: ProjectDto;
  isRestoring: boolean;
  formattedDeletedAt: string;
  onRestore: (id: string) => void;
}> = ({ project, isRestoring, formattedDeletedAt, onRestore }) => {
  const { t } = useTranslation();
  return (
    <div data-testid={`trash-project-row-${project.id}`} style={ROW_STYLE}>
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={ROW_TITLE_STYLE}>{project.name}</div>
        <div style={{ display: "flex", alignItems: "center", gap: "4px", marginTop: "2px" }}>
          {project.project_code && <span className="ff-metric-line">{project.project_code}</span>}
          <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>{formattedDeletedAt}</span>
        </div>
      </div>
      <button
        type="button"
        disabled={isRestoring}
        onClick={() => onRestore(project.id)}
        data-testid={`restore-project-btn-${project.id}`}
        className="cowok-btn cowok-btn-secondary"
        style={{ ...RESTORE_BTN_STYLE, cursor: isRestoring ? "wait" : "pointer" }}
      >
        {isRestoring ? t("workbench.restoringProjectBtn", "Đang khôi phục...") : t("workbench.restoreProjectBtn", "Khôi phục")}
      </button>
    </div>
  );
};

export const SidebarTrashJournalDrawer: React.FC<SidebarTrashJournalDrawerProps> = (props) => {
  const { t, locale } = useTranslation();
  const { formatTime } = useFormatter({ locale });
  const subscribed = useTrashedProjectsSubscription();
  const [isOpen, setIsOpen] = useState(false);

  const items = props.trashedProjects ?? subscribed.items;
  const restoringId = props.restoringProjectId ?? subscribed.restoringId;
  const handleRestore = props.onRestoreProject ?? ((id: string) => void triggerRestoreFromSidebar(id));

  if (items.length === 0) return null;

  return (
    <div id="sidebarTrashJournal" data-testid="sidebar-trash-journal" style={{ marginTop: "10px", paddingTop: "8px", borderTop: "1px solid var(--border-subtle)" }}>
      <button type="button" onClick={() => setIsOpen((v) => !v)} data-testid="sidebar-trash-journal-toggle" style={TOGGLE_BTN_STYLE}>
        <span style={{ display: "flex", alignItems: "center", gap: "5px" }}>
          <BrandIcon name="trash" size={TRASH_ICON_SIZE} color="var(--text-muted)" />
          <span>{t("workbench.trashJournalTitle", "Thùng rác Dự án")}</span>
        </span>
        <span className="ff-metric-line">{items.length}</span>
      </button>
      {isOpen && (
        <div style={{ padding: "4px 6px 2px" }}>
          {items.map((proj) => (
            <TrashedProjectRow
              key={proj.id}
              project={proj}
              isRestoring={restoringId === proj.id}
              formattedDeletedAt={proj.trash_deleted_at ? formatTime(proj.trash_deleted_at) : ""}
              onRestore={handleRestore}
            />
          ))}
        </div>
      )}
    </div>
  );
};
