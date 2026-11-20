// @story 04.1, 04.2, 06.1
// @implements AC-01, AC-02, AC-04, AC-06 (PA-A Dual-Intent + 3-Tab Form Fields)
import React from "react";
import { CowokSelect } from "../../../components/ui/CowokSelect";
import { useTranslation } from "../../../hooks/useTranslation";
import type { WorkbenchProject } from "../constants";
import { EditableAliasField } from "./EditableAliasField";
import { EntryKindOptionCards } from "./EntryKindOptionCards";
import {
  NewProjectSourceSection,
  type GitHubConnectionState,
  type ProjectSourceType,
} from "./NewProjectSourceSection";

export interface NewConversationFormFieldsProps {
  mode: "project" | "new-project" | "standalone";
  projects: WorkbenchProject[];
  activeProjectId?: string;
  selectedProjectId: string;
  onSelectProjectId: (id: string) => void;
  sourceType: ProjectSourceType | null;
  onChangeSourceType: (val: ProjectSourceType | null) => void;
  newProjectName: string;
  onChangeNewProjectName: (val: string) => void;
  managedSlug: string;
  localFolderPath: string;
  githubRepoUrl: string;
  onChangeGithubRepoUrl: (val: string) => void;
  githubState: GitHubConnectionState;
  onConnectGitHub: () => void;
  sessionTitle: string;
  onChangeSessionTitle: (val: string) => void;
  entryKind: "fe-dag" | "presentation";
  onChangeEntryKind: (val: "fe-dag" | "presentation") => void;
  onBrowseFolder: (preferredTab: "managed" | "browse") => void;
}

const SEMIBOLD_WEIGHT = 600;

const LABEL_STYLE: React.CSSProperties = {
  display: "block",
  fontSize: "11.5px",
  fontWeight: SEMIBOLD_WEIGHT,
  color: "var(--text-secondary)",
  marginBottom: "6px",
};

const CurrentProjectDropdownSection: React.FC<{
  projects: WorkbenchProject[];
  selectedProjectId: string;
  onSelectProjectId: (id: string) => void;
}> = ({ projects, selectedProjectId, onSelectProjectId }) => {
  const { t } = useTranslation();
  const fieldLabel = t("workbench.currentProjectLabel", "Dự án hiện tại (Chia sẻ chung DESIGN.md & tokens.css)");
  return (
    <div>
      <label htmlFor="newConvoProjectSelect" style={LABEL_STYLE}>
        {fieldLabel}
      </label>
      <CowokSelect
        id="newConvoProjectSelect"
        testId="project-selector-dropdown"
        ariaLabel={fieldLabel}
        value={selectedProjectId}
        onChange={onSelectProjectId}
        options={projects.map((proj) => ({ value: proj.id, label: proj.name }))}
      />
    </div>
  );
};

export const NewConversationFormFields: React.FC<NewConversationFormFieldsProps> = (p) => {
  const { t } = useTranslation();
  return (
    <>
      <EntryKindOptionCards
        selectedKind={p.entryKind}
        onSelect={p.onChangeEntryKind}
        testIdPrefix="new-convo-entry"
        label={t("workbench.entryKindLabel", "1. Chọn loại không gian làm việc")}
      />

      {p.mode === "project" && (
        <CurrentProjectDropdownSection
          projects={p.projects}
          selectedProjectId={p.selectedProjectId}
          onSelectProjectId={p.onSelectProjectId}
        />
      )}

      {p.mode === "new-project" && (
        <NewProjectSourceSection
          sourceType={p.sourceType}
          onChangeSourceType={p.onChangeSourceType}
          localFolderPath={p.localFolderPath}
          githubRepoUrl={p.githubRepoUrl}
          onChangeGithubRepoUrl={p.onChangeGithubRepoUrl}
          managedSlug={p.managedSlug}
          newProjectName={p.newProjectName}
          onChangeNewProjectName={p.onChangeNewProjectName}
          sessionTitle={p.sessionTitle}
          onChangeSessionTitle={p.onChangeSessionTitle}
          githubState={p.githubState}
          onConnectGitHub={p.onConnectGitHub}
          onBrowseFolder={p.onBrowseFolder}
        />
      )}

      {(p.mode === "project" || p.mode === "standalone") && (
        <EditableAliasField
          label={t("workbench.sessionAliasLabel", "Tên Session Chat (Mặc định: Tên dự án + STT)")}
          value={p.sessionTitle}
          inputId="newConvoSessionTitleInput"
          inputTestId="new-convo-session-title-input"
          editBtnTestId="rename-session-alias-btn"
          displayTestId="new-convo-session-alias-display"
          helperText={
            p.mode === "standalone"
              ? t("workbench.helperStandalone", "Phiên chat độc lập sẽ đặt trong nhóm Conversations và không gắn vào dự án nào.")
              : t("workbench.sessionAliasHint", "Tên alias dễ đọc trong dự án, có thể đổi bất cứ lúc nào.")
          }
          onChange={p.onChangeSessionTitle}
        />
      )}
    </>
  );
};
