// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-04
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { DEFAULT_FLOW_PROJECT_ID, ProjectProvenance, WorkbenchProject } from "../constants";
import { MANAGED_SLUG_REGEX, toManagedSlug } from "../hooks/useProjectTrashAndBrowse";
import { DaemonFolderPickerDialog } from "./DaemonFolderPickerDialog";
import { NewConversationFormFields } from "./NewConversationFormFields";

export interface NewConversationModalProps {
  isOpen: boolean;
  onClose: () => void;
  projects: WorkbenchProject[];
  activeProjectId?: string;
  onCreateInProject: (projectId: string, title: string) => void;
  onCreateNewProject: (
    projectName: string,
    initialTitle: string,
    provenance?: ProjectProvenance,
    location?: string | { localPath?: string; managedSlug?: string }
  ) => void;
  onCreateStandalone: (title: string) => void;
}

const buildTabBtnStyle = (isActive: boolean): React.CSSProperties => ({
  padding: "9px 8px", fontSize: "11.5px", fontWeight: isActive ? 600 : 500, borderRadius: 0, border: "none",
  borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
  color: isActive ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer",
  display: "flex", alignItems: "center", justifyContent: "center", gap: "5px", transition: "all var(--duration-fast)",
});

const ModalModeTabs: React.FC<{
  mode: "project" | "new-project" | "standalone";
  onSetMode: (mode: "project" | "new-project" | "standalone") => void;
}> = ({ mode, onSetMode }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", padding: "0 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-default)" }}>
      <button type="button" onClick={() => onSetMode("project")} style={buildTabBtnStyle(mode === "project")}>
        <BrandIcon name="folder" size={12} color="currentColor" />
        <span>{t("workbench.tabInProject", "Trong Dự Án")}</span>
      </button>
      <button id="tab-new-project-btn" type="button" onClick={() => onSetMode("new-project")} style={buildTabBtnStyle(mode === "new-project")}>
        <BrandIcon name="folder-plus" size={12} color="currentColor" />
        <span>{t("workbench.tabNewProject", "Dự Án Mới / Link Git")}</span>
      </button>
      <button type="button" onClick={() => onSetMode("standalone")} style={buildTabBtnStyle(mode === "standalone")}>
        <BrandIcon name="message-square" size={12} color="currentColor" />
        <span>{t("workbench.tabStandalone", "Hội Thoại Độc Lập")}</span>
      </button>
    </div>
  );
};

