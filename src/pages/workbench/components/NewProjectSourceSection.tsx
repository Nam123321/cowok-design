// @story 04.2, 05.3
// @story 06.1
// @implements AC-01, AC-04, AC-06 (Clean Progressive New Project Source & Alias Configuration)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { EditableAliasField } from "./EditableAliasField";

const ICON_SIZE_CARD = 14;
const ICON_SIZE_FOLDER_PATH = 13;
const SEMIBOLD_WEIGHT = 600;
const MEDIUM_WEIGHT = 500;

export type ProjectSourceType = "local" | "github";

export interface GitHubConnectionState {
  connected: boolean;
  activeLogin: string | null;
  connecting: boolean;
}

export interface NewProjectSourceSectionProps {
  sourceType: ProjectSourceType | null;
  onChangeSourceType: (next: ProjectSourceType | null) => void;
  localFolderPath: string;
  githubRepoUrl: string;
  onChangeGithubRepoUrl: (val: string) => void;
  managedSlug: string;
  newProjectName: string;
  onChangeNewProjectName: (val: string) => void;
  sessionTitle: string;
  onChangeSessionTitle: (val: string) => void;
  githubState: GitHubConnectionState;
  onConnectGitHub: () => void;
  onBrowseFolder: (preferredTab: "managed" | "browse") => void;
}

const GITHUB_INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  height: "32px",
  padding: "0 10px",
  fontSize: "11.5px",
  fontFamily: "var(--font-mono)",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  boxSizing: "border-box",
};

const sourceCardStyle = (active: boolean): React.CSSProperties => ({
  padding: "10px 12px",
  borderRadius: "var(--radius-sm)",
  textAlign: "left",
  cursor: "pointer",
  border: active ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: "var(--text-primary)",
  display: "flex",
  flexDirection: "column",
  gap: "4px",
});

const statusBadgeStyle = (active: boolean): React.CSSProperties => ({
  fontSize: "10px",
  fontWeight: SEMIBOLD_WEIGHT,
  fontFamily: "var(--font-mono)",
  padding: "1px 6px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: active ? "var(--state-success-soft)" : "var(--bg-surface)",
  color: active ? "var(--state-success)" : "var(--text-muted)",
  border: active ? "1px solid var(--state-success)" : "1px solid var(--border-default)",
});

const SourceChoiceCards: React.FC<{
  sourceType: ProjectSourceType | null;
  githubConnected: boolean;
  onSelectLocal: () => void;
  onSelectGithub: () => void;
}> = ({ sourceType, githubConnected, onSelectLocal, onSelectGithub }) => {
  const { t } = useTranslation();
  return (
    <div>
      <div style={{ fontSize: "11.5px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-secondary)", marginBottom: "6px" }}>
        {t("workbench.projectSourceLabel", "Nguồn Dự án")}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
        <button type="button" data-testid="source-local-folder-card" onClick={onSelectLocal} style={sourceCardStyle(sourceType === "local")}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT }}>
            <BrandIcon name="folder" size={ICON_SIZE_CARD} color="var(--human-accent)" />
            <span>{t("workbench.sourceLocalFolderTitle", "Mở thư mục")}</span>
          </span>
          <span style={{ fontSize: "10.5px", color: "var(--text-muted)", fontWeight: MEDIUM_WEIGHT }}>
            {t("workbench.sourceLocalFolderDesc", "Chọn thư mục trên máy (tự lấy làm tên dự án)")}
          </span>
        </button>

        <button type="button" data-testid="source-github-card" onClick={onSelectGithub} style={sourceCardStyle(sourceType === "github")}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT }}>
              <BrandIcon name="git-branch" size={ICON_SIZE_CARD} color="var(--action-primary)" />
              <span>{t("workbench.sourceGithubTitle", "Từ GitHub")}</span>
            </span>
            <span data-testid="github-status-badge" style={statusBadgeStyle(githubConnected)}>
              {githubConnected ? t("workbench.githubActive", "Hoạt động") : t("workbench.githubInactive", "Chưa kết nối")}
            </span>
          </div>
          <span style={{ fontSize: "10.5px", color: "var(--text-muted)", fontWeight: MEDIUM_WEIGHT }}>
            {t("workbench.sourceGithubDesc", "Kết nối hoặc nhập URL repository GitHub")}
          </span>
        </button>
      </div>
    </div>
  );
};

const SelectedFolderPill: React.FC<{
  localFolderPath: string;
  onChangeFolder: () => void;
}> = ({ localFolderPath, onChangeFolder }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "6px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-subtle)" }}>
      <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        <BrandIcon name="folder-git" size={ICON_SIZE_FOLDER_PATH} color="var(--human-accent)" />
        <span>{localFolderPath}</span>
      </span>
      <button type="button" onClick={onChangeFolder} style={{ border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", borderRadius: "var(--radius-xs)", padding: "2px 8px", fontSize: "10.5px", cursor: "pointer", flexShrink: 0 }}>
        {t("workbench.changeFolderBtn", "Đổi thư mục")}
      </button>
    </div>
  );
};

