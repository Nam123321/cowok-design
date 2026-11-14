// @story 04.1
import { useCallback, useEffect, useState } from "react";
import { getGlobalLocale, setGlobalLocale, useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ThemeMode } from "../components/WorkbenchTopNav";
import { DEFAULT_FLOW_PROJECT_ID, DEFAULT_PROJECTS, DEFAULT_SESSIONS, type WorkbenchProject } from "../constants";
import type { ChatSession, PersistedWorkbenchSettings } from "../types";
import { apiHarnessDaemon } from "../../../api/harness-daemon";

export const WORKBENCH_SETTINGS_STORAGE_KEY = "cowok:workbench:settings";

function loadPersistedSettings(): Partial<PersistedWorkbenchSettings> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(WORKBENCH_SETTINGS_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Partial<PersistedWorkbenchSettings>) : {};
  } catch {
    return {};
  }
}

export function savePersistedSettings(patch: Partial<PersistedWorkbenchSettings>): void {
  if (typeof window === "undefined") return;
  try {
    const prev = loadPersistedSettings();
    const safeNext: Partial<PersistedWorkbenchSettings> = {
      theme: patch.theme ?? prev.theme ?? "warm",
      locale: patch.locale ?? prev.locale ?? "vi",
      fontSize: patch.fontSize ?? prev.fontSize ?? "13px",
      defaultModel: patch.defaultModel ?? prev.defaultModel ?? "claude-sonnet-4-6",
      byosEndpoint: patch.byosEndpoint ?? prev.byosEndpoint ?? "http://127.0.0.1:43110/v1",
      autoContextSlice: patch.autoContextSlice ?? prev.autoContextSlice ?? true,
    };
    window.localStorage.setItem(WORKBENCH_SETTINGS_STORAGE_KEY, JSON.stringify(safeNext));
  } catch {
    // Storage restricted in test sandbox
  }
}

export function useWorkbenchSettings() {
  const initial = loadPersistedSettings();
  const [themeMode, setThemeModeState] = useState<ThemeMode>(initial.theme || "warm");
  const [currentLocale, setCurrentLocaleState] = useState<string>(initial.locale || getGlobalLocale());
  const [currentFontSize, setCurrentFontSizeState] = useState(initial.fontSize || "13px");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [settingsActiveTab, setSettingsActiveTab] = useState<"general" | "models" | "mcp" | "automations">("general");

  const setThemeMode = useCallback((t: ThemeMode) => { setThemeModeState(t); savePersistedSettings({ theme: t }); }, []);
  const setCurrentLocale = useCallback((l: string) => { setCurrentLocaleState(l); setGlobalLocale(l); savePersistedSettings({ locale: l }); }, []);
  const setCurrentFontSize = useCallback((s: string) => { setCurrentFontSizeState(s); savePersistedSettings({ fontSize: s }); }, []);
  const openSettings = useCallback((tab: "general" | "models" | "mcp" | "automations" = "general") => { setSettingsActiveTab(tab); setIsSettingsOpen(true); }, []);

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.setAttribute("data-theme", themeMode);
      document.documentElement.dataset.theme = themeMode;
    }
  }, [themeMode]);

  return {
    themeMode, setThemeMode, currentLocale, setCurrentLocale, currentFontSize, setCurrentFontSize,
    isSettingsOpen, setIsSettingsOpen, settingsActiveTab, setSettingsActiveTab, openSettings,
  };
}

interface NewProjectBuildInput {
  projectName: string;
  initialTitle: string;
  provenance: "folder-backed" | "scratch";
  justNowText: string;
  localPath?: string;
}

function buildNewProjectObjects({
  projectName,
  initialTitle,
  provenance,
  justNowText,
  localPath,
}: NewProjectBuildInput): { newProj: WorkbenchProject; newSession: ChatSession } {
  const newProjId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : DEFAULT_FLOW_PROJECT_ID;
  const slug = projectName.toLowerCase().replace(/\s+/g, "-");
  const resolvedPath = localPath || (provenance === "folder-backed" ? `./projects/${slug}` : `~/.cowok-design/projects/${slug}`);
  return {
    newProj: { id: newProjId, name: projectName, isExpanded: true, provenance, localPath: resolvedPath },
    newSession: { id: `session-${Date.now()}`, title: initialTitle, projectId: newProjId, updatedAt: justNowText },
  };
}

