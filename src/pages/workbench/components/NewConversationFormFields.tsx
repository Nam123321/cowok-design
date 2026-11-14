// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-04
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { ProjectProvenance, WorkbenchProject } from "../constants";
import { MANAGED_SLUG_REGEX } from "../hooks/useProjectTrashAndBrowse";

export interface NewConversationFormFieldsProps {
  mode: "project" | "new-project" | "standalone";
  projects: WorkbenchProject[];
  activeProjectId?: string;
  selectedProjectId: string;
  onSelectProjectId: (id: string) => void;
  newProjectName: string;
  onChangeNewProjectName: (val: string) => void;
  managedSlug: string;
  onChangeManagedSlug: (val: string) => void;
  provenance: ProjectProvenance;
  onChangeProvenance: (val: ProjectProvenance) => void;
  localFolderPath: string;
  onChangeLocalFolderPath: (val: string) => void;
  sessionTitle: string;
  onChangeSessionTitle: (val: string) => void;
  onBrowseFolder: () => void;
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
            <BrandIcon name="folder-git" size={12} color="var(--human-accent)" />
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

const ManagedSlugField: React.FC<{
  managedSlug: string;
  onChangeManagedSlug: (val: string) => void;
}> = ({ managedSlug, onChangeManagedSlug }) => {
  const { t } = useTranslation();
  const isValid = MANAGED_SLUG_REGEX.test(managedSlug.trim());
  return (
    <div>
      <label htmlFor="newConvoManagedSlugInput" style={{ ...LABEL_STYLE, fontSize: "11px", marginBottom: "4px" }}>
        {t("workbench.managedSlugLabel", "Slug Workspace (~/CowokProjects/<slug>)")}
      </label>
      <input
        id="newConvoManagedSlugInput"
        type="text"
        value={managedSlug}
        onChange={(e) => onChangeManagedSlug(e.target.value)}
        className="cowok-input"
        style={{ ...INPUT_STYLE, height: "32px", fontFamily: "var(--font-mono)" }}
      />
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", marginTop: "4px" }}>
        <span style={{ fontSize: "10.5px", color: isValid ? "var(--text-muted)" : "var(--state-danger)" }}>
          {isValid
            ? t("workbench.managedSlugHint", "Chỉ gồm chữ thường a-z, số 0-9, '-' hoặc '_' (tối đa 63 ký tự)")
            : t("workbench.invalidSlugError", "Slug không hợp lệ. Phải khớp ^[a-z0-9][a-z0-9_-]{0,62}$")}
        </span>
        <span className="ff-metric-line">{`~/CowokProjects/${managedSlug.trim() || "..."}`}</span>
      </div>
    </div>
  );
};

const FolderPathPicker: React.FC<{
  localFolderPath: string;
  onChangeLocalFolderPath: (val: string) => void;
  onBrowseFolder: () => void;
}> = ({ localFolderPath, onChangeLocalFolderPath, onBrowseFolder }) => {
  const { t } = useTranslation();
  return (
    <div>
      <label style={{ ...LABEL_STYLE, fontSize: "11px", fontWeight: 500, color: "var(--text-muted)", marginBottom: "4px" }}>
        {t("workbench.localGitPathLabel", "Đường dẫn thư mục dự án cục bộ (Local Folder / Git Repo)")}
      </label>
      <div style={{ display: "flex", gap: "6px" }}>
        <input
          type="text"
          value={localFolderPath}
          placeholder={t("workbench.noLocalPathSelected", "Dùng mặc định 1-Click ~/CowokProjects/<slug> hoặc chọn qua Daemon FS")}
          onChange={(e) => onChangeLocalFolderPath(e.target.value)}
          className="cowok-input"
          style={{ ...INPUT_STYLE, flex: 1, height: "32px", fontSize: "11.5px", fontFamily: "var(--font-mono)" }}
        />
        <button
          type="button"
          onClick={onBrowseFolder}
          data-testid="open-daemon-fs-picker-btn"
          className="cowok-btn cowok-btn-secondary"
          style={{ display: "flex", alignItems: "center", gap: "6px", padding: "0 10px", height: "32px", fontSize: "11.5px", fontWeight: 500, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", whiteSpace: "nowrap", flexShrink: 0 }}
        >
          <BrandIcon name="folder-git" size={13} color="var(--human-accent)" />
          <span>{t("workbench.browseDaemonFsBtn", "Chọn qua Daemon FS")}</span>
        </button>
      </div>
    </div>
  );
};

function getTitlePlaceholder(
  mode: "project" | "new-project" | "standalone",
  t: (k: string, fallback: string) => string
): string {
  if (mode === "project") return t("workbench.sessionPlaceholderProject", "Ví dụ: Thiết kế Luồng Checkout & 3DS...");
  if (mode === "new-project") return t("workbench.sessionPlaceholderNewProject", "Ví dụ: Khởi tạo kiến trúc Userflow & Tokens...");
  return t("workbench.sessionPlaceholderStandalone", "Ví dụ: Khảo sát ý tưởng UI nhanh (Không gắn dự án)...");
}

export const NewConversationFormFields: React.FC<NewConversationFormFieldsProps> = (p) => {
  const { t } = useTranslation();
  const titlePlaceholder = getTitlePlaceholder(p.mode, t);

  return (
    <>
      {p.mode === "project" && (
        <div>
          <label style={LABEL_STYLE}>{t("workbench.selectProjectLabel", "Chọn Dự án (Chia sẻ DESIGN.md, tokens.css & đồ thị 2D DAG)")}</label>
          <select value={p.selectedProjectId} onChange={(e) => p.onSelectProjectId(e.target.value)} className="cowok-input" style={INPUT_STYLE}>
            {p.projects.map((proj) => (
              <option key={proj.id} value={proj.id}>
                {proj.name} ({proj.provenance === "scratch" ? t("workbench.provenanceScratch", "Scratch Managed") : t("workbench.provenanceGit", "Git Folder In-Place")}) {proj.id === p.activeProjectId ? `• ${t("workbench.currentlyOpen", "Đang mở")}` : ""}
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
          <ManagedSlugField managedSlug={p.managedSlug} onChangeManagedSlug={p.onChangeManagedSlug} />
          <ProvenanceSelector provenance={p.provenance} onChangeProvenance={p.onChangeProvenance} />
          {p.provenance === "folder-backed" && (
            <FolderPathPicker localFolderPath={p.localFolderPath} onChangeLocalFolderPath={p.onChangeLocalFolderPath} onBrowseFolder={p.onBrowseFolder} />
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
