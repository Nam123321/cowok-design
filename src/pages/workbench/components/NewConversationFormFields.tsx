// @story 04.1
// @implements AC-01, AC-02
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { ProjectProvenance, WorkbenchProject } from "../constants";

export interface NewConversationFormFieldsProps {
  mode: "project" | "new-project" | "standalone";
  projects: WorkbenchProject[];
  activeProjectId?: string;
  selectedProjectId: string;
  onSelectProjectId: (id: string) => void;
  newProjectName: string;
  onChangeNewProjectName: (val: string) => void;
  provenance: ProjectProvenance;
  onChangeProvenance: (val: ProjectProvenance) => void;
  localFolderPath: string;
  onChangeLocalFolderPath: (val: string) => void;
  sessionTitle: string;
  onChangeSessionTitle: (val: string) => void;
  folderInputRef: React.RefObject<HTMLInputElement | null>;
  onBrowseFolder: () => void;
  onFolderChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

const INPUT_STYLE: React.CSSProperties = {
  width: "100%", height: "34px", padding: "0 10px", fontSize: "12px", borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", border: "1px solid var(--border-default)", boxSizing: "border-box",
};

const LABEL_STYLE: React.CSSProperties = {
  display: "block", fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)", marginBottom: "6px",
};

const cardOptionStyle = (active: boolean): React.CSSProperties => ({
  padding: "8px 10px", borderRadius: "var(--radius-xs)", textAlign: "left", cursor: "pointer",
  border: active ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: "var(--text-primary)",
});

const ProvenanceSelector: React.FC<{
  provenance: ProjectProvenance;
  onChangeProvenance: (val: ProjectProvenance) => void;
}> = ({ provenance, onChangeProvenance }) => {
  const { t } = useTranslation();
  return (
    <div>
      <label style={LABEL_STYLE}>{t("workbench.provenanceLabel", "Kiến trúc Lưu trữ Dự án (Hybrid Project Boundary — Section 9.3)")}</label>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
        <button type="button" onClick={() => onChangeProvenance("folder-backed")} style={cardOptionStyle(provenance === "folder-backed")}>
          <div style={{ fontSize: "11.5px", fontWeight: 600, marginBottom: "2px", display: "flex", alignItems: "center", gap: "5px" }}>
            <BrandIcon name="git-branch" size={12} color="var(--human-accent)" />
            <span>{t("workbench.folderBackedTitle", "Folder-Backed (In-Place)")}</span>
          </div>
          <div style={{ fontSize: "10px", color: "var(--text-muted)", lineHeight: "1.35" }}>
            {t("workbench.folderBackedDesc", 'Ghi trực tiếp vào thư mục Git cục bộ (writeback: "in-place")')}
          </div>
        </button>
        <button type="button" onClick={() => onChangeProvenance("scratch")} style={cardOptionStyle(provenance === "scratch")}>
          <div style={{ fontSize: "11.5px", fontWeight: 600, marginBottom: "2px", display: "flex", alignItems: "center", gap: "5px" }}>
            <BrandIcon name="sparkles" size={12} color="var(--agent-accent)" />
            <span>{t("workbench.managedScratchTitle", "Managed Scratch")}</span>
          </div>
          <div style={{ fontSize: "10px", color: "var(--text-muted)", lineHeight: "1.35" }}>
            {t("workbench.managedScratchDesc", "Khởi tạo nhanh tại ~/.cowok-design/projects/ (hỗ trợ Link Git sau)")}
          </div>
        </button>
      </div>
    </div>
  );
};