function useSessionCrudActions(
  activeSessionId: string,
  setActiveSessionId: React.Dispatch<React.SetStateAction<string>>,
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>,
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>
) {
  const handleTogglePinSession = useCallback((id: string) => {
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, isPinned: !s.isPinned } : s)));
  }, [setSessions]);

  const handleRenameSession = useCallback((id: string, title: string) => {
    if (title.trim()) {
      setSessions((prev) => {
        const target = prev.find((s) => s.id === id);
        if (target?.projectId) {
          apiHarnessDaemon.updateSession(target.projectId, id, { title: title.trim() }).catch(() => undefined);
        }
        return prev.map((s) => (s.id === id ? { ...s, title: title.trim() } : s));
      });
    }
  }, [setSessions]);

  const handleDeleteSession = useCallback((id: string) => {
    setSessions((prev) => {
      const target = prev.find((s) => s.id === id);
      if (target?.projectId) {
        apiHarnessDaemon.deleteSession(target.projectId, id).catch(() => undefined);
      }
      const filtered = prev.filter((s) => s.id !== id);
      if (activeSessionId === id && filtered.length > 0) setActiveSessionId(filtered[0].id);
      return filtered;
    });
  }, [activeSessionId, setActiveSessionId, setSessions]);

  const handleRenameProject = useCallback((id: string, name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, name: trimmed } : p)));
    void apiHarnessDaemon.updateProject(id, { name: trimmed }).catch(() => {
      // Offline optimistic UI fallback until daemon reconnects
    });
  }, [setProjects]);

  const handleDeleteProject = useCallback((id: string) => {
    setProjects((prev) => prev.filter((p) => p.id !== id));
    setSessions((prev) => prev.filter((s) => s.projectId !== id));
    void apiHarnessDaemon.deleteProject(id).catch(() => {
      // Offline optimistic UI fallback until daemon reconnects
    });
  }, [setProjects, setSessions]);

  const handleLinkProjectFolder = useCallback((id: string, path: string) => {
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, provenance: "folder-backed", localPath: path.trim() || p.localPath } : p)));
  }, [setProjects]);

  return { handleTogglePinSession, handleRenameSession, handleDeleteSession, handleRenameProject, handleDeleteProject, handleLinkProjectFolder };
}

type SettledProjectSessions = PromiseSettledResult<{
  projId: string;
  sessList: { id: string; title: string; project_id?: string | null; updated_at?: number | null }[];
}>;

function collectFetchedProjectSessions(
  settled: SettledProjectSessions[],
  formatTime: (v: number) => string
) {
  const fetchedByProject = new Map<string, ChatSession[]>();
  const allFetchedSessions: ChatSession[] = [];
  const seenIds = new Set<string>();

  for (const item of settled) {
    if (item.status !== "fulfilled" || !Array.isArray(item.value.sessList) || item.value.sessList.length === 0) {
      continue;
    }
    const mappedForProj: ChatSession[] = item.value.sessList.map((s) => ({
      id: s.id,
      title: s.title,
      projectId: s.project_id || item.value.projId,
      updatedAt: formatTime(s.updated_at || Date.now()),
    }));
    fetchedByProject.set(item.value.projId, mappedForProj);
    for (const sess of mappedForProj) {
      if (seenIds.has(sess.id)) continue;
      seenIds.add(sess.id);
      allFetchedSessions.push(sess);
    }
  }

  return { fetchedByProject, allFetchedSessions, seenIds };
}

