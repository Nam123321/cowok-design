// @story 04.1
// @implements AC-01, AC-02
import React, { useState, useRef, useEffect } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { DEFAULT_FLOW_PROJECT_ID, ProjectProvenance, WorkbenchProject } from "../constants";
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
    localPath?: string
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

async function openDirectoryPickerOrFallback(
  folderInputRef: React.RefObject<HTMLInputElement | null>,
  setLocalFolderPath: (val: string) => void
): Promise<void> {
  try {
    if ("showDirectoryPicker" in window) {
      const dirHandle = await (window as unknown as { showDirectoryPicker: () => Promise<{ name?: string }> }).showDirectoryPicker();
      if (dirHandle?.name) setLocalFolderPath(`./projects/${dirHandle.name}`);
    } else {
      folderInputRef.current?.click();
    }
  } catch (err: unknown) {
    if ((err as { name?: string })?.name !== "AbortError") folderInputRef.current?.click();
  }
}

function useNewConvoDialogState(props: NewConversationModalProps) {
  const [mode, setMode] = useState<"project" | "new-project" | "standalone">("project");
  const [selectedProjectId, setSelectedProjectId] = useState<string>(props.activeProjectId || props.projects[0]?.id || DEFAULT_FLOW_PROJECT_ID);
  const [sessionTitle, setSessionTitle] = useState("");
  const [newProjectName, setNewProjectName] = useState("");
  const [provenance, setProvenance] = useState<ProjectProvenance>("folder-backed");
  const [localFolderPath, setLocalFolderPath] = useState("./projects/my-design-system");
  const folderInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!props.isOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") props.onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);

  useEffect(() => {
    if (props.activeProjectId) setSelectedProjectId(props.activeProjectId);
  }, [props.activeProjectId]);

  const handleFolderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const topDir = (file.webkitRelativePath || "").split("/")[0] || file.name;
    const absPath = (file as unknown as { path?: string }).path;
    setLocalFolderPath(absPath ? absPath.substring(0, absPath.lastIndexOf("/")) : `~/Projects/${topDir}`);
  };

  return {
    mode, setMode, selectedProjectId, setSelectedProjectId, sessionTitle, setSessionTitle,
    newProjectName, setNewProjectName, provenance, setProvenance, localFolderPath, setLocalFolderPath,
    folderInputRef, handleFolderChange,
  };
}

export const NewConversationModal: React.FC<NewConversationModalProps> = (props) => {
  const { t } = useTranslation();
  const s = useNewConvoDialogState(props);
  if (!props.isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (s.mode === "project") {
      props.onCreateInProject(s.selectedProjectId, s.sessionTitle.trim() || t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới"));
    } else if (s.mode === "new-project") {
      const projName = s.newProjectName.trim() || t("workbench.defaultNewProjectName", "Dự án mới");
      props.onCreateNewProject(projName, s.sessionTitle.trim() || `Khởi tạo ${projName}`, s.provenance, s.localFolderPath);
    } else {
      props.onCreateStandalone(s.sessionTitle.trim() || t("workbench.defaultStandaloneTitle", "Hội thoại độc lập"));
    }
    props.onClose();
  };

  return (
    <div role="dialog" aria-modal="true" aria-labelledby="new-convo-title" style={{ position: "fixed", inset: 0, backgroundColor: "rgba(8, 14, 26, 0.65)", backdropFilter: "blur(4px)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 9999, padding: "16px" }} onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}>
      <div style={{ width: "100%", maxWidth: "520px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden", display: "flex", flexDirection: "column", fontFamily: "var(--font-sans)" }}>
        <div style={{ padding: "13px 18px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="plus" size={15} color="currentColor" />
            <h3 id="new-convo-title" style={{ margin: 0, fontSize: "13.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("workbench.newConvoModalTitle", "Mở Phiên Hội Thoại & Không Gian Dự Án")}</h3>
          </div>
          <button type="button" onClick={props.onClose} aria-label={t("workbench.closeModal", "Đóng hộp thoại")} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", padding: "4px", color: "var(--text-muted)", fontSize: "16px", lineHeight: 1 }}>×</button>
        </div>
        <ModalModeTabs mode={s.mode} onSetMode={s.setMode} />
        <form onSubmit={handleSubmit} style={{ padding: "18px", display: "flex", flexDirection: "column", gap: "14px" }}>
          <NewConversationFormFields
            mode={s.mode} projects={props.projects} activeProjectId={props.activeProjectId}
            selectedProjectId={s.selectedProjectId} onSelectProjectId={s.setSelectedProjectId}
            newProjectName={s.newProjectName} onChangeNewProjectName={s.setNewProjectName}
            provenance={s.provenance} onChangeProvenance={s.setProvenance}
            localFolderPath={s.localFolderPath} onChangeLocalFolderPath={s.setLocalFolderPath}
            sessionTitle={s.sessionTitle} onChangeSessionTitle={s.setSessionTitle}
            folderInputRef={s.folderInputRef}
            onBrowseFolder={() => void openDirectoryPickerOrFallback(s.folderInputRef, s.setLocalFolderPath)}
            onFolderChange={s.handleFolderChange}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
            <button type="button" onClick={props.onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("workbench.cancelBtn", "Hủy")}</button>
            <button type="submit" className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: 600, borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>{t("workbench.confirmStartBtn", "Bắt đầu Hội thoại")}</button>
          </div>
        </form>
      </div>
    </div>
  );
};