const GitHubSourcePanel: React.FC<{
  githubRepoUrl: string;
  githubState: GitHubConnectionState;
  onChangeGithubRepoUrl: (v: string) => void;
  onConnectGitHub: () => void;
}> = ({ githubRepoUrl, githubState, onChangeGithubRepoUrl, onConnectGitHub }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", fontSize: "11px", color: "var(--text-secondary)" }}>
        <span>
          {githubState.connected
            ? `${t("workbench.githubConnectedAs", "Đã kết nối GitHub")}: ${githubState.activeLogin ? `@${githubState.activeLogin}` : t("workbench.oauthCliFallback", "OAuth/CLI")}`
            : t("workbench.githubNotConnectedHint", "GitHub đang ở trạng thái Inactive. Kết nối nhanh qua CLI hoặc nhập URL công khai.")}
        </span>
        {!githubState.connected && (
          <button type="button" disabled={githubState.connecting} data-testid="connect-github-inline-btn" onClick={onConnectGitHub} className="cowok-btn cowok-btn-secondary" style={{ padding: "4px 10px", fontSize: "11px", fontWeight: SEMIBOLD_WEIGHT, borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", color: "var(--action-primary)", backgroundColor: "var(--bg-surface)", cursor: "pointer", whiteSpace: "nowrap" }}>
            {githubState.connecting ? t("workbench.githubConnecting", "Đang kết nối...") : t("workbench.githubConnectBtn", "Kết nối GitHub")}
          </button>
        )}
      </div>
      <input
        type="url"
        data-testid="github-repo-url-input"
        value={githubRepoUrl}
        onChange={(e) => onChangeGithubRepoUrl(e.target.value)}
        placeholder={t("workbench.githubUrlPlaceholder", "https://github.com/owner/repository")}
        className="cowok-input"
        style={GITHUB_INPUT_STYLE}
      />
    </div>
  );
};

const SourceConfigurationPanel: React.FC<{ p: NewProjectSourceSectionProps; hasReadyAlias: boolean }> = ({ p, hasReadyAlias }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px", padding: "12px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
      {p.sourceType === "local" ? (
        <SelectedFolderPill localFolderPath={p.localFolderPath || `~/CowokProjects/${p.managedSlug}`} onChangeFolder={() => p.onBrowseFolder("browse")} />
      ) : (
        <GitHubSourcePanel githubRepoUrl={p.githubRepoUrl} githubState={p.githubState} onChangeGithubRepoUrl={p.onChangeGithubRepoUrl} onConnectGitHub={p.onConnectGitHub} />
      )}
      {hasReadyAlias && (
        <>
          <EditableAliasField
            label={t("workbench.newProjectAliasLabel", "Tên Dự án (Tên hiển thị trên cowok-design)")}
            value={p.newProjectName}
            placeholder={t("workbench.newProjectPlaceholder", "Ví dụ: E-Commerce Checkout V2")}
            inputTestId="new-project-name-input"
            editBtnTestId="rename-project-alias-btn"
            displayTestId="new-project-alias-display"
            onChange={p.onChangeNewProjectName}
          />
          <EditableAliasField
            label={t("workbench.sessionAliasLabel", "Tên Session Chat (Mặc định: Tên dự án + STT)")}
            value={p.sessionTitle}
            inputId="newConvoSessionTitleInput"
            inputTestId="new-convo-session-title-input"
            editBtnTestId="rename-session-alias-btn"
            displayTestId="new-convo-session-alias-display"
            helperText={t("workbench.sessionAliasHint", "Tên alias dễ đọc trong dự án, có thể đổi bất cứ lúc nào.")}
            onChange={p.onChangeSessionTitle}
          />
        </>
      )}
    </div>
  );
};

export const NewProjectSourceSection: React.FC<NewProjectSourceSectionProps> = (p) => {
  const { t } = useTranslation();
  const hasReadyAlias =
    (p.sourceType === "local" && Boolean(p.localFolderPath.trim() || p.newProjectName.trim())) ||
    (p.sourceType === "github" && Boolean(p.githubRepoUrl.trim() || p.newProjectName.trim()));
  return (
    <>
      <SourceChoiceCards sourceType={p.sourceType} githubConnected={p.githubState.connected} onSelectLocal={() => p.onBrowseFolder("browse")} onSelectGithub={() => p.onChangeSourceType("github")} />
      <input id="newConvoManagedSlugInput" type="hidden" value={p.managedSlug} readOnly />
      {p.sourceType === null ? (
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", fontSize: "11px", color: "var(--text-muted)" }}>
          <span>{t("workbench.selectSourceHint", "Chọn Mở thư mục hoặc Từ GitHub để tự điền tên dự án.")}</span>
          <button type="button" data-testid="open-daemon-fs-picker-btn" onClick={() => p.onBrowseFolder("managed")} style={{ border: "none", background: "transparent", color: "var(--action-primary)", fontSize: "11px", cursor: "pointer", padding: 0, textDecoration: "underline", flexShrink: 0 }}>
            {t("workbench.managedRootQuickLink", "Hoặc tạo trong ~/CowokProjects")}
          </button>
        </div>
      ) : (
        <SourceConfigurationPanel p={p} hasReadyAlias={hasReadyAlias} />
      )}
    </>
  );
};
