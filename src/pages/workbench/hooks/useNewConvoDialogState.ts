// @story 04.1, 04.2, 06.1
// @implements AC-01, AC-04, AC-06 (State Hook for 3-Tab PA-A NewConversationModal)
import { useCallback, useEffect, useMemo, useState } from "react";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import { useTranslation } from "../../../hooks/useTranslation";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";
import { toManagedSlug } from "./useProjectTrashAndBrowse";
import type { ChatSession } from "../types";
import { extractFolderBasename, generateUniqueSessionTitle } from "../utils/session-defaults";
import type { GitHubConnectionState, ProjectSourceType } from "../components/NewProjectSourceSection";
import type { NewConversationModalProps } from "../components/NewConversationModal";

function useGitHubStatusSync(isOpen: boolean, selectedProjectId: string) {
  const [githubState, setGithubState] = useState<GitHubConnectionState>({
    connected: false,
    activeLogin: null,
    connecting: false,
  });

  const refresh = useCallback(async () => {
    if (!selectedProjectId) return;
    try {
      const status = await apiVersionSnapshot.getIntegrationsStatus(selectedProjectId);
      setGithubState((prev) => ({
        ...prev,
        connected: Boolean(status.github?.connected),
        activeLogin: status.github?.activeLogin ?? null,
      }));
    } catch {
      setGithubState((prev) => ({ ...prev, connected: false }));
    }
  }, [selectedProjectId]);

  useEffect(() => {
    if (isOpen) void refresh();
  }, [isOpen, refresh]);

  const connectGitHub = useCallback(async () => {
    setGithubState((prev) => ({ ...prev, connecting: true }));
    try {
      await apiVersionSnapshot.connectGitHubAccount(selectedProjectId, { useGhCli: true });
      await refresh();
    } catch {
      // Keep status inactive when gh CLI auth is unavailable
    } finally {
      setGithubState((prev) => ({ ...prev, connecting: false }));
    }
  }, [refresh, selectedProjectId]);

  return { githubState, connectGitHub };
}

function useAutoSessionAliasSync(opts: {
  isOpen: boolean;
  mode: "project" | "new-project" | "standalone";
  selectedProjectId: string;
  selectedProjectName: string;
  newProjectName: string;
  existingSessions?: ChatSession[];
}) {
  const { t } = useTranslation();
  const [sessionTitle, setSessionTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);

  const defaultTitle = useMemo(() => {
    const sessions = opts.existingSessions ?? [];
    if (opts.mode === "standalone") {
      const standaloneList = sessions.filter((s) => !s.projectId || s.projectId === "general" || s.projectId === "standalone");
      return generateUniqueSessionTitle({
        projectName: t("workbench.defaultStandaloneAliasPrefix", "Hội thoại"),
        existingTitles: standaloneList.map((s) => s.title),
        existingCount: standaloneList.length,
      });
    }
    if (opts.mode === "new-project") {
      const baseName = opts.newProjectName.trim() || t("workbench.defaultNewProjectName", "Dự án mới");
      return generateUniqueSessionTitle({ projectName: baseName, existingTitles: [], existingCount: 0 });
    }
    const projSessions = sessions.filter((s) => s.projectId === opts.selectedProjectId);
    return generateUniqueSessionTitle({
      projectName: opts.selectedProjectName,
      existingTitles: projSessions.map((s) => s.title),
      existingCount: projSessions.length,
    });
  }, [opts.existingSessions, opts.mode, opts.newProjectName, opts.selectedProjectId, opts.selectedProjectName, t]);

  useEffect(() => {
    if (opts.isOpen && !titleTouched) setSessionTitle(defaultTitle);
  }, [defaultTitle, opts.isOpen, titleTouched]);

  useEffect(() => {
    if (!opts.isOpen) setTitleTouched(false);
  }, [opts.isOpen]);

  return {
    sessionTitle,
    defaultTitle,
    handleSessionTitleChange: (val: string) => {
      setTitleTouched(true);
      setSessionTitle(val);
    },
    resetTitleTouched: () => setTitleTouched(false),
  };
}

