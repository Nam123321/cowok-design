// @story 04.1, 04.2
// @story 06.1
// @implements AC-01, AC-02, AC-04, AC-06 (PA-A Dual-Intent + 3-Tab Modal: Current Project / Create Project / Standalone)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { WorkbenchProject } from "../constants";
import { MANAGED_SLUG_REGEX, toManagedSlug } from "../hooks/useProjectTrashAndBrowse";
import { useNewConvoDialogState } from "../hooks/useNewConvoDialogState";
import type { ChatSession } from "../types";
import { extractFolderBasename } from "../utils/session-defaults";
import { DaemonFolderPickerDialog } from "./DaemonFolderPickerDialog";
import { NewConversationFormFields } from "./NewConversationFormFields";

export interface NewProjectSubmitOptions {
  openMaterialLaunchpad?: boolean;
  githubRepoUrl?: string;
}

export interface NewConversationModalProps {
  isOpen: boolean;
  onClose: () => void;
  projects: WorkbenchProject[];
  activeProjectId?: string;
  existingSessions?: ChatSession[];
  onCreateInProject: (projectId: string, title: string, entryKind?: "fe-dag" | "presentation") => void;
  onCreateNewProject: (
    projectName: string,
    initialTitle: string,
    provenance?: "folder-backed" | "scratch",
    location?: string | { localPath?: string; managedSlug?: string },
    entryKind?: "fe-dag" | "presentation",
    options?: NewProjectSubmitOptions
  ) => void;
  onCreateStandalone: (title: string, entryKind?: "fe-dag" | "presentation") => void;
}

const MODAL_Z_INDEX = 9999;
const TAB_ICON_SIZE = 12;
const HEADER_ICON_SIZE = 15;
const SEMIBOLD_WEIGHT = 600;
const MEDIUM_WEIGHT = 500;
const DISABLED_SUBMIT_OPACITY = 0.5;

const buildTabBtnStyle = (isActive: boolean): React.CSSProperties => ({
  padding: "9px 8px",
  fontSize: "11.5px",
  fontWeight: isActive ? SEMIBOLD_WEIGHT : MEDIUM_WEIGHT,
  borderRadius: 0,
  border: "none",
  borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
  color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "5px",
  transition: "all var(--duration-fast)",
});

const ModalModeTabs: React.FC<{
  mode: "project" | "new-project" | "standalone";
  onSetMode: (mode: "project" | "new-project" | "standalone") => void;
}> = ({ mode, onSetMode }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", padding: "0 10px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-default)" }}>
      <button type="button" data-testid="tab-current-project-btn" onClick={() => onSetMode("project")} style={buildTabBtnStyle(mode === "project")}>
        <BrandIcon name="folder" size={TAB_ICON_SIZE} color="currentColor" />
        <span>{t("workbench.tabCurrentProject", "Dự án hiện tại")}</span>
      </button>
      <button id="tab-new-project-btn" type="button" onClick={() => onSetMode("new-project")} style={buildTabBtnStyle(mode === "new-project")}>
        <BrandIcon name="folder-plus" size={TAB_ICON_SIZE} color="currentColor" />
        <span>{t("workbench.tabCreateProject", "Tạo dự án")}</span>
      </button>
      <button type="button" data-testid="tab-standalone-btn" onClick={() => onSetMode("standalone")} style={buildTabBtnStyle(mode === "standalone")}>
        <BrandIcon name="message-square" size={TAB_ICON_SIZE} color="currentColor" />
        <span>{t("workbench.tabStandaloneNoProject", "Chat không cần dự án")}</span>
      </button>
    </div>
  );
};

function submitNewConversation(
  s: ReturnType<typeof useNewConvoDialogState>,
  props: NewConversationModalProps,
  t: (k: string, f: string) => string
): void {
  const finalTitle = s.sessionTitle.trim() || s.defaultTitle;
  if (s.mode === "project") {
    props.onCreateInProject(s.selectedProjectId, finalTitle, s.entryKind);
  } else if (s.mode === "new-project") {
    const inferred = extractFolderBasename(s.sourceType === "github" ? s.githubRepoUrl : s.localFolderPath);
    const projName = s.newProjectName.trim() || inferred || t("workbench.defaultNewProjectName", "Dự án mới");
    const trimmedPath = s.localFolderPath.trim();
    const locationArg = s.sourceType === "local" && trimmedPath ? { localPath: trimmedPath } : { managedSlug: s.managedSlug.trim() || toManagedSlug(projName) };
    props.onCreateNewProject(projName, finalTitle, "folder-backed", locationArg, s.entryKind, {
      openMaterialLaunchpad: true,
      githubRepoUrl: s.sourceType === "github" ? s.githubRepoUrl.trim() || undefined : undefined,
    });
  } else {
    props.onCreateStandalone(finalTitle, s.entryKind);
  }
  props.onClose();
}

const NewConvoModalHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "13px 18px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="plus" size={HEADER_ICON_SIZE} color="currentColor" />
        <h3 id="new-convo-title" style={{ margin: 0, fontSize: "13.5px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-primary)" }}>{t("workbench.newConvoModalTitle", "Tạo Cuộc Trò Chuyện hoặc Dự Án Mới")}</h3>
      </div>
      <button type="button" onClick={onClose} aria-label={t("workbench.closeModal", "Đóng hộp thoại")} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "4px", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
        <BrandIcon name="close" size={HEADER_ICON_SIZE} color="currentColor" />
      </button>
    </div>
  );
};

const NewConvoModalFooter: React.FC<{
  canSubmit: boolean;
  submitLabel: string;
  onClose: () => void;
}> = ({ canSubmit, submitLabel, onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
      <button type="button" data-testid="new-convo-cancel-btn" onClick={onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("workbench.cancelBtn", "Hủy")}</button>
      <button type="submit" disabled={!canSubmit} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT, borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", opacity: canSubmit ? 1 : DISABLED_SUBMIT_OPACITY, cursor: canSubmit ? "pointer" : "not-allowed" }}>{submitLabel}</button>
    </div>
  );
};

function resolveSubmitState(s: ReturnType<typeof useNewConvoDialogState>) {
  if (s.mode !== "new-project") return true;
  if (s.sourceType === "local") return Boolean(s.localFolderPath.trim()) || MANAGED_SLUG_REGEX.test(s.managedSlug.trim());
  if (s.sourceType === "github") return Boolean(s.githubRepoUrl.trim() || s.newProjectName.trim());
  return false;
}

export const NewConversationModal: React.FC<NewConversationModalProps> = (props) => {
  const { t } = useTranslation();
  const s = useNewConvoDialogState(props);
  if (!props.isOpen) return null;

  const canSubmit = resolveSubmitState(s);
  let submitLabel = t("workbench.confirmStartAppBtn", "Mở Studio Ứng dụng");
  if (s.mode === "new-project") submitLabel = t("workbench.confirmCreateProjectBtn", "Tạo dự án & Chọn tài liệu");
  else if (s.entryKind === "presentation") submitLabel = t("workbench.confirmStartSlideBtn", "Mở Presentation 16:9");

  return (
    <>
      <div role="dialog" aria-modal="true" aria-labelledby="new-convo-title" style={{ position: "fixed", inset: 0, backgroundColor: "var(--bg-overlay)", backdropFilter: "blur(4px)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: MODAL_Z_INDEX, padding: "16px" }} onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}>
        <div style={{ width: "100%", maxWidth: "560px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden", display: "flex", flexDirection: "column", fontFamily: "var(--font-sans)" }}>
          <NewConvoModalHeader onClose={props.onClose} />
          <ModalModeTabs mode={s.mode} onSetMode={s.setMode} />
          <form onSubmit={(e) => { e.preventDefault(); if (canSubmit) submitNewConversation(s, props, t); }} style={{ padding: "18px", display: "flex", flexDirection: "column", gap: "14px" }}>
            <NewConversationFormFields
              mode={s.mode} projects={props.projects} activeProjectId={props.activeProjectId}
              selectedProjectId={s.selectedProjectId} onSelectProjectId={s.setSelectedProjectId}
              sourceType={s.sourceType} onChangeSourceType={s.setSourceType}
              newProjectName={s.newProjectName} onChangeNewProjectName={s.handleProjectNameChange}
              managedSlug={s.managedSlug} localFolderPath={s.localFolderPath}
              githubRepoUrl={s.githubRepoUrl} onChangeGithubRepoUrl={s.handleGithubRepoUrlChange}
              githubState={s.githubState} onConnectGitHub={() => void s.connectGitHub()}
              sessionTitle={s.sessionTitle} onChangeSessionTitle={s.handleSessionTitleChange}
              entryKind={s.entryKind} onChangeEntryKind={s.setEntryKind}
              onBrowseFolder={(tab) => { s.setPickerInitialTab(tab); s.setIsDaemonPickerOpen(true); }}
            />
            <NewConvoModalFooter canSubmit={canSubmit} submitLabel={submitLabel} onClose={props.onClose} />
          </form>
        </div>
      </div>
      <DaemonFolderPickerDialog
        isOpen={s.isDaemonPickerOpen} mode="create" initialSlug={s.managedSlug} initialTab={s.pickerInitialTab}
        initialPath={s.localFolderPath || undefined}
        onClose={() => s.setIsDaemonPickerOpen(false)}
        onSelectForCreate={s.handleFolderPicked}
      />
    </>
  );
};
