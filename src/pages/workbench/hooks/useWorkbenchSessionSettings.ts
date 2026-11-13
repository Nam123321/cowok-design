// @story 04.1
import { useCallback, useEffect, useState } from "react";
import { getGlobalLocale, setGlobalLocale } from "../../../hooks/useTranslation";
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

function buildNewProjectObjects(
  projectName: string,
  initialTitle: string,
  provenance: "folder-backed" | "scratch",
  localPath?: string
): { newProj: WorkbenchProject; newSession: ChatSession } {
  const newProjId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : DEFAULT_FLOW_PROJECT_ID;
  const slug = projectName.toLowerCase().replace(/\s+/g, "-");
  const resolvedPath = localPath || (provenance === "folder-backed" ? `./projects/${slug}` : `~/.cowok-design/projects/${slug}`);
  return {
    newProj: { id: newProjId, name: projectName, isExpanded: true, provenance, localPath: resolvedPath },
    newSession: { id: `session-${Date.now()}`, title: initialTitle, projectId: newProjId, updatedAt: "Vừa xong" },
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

export function useWorkbenchProjectsAndSessions() {
  const [projects, setProjects] = useState<WorkbenchProject[]>(DEFAULT_PROJECTS);
  const [sessions, setSessions] = useState<ChatSession[]>(DEFAULT_SESSIONS);
  const [activeSessionId, setActiveSessionId] = useState("session-2");
  const [activeProjectId, setActiveProjectId] = useState(DEFAULT_FLOW_PROJECT_ID);
  const [isNewConvoModalOpen, setIsNewConvoModalOpen] = useState(false);

  // F5 Recovery: Hydrate projects and sessions from SQLite via loopback BFF
  useEffect(() => {
    let isCancelled = false;
    async function hydrateProjectsAndSessions() {
      try {
        const list = await apiHarnessDaemon.listProjects();
        if (Array.isArray(list) && list.length > 0 && !isCancelled) {
          const mappedProjs: WorkbenchProject[] = list.map((p) => {
            const isScratchRoot = !p.workspace_root || p.workspace_root.includes("/.cowok-design/projects/");
            return {
              id: p.id,
              name: p.name,
              isExpanded: true,
              provenance: isScratchRoot ? "scratch" : "folder-backed",
              localPath: p.workspace_root || `./projects/${p.slug || p.id}`,
            };
          });
          setProjects(mappedProjs);

          const targetProjId = mappedProjs[0].id;
          const sessList = await apiHarnessDaemon.listSessions(targetProjId);
          if (Array.isArray(sessList) && sessList.length > 0 && !isCancelled) {
            const mappedSess: ChatSession[] = sessList.map((s) => ({
              id: s.id,
              title: s.title,
              projectId: s.project_id || targetProjId,
              updatedAt: new Date(s.updated_at || Date.now()).toLocaleTimeString(),
            }));
            setSessions(mappedSess);
            setActiveSessionId(mappedSess[0].id);
            setActiveProjectId(targetProjId);
          }
        }
      } catch {
        // Fallback to static defaults
      }
    }
    hydrateProjectsAndSessions();
    return () => {
      isCancelled = true;
    };
  }, []);

  const toggleProjectFolder = useCallback((id: string) => {
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, isExpanded: p.isExpanded === false } : p)));
  }, []);

  const handleCreateInProject = useCallback((targetProjectId: string, title: string) => {
    const sId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    const s: ChatSession = { id: sId, title, projectId: targetProjectId, updatedAt: "Vừa xong" };
    setSessions((prev) => [s, ...prev]);
    setActiveSessionId(s.id);
    setActiveProjectId(targetProjectId);

    apiHarnessDaemon.createSession(targetProjectId, { id: sId, title }).catch(() => undefined);
  }, []);

  const handleCreateNewProject = useCallback((name: string, title: string, provenance: "folder-backed" | "scratch" = "folder-backed", localPath?: string) => {
    const { newProj, newSession } = buildNewProjectObjects(name, title, provenance, localPath);
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
  }, []);

  const handleCreateStandalone = useCallback((title: string) => {
    const s: ChatSession = { id: `session-${Date.now()}`, title, projectId: undefined, updatedAt: "Vừa xong" };
    setSessions((prev) => [s, ...prev]);
    setActiveSessionId(s.id);
  }, []);

  const crud = useSessionCrudActions(activeSessionId, setActiveSessionId, setSessions, setProjects);

  return {
    projects, sessions, activeSessionId, setActiveSessionId, activeProjectId, setActiveProjectId,
    isNewConvoModalOpen, setIsNewConvoModalOpen, toggleProjectFolder, handleCreateInProject,
    handleCreateNewProject, handleCreateStandalone, ...crud,
  };
}