export function useWorkbenchProjectsAndSessions() {
  const { t, locale } = useTranslation();
  const { formatTime } = useFormatter({ locale });
  const justNowText = t("workbench.justNow", "Vừa xong");
  const [projects, setProjects] = useState<WorkbenchProject[]>(DEFAULT_PROJECTS);
  const [sessions, setSessions] = useState<ChatSession[]>(DEFAULT_SESSIONS);
  const [activeSessionId, setActiveSessionId] = useState("session-2");
  const [activeProjectId, setActiveProjectId] = useState(DEFAULT_FLOW_PROJECT_ID);
  const [isNewConvoModalOpen, setIsNewConvoModalOpen] = useState(false);

  async function runHydrateProjectsAndSessions(isCancelled: () => boolean) {
    try {
      const list = await apiHarnessDaemon.listProjects();
      if (!Array.isArray(list) || list.length === 0 || isCancelled()) return;
      const mappedProjs: WorkbenchProject[] = list.map((p) => {
        const isScratchRoot = !p.workspace_root || p.workspace_root.includes("/.cowok-design/projects/");
        return { id: p.id, name: p.name, isExpanded: true, provenance: isScratchRoot ? "scratch" : "folder-backed", localPath: p.workspace_root || `./projects/${p.slug || p.id}` };
      });
      setProjects(mappedProjs);
      const targetProjId = (mappedProjs.find((p) => p.id === DEFAULT_FLOW_PROJECT_ID) || mappedProjs[0]).id;
      const settled = await Promise.allSettled(mappedProjs.map(async (proj) => ({ projId: proj.id, sessList: await apiHarnessDaemon.listSessions(proj.id) })));
      if (isCancelled()) return;
      const { fetchedByProject, allFetchedSessions, seenIds } = collectFetchedProjectSessions(settled, formatTime);
      if (allFetchedSessions.length === 0) return;
      setSessions((prev) => {
        const preserved = prev.filter((existing) => (!existing.projectId || !fetchedByProject.has(existing.projectId)) && !seenIds.has(existing.id));
        return [...allFetchedSessions, ...preserved];
      });
      const targetSessions = fetchedByProject.get(targetProjId) || allFetchedSessions;
      setActiveSessionId(targetSessions[0].id);
      setActiveProjectId(targetProjId);
    } catch {
      // Fallback to static defaults
    }
  }

  // F5 Recovery: Hydrate projects and sessions from SQLite via loopback BFF
  useEffect(() => {
    let cancelled = false;
    runHydrateProjectsAndSessions(() => cancelled);
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleProjectFolder = useCallback((id: string) => {
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, isExpanded: p.isExpanded === false } : p)));
  }, []);

  const handleCreateInProject = useCallback((targetProjectId: string, title: string) => {
    const sId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    const s: ChatSession = { id: sId, title, projectId: targetProjectId, updatedAt: justNowText };
    setSessions((prev) => [s, ...prev]);
    setActiveSessionId(s.id);
    setActiveProjectId(targetProjectId);

    apiHarnessDaemon.createSession(targetProjectId, { id: sId, title }).catch(() => undefined);
  }, [justNowText]);

  const handleCreateNewProject = useCallback((name: string, title: string, provenance: "folder-backed" | "scratch" = "folder-backed", localPath?: string) => {
    const { newProj, newSession } = buildNewProjectObjects({ projectName: name, initialTitle: title, provenance, justNowText, localPath });
    setProjects((prev) => [...prev, newProj]);
    setSessions((prev) => [newSession, ...prev]);
    setActiveSessionId(newSession.id);
    setActiveProjectId(newProj.id);

    apiHarnessDaemon
      .createProject({
        id: newProj.id,
        slug: newProj.name.toLowerCase().replace(/\s+/g, "-"),
        name: newProj.name,
        product_brief: `Created via workbench in ${provenance} mode`,
        workspace_root: localPath || null,
      })
      .then(() =>
        apiHarnessDaemon
          .createSession(newProj.id, { id: newSession.id, title: newSession.title })
          .catch(() => undefined)
      )
      .catch(() => undefined);
  }, [justNowText]);

  const handleCreateStandalone = useCallback((title: string) => {
    const s: ChatSession = { id: `session-${Date.now()}`, title, projectId: undefined, updatedAt: justNowText };
    setSessions((prev) => [s, ...prev]);
    setActiveSessionId(s.id);
  }, [justNowText]);

  const crud = useSessionCrudActions(activeSessionId, setActiveSessionId, setSessions, setProjects);

  return {
    projects, sessions, activeSessionId, setActiveSessionId, activeProjectId, setActiveProjectId,
    isNewConvoModalOpen, setIsNewConvoModalOpen, toggleProjectFolder, handleCreateInProject,
    handleCreateNewProject, handleCreateStandalone, ...crud,
  };
}