function useNewConvoDialogState(props: NewConversationModalProps) {
  const [mode, setMode] = useState<"project" | "new-project" | "standalone">("project");
  const [selectedProjectIdOverride, setSelectedProjectIdOverride] = useState<string | null>(null);
  const [sessionTitle, setSessionTitle] = useState("");
  const [newProjectName, setNewProjectName] = useState("");
  const [managedSlug, setManagedSlug] = useState("cowok-project");
  const [slugTouched, setSlugTouched] = useState(false);
  const [provenance, setProvenance] = useState<ProjectProvenance>("folder-backed");
  const [localFolderPath, setLocalFolderPath] = useState("");
  const [isDaemonPickerOpen, setIsDaemonPickerOpen] = useState(false);

  const selectedProjectId =
    (selectedProjectIdOverride && props.projects.some((p) => p.id === selectedProjectIdOverride)
      ? selectedProjectIdOverride
      : undefined) ||
    props.activeProjectId ||
    props.projects[0]?.id ||
    DEFAULT_FLOW_PROJECT_ID;

  useEffect(() => {
    if (!props.isOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape" && !isDaemonPickerOpen) props.onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isDaemonPickerOpen, props]);

  const handleProjectNameChange = (val: string) => {
    setNewProjectName(val);
    if (!slugTouched) setManagedSlug(toManagedSlug(val));
  };

  const handleManagedSlugChange = (val: string) => {
    setSlugTouched(true);
    setManagedSlug(val);
  };

  return {
    mode, setMode, selectedProjectId, setSelectedProjectId: setSelectedProjectIdOverride, sessionTitle, setSessionTitle,
    newProjectName, handleProjectNameChange, managedSlug, setManagedSlug, handleManagedSlugChange,
    setSlugTouched, provenance, setProvenance, localFolderPath, setLocalFolderPath,
    isDaemonPickerOpen, setIsDaemonPickerOpen,
  };
}

const ModalHeaderBar: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "13px 18px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="plus" size={15} color="currentColor" />
        <h3 id="new-convo-title" style={{ margin: 0, fontSize: "13.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("workbench.newConvoModalTitle", "Mở Phiên Hội Thoại & Không Gian Dự Án")}</h3>
      </div>
      <button type="button" onClick={onClose} aria-label={t("workbench.closeModal", "Đóng hộp thoại")} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "4px", color: "var(--text-muted)", fontSize: "16px", lineHeight: 1 }}>×</button>
    </div>
  );
};

const ModalFooterButtons: React.FC<{ isSlugValid: boolean; onClose: () => void }> = ({ isSlugValid, onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
      <button type="button" onClick={onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("workbench.cancelBtn", "Hủy")}</button>
      <button type="submit" disabled={!isSlugValid} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: 600, borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>{t("workbench.confirmStartBtn", "Bắt đầu Hội thoại")}</button>
    </div>
  );
};

function submitNewConversation(
  s: ReturnType<typeof useNewConvoDialogState>,
  props: NewConversationModalProps,
  t: ReturnType<typeof useTranslation>["t"],
): void {
  if (s.mode === "project") {
    props.onCreateInProject(s.selectedProjectId, s.sessionTitle.trim() || t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới"));
  } else if (s.mode === "new-project") {
    const projName = s.newProjectName.trim() || t("workbench.defaultNewProjectName", "Dự án mới");
    const trimmedPath = s.localFolderPath.trim();
    let locationArg: { localPath: string } | { managedSlug: string } | undefined;
    if (s.provenance !== "scratch") {
      locationArg = trimmedPath ? { localPath: trimmedPath } : { managedSlug: s.managedSlug.trim() };
    }
    props.onCreateNewProject(projName, s.sessionTitle.trim() || `${t("workbench.initProjectPrefix", "Khởi tạo")} ${projName}`, s.provenance, locationArg);
  } else {
    props.onCreateStandalone(s.sessionTitle.trim() || t("workbench.defaultStandaloneTitle", "Hội thoại độc lập"));
  }
  props.onClose();
}

export const NewConversationModal: React.FC<NewConversationModalProps> = (props) => {
  const { t } = useTranslation();
  const s = useNewConvoDialogState(props);
  if (!props.isOpen) return null;

  const isManagedRootMode = s.mode === "new-project" && s.provenance === "folder-backed" && !s.localFolderPath.trim();
  const isSlugValid = !isManagedRootMode || MANAGED_SLUG_REGEX.test(s.managedSlug.trim());
  const handleSubmit = (e: React.FormEvent) => { e.preventDefault(); if (isSlugValid) submitNewConversation(s, props, t); };

  return (
    <>
      <div role="dialog" aria-modal="true" aria-labelledby="new-convo-title" style={{ position: "fixed", inset: 0, backgroundColor: "var(--bg-overlay)", backdropFilter: "blur(4px)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 9999, padding: "16px" }} onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}>
        <div style={{ width: "100%", maxWidth: "520px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden", display: "flex", flexDirection: "column", fontFamily: "var(--font-sans)" }}>
          <ModalHeaderBar onClose={props.onClose} />
          <ModalModeTabs mode={s.mode} onSetMode={s.setMode} />
          <form onSubmit={handleSubmit} style={{ padding: "18px", display: "flex", flexDirection: "column", gap: "14px" }}>
            <NewConversationFormFields
              mode={s.mode} projects={props.projects} activeProjectId={props.activeProjectId}
              selectedProjectId={s.selectedProjectId} onSelectProjectId={s.setSelectedProjectId}
              newProjectName={s.newProjectName} onChangeNewProjectName={s.handleProjectNameChange}
              managedSlug={s.managedSlug} onChangeManagedSlug={s.handleManagedSlugChange}
              provenance={s.provenance} onChangeProvenance={s.setProvenance}
              localFolderPath={s.localFolderPath} onChangeLocalFolderPath={s.setLocalFolderPath}
              sessionTitle={s.sessionTitle} onChangeSessionTitle={s.setSessionTitle}
              onBrowseFolder={() => s.setIsDaemonPickerOpen(true)}
            />
            <ModalFooterButtons isSlugValid={isSlugValid} onClose={props.onClose} />
          </form>
        </div>
      </div>
      <DaemonFolderPickerDialog
        isOpen={s.isDaemonPickerOpen} mode="create" initialSlug={s.managedSlug} onClose={() => s.setIsDaemonPickerOpen(false)}
        onSelectForCreate={(selectedPath, slug) => { s.setManagedSlug(slug); s.setSlugTouched(true); if (selectedPath) s.setLocalFolderPath(selectedPath); }}
      />
    </>
  );
};