export function useNewConvoDialogState(props: NewConversationModalProps) {
  const [mode, setMode] = useState<"project" | "new-project" | "standalone">("project");
  const [selectedProjectIdOverride, setSelectedProjectIdOverride] = useState<string | null>(null);
  const [entryKind, setEntryKind] = useState<"fe-dag" | "presentation">("fe-dag");
  const [sourceType, setSourceType] = useState<ProjectSourceType | null>(null);
  const [newProjectName, setNewProjectName] = useState("");
  const [projectNameTouched, setProjectNameTouched] = useState(false);
  const [managedSlug, setManagedSlug] = useState("cowok-project");
  const [slugTouched, setSlugTouched] = useState(false);
  const [localFolderPath, setLocalFolderPath] = useState("");
  const [githubRepoUrl, setGithubRepoUrl] = useState("");
  const [isDaemonPickerOpen, setIsDaemonPickerOpen] = useState(false);
  const [pickerInitialTab, setPickerInitialTab] = useState<"managed" | "browse">("browse");

  const selectedProjectId =
    (selectedProjectIdOverride && props.projects.some((p) => p.id === selectedProjectIdOverride)
      ? selectedProjectIdOverride
      : undefined) ||
    props.activeProjectId ||
    props.projects[0]?.id ||
    DEFAULT_FLOW_PROJECT_ID;

  const selectedProjectObj = props.projects.find((p) => p.id === selectedProjectId) ?? props.projects[0];
  const selectedProjectName = selectedProjectObj?.name || "cowok-design";

  useEffect(() => {
    if (!props.isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isDaemonPickerOpen) props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isDaemonPickerOpen, props]);

  const { githubState, connectGitHub } = useGitHubStatusSync(props.isOpen, selectedProjectId);
  const aliasSync = useAutoSessionAliasSync({
    isOpen: props.isOpen,
    mode,
    selectedProjectId,
    selectedProjectName,
    newProjectName,
    existingSessions: props.existingSessions,
  });

  const handleProjectNameChange = (val: string) => {
    setProjectNameTouched(true);
    setNewProjectName(val);
    if (!slugTouched) setManagedSlug(toManagedSlug(val));
  };

  const handleFolderPicked = (selectedPath: string | null, slug: string) => {
    setSourceType("local");
    setManagedSlug(slug);
    setSlugTouched(true);
    if (!selectedPath) return;
    setLocalFolderPath(selectedPath);
    const base = extractFolderBasename(selectedPath) || slug;
    if (base && !projectNameTouched) setNewProjectName(base);
  };

  const handleGithubRepoUrlChange = (val: string) => {
    setGithubRepoUrl(val);
    const repoBase = extractFolderBasename(val);
    if (repoBase && !projectNameTouched) {
      setNewProjectName(repoBase);
      if (!slugTouched) setManagedSlug(toManagedSlug(repoBase));
    }
  };

  const handleSetMode = (nextMode: "project" | "new-project" | "standalone") => {
    setMode(nextMode);
    aliasSync.resetTitleTouched();
  };

  return {
    mode, setMode: handleSetMode, selectedProjectId,
    setSelectedProjectId: (id: string) => { setSelectedProjectIdOverride(id); aliasSync.resetTitleTouched(); },
    entryKind, setEntryKind, sourceType, setSourceType, newProjectName, handleProjectNameChange,
    managedSlug, localFolderPath, setLocalFolderPath, githubRepoUrl, handleGithubRepoUrlChange,
    githubState, connectGitHub, ...aliasSync,
    isDaemonPickerOpen, setIsDaemonPickerOpen, pickerInitialTab, setPickerInitialTab, handleFolderPicked,
  };
}