const FolderPathPicker: React.FC<{
  localFolderPath: string;
  onChangeLocalFolderPath: (val: string) => void;
  onBrowseFolder: () => void;
  onFolderChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  folderInputRef: React.RefObject<HTMLInputElement | null>;
}> = ({ localFolderPath, onChangeLocalFolderPath, onBrowseFolder, onFolderChange, folderInputRef }) => {
  const { t } = useTranslation();
  return (
    <div>
      <label style={{ ...LABEL_STYLE, fontSize: "11px", fontWeight: 500, color: "var(--text-muted)", marginBottom: "4px" }}>
        {t("workbench.localGitPathLabel", "Đường dẫn thư mục dự án cục bộ (Local Folder / Git Repo)")}
      </label>
      <div style={{ display: "flex", gap: "6px" }}>
        <input type="text" value={localFolderPath} onChange={(e) => onChangeLocalFolderPath(e.target.value)} className="cowok-input" style={{ ...INPUT_STYLE, flex: 1, height: "32px", fontSize: "11.5px", fontFamily: "var(--font-mono)" }} />
        <button type="button" onClick={onBrowseFolder} className="cowok-btn cowok-btn-secondary" style={{ display: "flex", alignItems: "center", gap: "6px", padding: "0 10px", height: "32px", fontSize: "11.5px", fontWeight: 500, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", whiteSpace: "nowrap", flexShrink: 0 }}>
          <BrandIcon name="folder" size={13} color="var(--human-accent)" />
          <span>{t("workbench.browseFolderBtn", "Chọn thư mục")}</span>
        </button>
        <input ref={folderInputRef} type="file" {...({ webkitdirectory: "", directory: "" } as Record<string, string>)} style={{ display: "none" }} onChange={onFolderChange} />
      </div>
    </div>
  );
};

export const NewConversationFormFields: React.FC<NewConversationFormFieldsProps> = (p) => {
  const { t } = useTranslation();
  const titlePlaceholder =
    p.mode === "project"
      ? t("workbench.sessionPlaceholderProject", "Ví dụ: Thiết kế Luồng Checkout & 3DS...")
      : p.mode === "new-project"
      ? t("workbench.sessionPlaceholderNewProject", "Ví dụ: Khởi tạo kiến trúc Userflow & Tokens...")
      : t("workbench.sessionPlaceholderStandalone", "Ví dụ: Khảo sát ý tưởng UI nhanh (Không gắn dự án)...");

  return (
    <>
      {p.mode === "project" && (
        <div>
          <label style={LABEL_STYLE}>{t("workbench.selectProjectLabel", "Chọn Dự án (Chia sẻ DESIGN.md, tokens.css & đồ thị 2D DAG)")}</label>
          <select value={p.selectedProjectId} onChange={(e) => p.onSelectProjectId(e.target.value)} className="cowok-input" style={INPUT_STYLE}>
            {p.projects.map((proj) => (
              <option key={proj.id} value={proj.id}>
                {proj.name} ({proj.provenance === "scratch" ? "Scratch Managed" : "Git Folder In-Place"}) {proj.id === p.activeProjectId ? "• Đang mở" : ""}
              </option>
            ))}
          </select>
        </div>
      )}
      {p.mode === "new-project" && (
        <>
          <div>
            <label style={LABEL_STYLE}>{t("workbench.newProjectNameLabel", "Tên Dự án Mới")}</label>
            <input type="text" value={p.newProjectName} onChange={(e) => p.onChangeNewProjectName(e.target.value)} placeholder={t("workbench.newProjectPlaceholder", "Ví dụ: E-Commerce Checkout V2")} required className="cowok-input" style={INPUT_STYLE} />
          </div>
          <ProvenanceSelector provenance={p.provenance} onChangeProvenance={p.onChangeProvenance} />
          {p.provenance === "folder-backed" && (
            <FolderPathPicker localFolderPath={p.localFolderPath} onChangeLocalFolderPath={p.onChangeLocalFolderPath} onBrowseFolder={p.onBrowseFolder} onFolderChange={p.onFolderChange} folderInputRef={p.folderInputRef} />
          )}
        </>
      )}
      <div>
        <label style={LABEL_STYLE}>{t("workbench.sessionTitleLabel", "Tiêu đề Phiên Hội thoại")}</label>
        <input type="text" value={p.sessionTitle} onChange={(e) => p.onChangeSessionTitle(e.target.value)} placeholder={titlePlaceholder} className="cowok-input" style={INPUT_STYLE} />
      </div>
    </>
  );
};
