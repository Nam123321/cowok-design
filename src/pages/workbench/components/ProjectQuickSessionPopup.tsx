// @story 04.1
// @story 06.1
// @implements AC-06 (Quick New Conversation in Project Intent Popup: App vs Presentation)
import React, { useEffect } from "react";
import { createPortal } from "react-dom";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { EntryKindOptionCards } from "./EntryKindOptionCards";

const POPUP_Z_INDEX = 10010;
const ICON_FOLDER_SIZE = 14;
const SEMIBOLD_WEIGHT = 600;

export interface ProjectQuickSessionPopupProps {
  project: { id: string; name: string } | null;
  onClose: () => void;
  onPickIntent: (projectId: string, entryKind: "fe-dag" | "presentation") => void;
}

const QuickSessionPopupHeader: React.FC<{ projectName: string; onClose: () => void }> = ({
  projectName,
  onClose,
}) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "12px 16px", borderBottom: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "7px", minWidth: 0 }}>
        <BrandIcon name="folder" size={ICON_FOLDER_SIZE} color="var(--action-primary)" />
        <span style={{ fontSize: "12.5px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {`${t("workbench.newSessionInProject", "New Conversation in Project")}: ${projectName}`}
        </span>
      </div>
      <button type="button" onClick={onClose} aria-label={t("workbench.closeModal", "Đóng hộp thoại")} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "4px", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
        <BrandIcon name="close" size={ICON_FOLDER_SIZE} color="currentColor" />
      </button>
    </div>
  );
};

export const ProjectQuickSessionPopup: React.FC<ProjectQuickSessionPopupProps> = ({
  project,
  onClose,
  onPickIntent,
}) => {
  const { t } = useTranslation();
  useEffect(() => {
    if (!project) return;
    const onKeyDown = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, project]);
  if (!project || typeof document === "undefined") return null;

  return createPortal(
    <div role="dialog" aria-modal="true" data-testid="project-quick-session-popup" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }} style={{ position: "fixed", inset: 0, zIndex: POPUP_Z_INDEX, backgroundColor: "var(--bg-overlay)", backdropFilter: "blur(3px)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px" }}>
      <div style={{ width: "100%", maxWidth: "460px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-strong)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)", overflow: "hidden", fontFamily: "var(--font-sans)" }}>
        <QuickSessionPopupHeader projectName={project.name} onClose={onClose} />
        <div style={{ padding: "16px", display: "flex", flexDirection: "column", gap: "10px" }}>
          <EntryKindOptionCards testIdPrefix="quick-popup-entry" label={t("workbench.quickAddChooseKind", "Chọn loại không gian làm việc cho phiên mới:")} onSelect={(kind) => { onPickIntent(project.id, kind); onClose(); }} />
        </div>
      </div>
    </div>,
    document.body
  );
};
