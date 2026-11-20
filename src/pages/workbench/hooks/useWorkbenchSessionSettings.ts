// @story 04.1, 04.2
// @implements AC-03, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  ProjectDtoSchema,
  type ProjectDto,
} from "../../../lib/validations/harness-daemon";
import type { WorkbenchProject } from "../constants";
import type { ChatSession } from "../types";
import {
  readPinnedSessionIds,
  readStandaloneSessions,
  writePinnedSessionIds,
  writeStandaloneSessions,
} from "./usePinnedSessionsStorage";
import {
  collectFetchedProjectSessions,
  extractDaemonErrorMessage,
  mapDtoToWorkbenchProject,
  resolveValidatedActiveProjectId,
  toPlaceholderTrashedDto,
  useProjectTrashAndBrowse,
  type WorkbenchToastState,
  writeActiveProjectHint,
} from "./useProjectTrashAndBrowse";
import {
  useLaunchpadEventSync,
  useProjectCreationActions,
} from "./useWorkbenchProjectCreation";
export {
  WORKBENCH_SETTINGS_STORAGE_KEY,
  loadPersistedSettings,
  savePersistedSettings,
  useWorkbenchSettings,
} from "./useWorkbenchSettingsStorage";

export function resolveTargetProjectId(projectId?: string | null): string | null {
  if (projectId && projectId.trim().length > 0) return projectId.trim();
  if (typeof window !== "undefined") {
    const bootstrapDefault = (window as unknown as { __COWOK_BOOTSTRAP__?: { defaultProjectId?: string } })
      .__COWOK_BOOTSTRAP__?.defaultProjectId;
    if (bootstrapDefault && bootstrapDefault.trim().length > 0) return bootstrapDefault.trim();
    const stored =
      window.localStorage.getItem("cowok:active-project-id") ||
      window.localStorage.getItem("cowok.activeProjectId");
    if (stored && stored.trim().length > 0) return stored.trim();
  }
  return null;
}

function isGenuineStandaloneSession(s: ChatSession): boolean {
  return !s.projectId || s.projectId === "general" || s.projectId === "standalone";
}

interface SessionCrudDeps {
  projectsRef: React.MutableRefObject<WorkbenchProject[]>;
  sessionsRef: React.MutableRefObject<ChatSession[]>;
  activeSessionIdRef: React.MutableRefObject<string>;
  activeProjectIdRef: React.MutableRefObject<string>;
  setActiveSessionIdState: React.Dispatch<React.SetStateAction<string>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>;
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  showToast: (next: WorkbenchToastState) => void;
  addTrashedProject: (proj: ProjectDto) => void;
  t: (key: string, fallback: string) => string;
}

function executeProjectDelete(id: string, deps: SessionCrudDeps, notifyErr: (err: unknown) => void) {
  const { projectsRef, sessionsRef, activeProjectIdRef, activeSessionIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, addTrashedProject, showToast, t } = deps;
  const prevProjects = projectsRef.current;
  const prevSessions = sessionsRef.current;
  const prevActProj = activeProjectIdRef.current;
  const prevActSess = activeSessionIdRef.current;
  const removed = prevProjects.find((p) => p.id === id);
  const nextProjs = prevProjects.filter((p) => p.id !== id);
  const nextSessions = prevSessions.filter((s) => s.projectId !== id);
  setProjects(nextProjs);
  setSessions(nextSessions);
  if (prevActProj === id) {
    const nextProjId = nextProjs[0]?.id || "";
    setActiveProjectId(nextProjId);
    setActiveSessionIdState(nextSessions.find((s) => s.projectId === nextProjId)?.id ?? nextSessions[0]?.id ?? "");
  }
  void apiHarnessDaemon.deleteProject(id).then((res) => {
    if (removed) addTrashedProject(toPlaceholderTrashedDto(removed, res.trash_deleted_at || Date.now()));
    showToast({ kind: "warning", message: `${t("workbench.projectDeletedToast", "Đã chuyển dự án vào Thùng rác")}: ${removed?.name || id}`, undoProjectId: id });
  }).catch((err) => {
    setProjects(prevProjects);
    setSessions(prevSessions);
    setActiveProjectId(prevActProj);
    setActiveSessionIdState(prevActSess);
    notifyErr(err);
  });
}

function useSessionRenameDeleteActions(deps: SessionCrudDeps, notifyErr: (err: unknown) => void) {
  const { sessionsRef, activeSessionIdRef, setActiveSessionIdState, setSessions } = deps;
  const handleTogglePinSession = useCallback((id: string) => {
    setSessions((prev) => {
      const next = prev.map((s) => (s.id === id ? { ...s, isPinned: !s.isPinned } : s));
      writePinnedSessionIds(next);
      return next;
    });
  }, [setSessions]);

  const handleRenameSession = useCallback((id: string, title: string) => {
    const trimmed = title.trim();
    if (!trimmed) return;
    const target = sessionsRef.current.find((s) => s.id === id);
    const prevTitle = target?.title;
    setSessions((prev) => {
      const next = prev.map((s) => (s.id === id ? { ...s, title: trimmed } : s));
      if (!target?.projectId) writeStandaloneSessions(next);
      return next;
    });
    if (!target?.projectId) return;
    void apiHarnessDaemon.updateSession(target.projectId, id, { title: trimmed }).catch((err) => {
      if (prevTitle !== undefined) setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, title: prevTitle } : s)));
      notifyErr(err);
    });
  }, [notifyErr, sessionsRef, setSessions]);

  const handleDeleteSession = useCallback((id: string) => {
    const prev = sessionsRef.current;
    const prevAct = activeSessionIdRef.current;
    const target = prev.find((s) => s.id === id);
    const remaining = prev.filter((s) => s.id !== id);
    setSessions(remaining);
    writePinnedSessionIds(remaining);
    if (!target?.projectId) writeStandaloneSessions(remaining);
    if (prevAct === id && remaining.length > 0) setActiveSessionIdState(remaining[0].id);
    if (!target?.projectId) return;
    void apiHarnessDaemon.deleteSession(target.projectId, id).catch((err) => {
      setSessions(prev);
      writePinnedSessionIds(prev);
      setActiveSessionIdState(prevAct);
      notifyErr(err);
    });
  }, [activeSessionIdRef, notifyErr, sessionsRef, setActiveSessionIdState, setSessions]);

  return { handleTogglePinSession, handleRenameSession, handleDeleteSession };
}

function useSessionCrudActions(deps: SessionCrudDeps) {
  const { projectsRef, setProjects, showToast, t } = deps;
  const notifyErr = useCallback((err: unknown) => {
    showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  }, [showToast, t]);

  const sessionActions = useSessionRenameDeleteActions(deps, notifyErr);

  const handleRenameProject = useCallback((id: string, name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const prevName = projectsRef.current.find((p) => p.id === id)?.name;
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, name: trimmed } : p)));
    void apiHarnessDaemon.updateProject(id, { name: trimmed }).catch((err) => {
      if (prevName !== undefined) setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, name: prevName } : p)));
      notifyErr(err);
    });
  }, [notifyErr, projectsRef, setProjects]);

  const handleDeleteProject = useCallback((id: string) => executeProjectDelete(id, deps, notifyErr), [deps, notifyErr]);

  return { ...sessionActions, handleRenameProject, handleDeleteProject };
}

interface InitialHydrationArgs {
  syncProjectDtos: (dtos: ProjectDto[]) => { activeDtos: ProjectDto[] };
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>;
  setActiveSessionIdState: React.Dispatch<React.SetStateAction<string>>;
  formatTime: (v: number) => string;
  onError: (err: unknown) => void;
}

async function runInitialHydrationPass(args: InitialHydrationArgs, isCancelled: () => boolean) {
  const { syncProjectDtos, setProjects, setActiveProjectId, setSessions, setActiveSessionIdState, formatTime, onError } = args;
  try {
    const list = await apiHarnessDaemon.listProjects({ includeTrashed: true });
    if (!Array.isArray(list) || isCancelled()) return;
    const { activeDtos } = syncProjectDtos(list);
    const mappedProjs = activeDtos.map(mapDtoToWorkbenchProject);
    const targetProjId = resolveValidatedActiveProjectId(mappedProjs);
    setProjects((prev) => {
      const seen = new Set(mappedProjs.map((p) => p.id));
      return [...mappedProjs, ...prev.filter((p) => !seen.has(p.id))];
    });
    setActiveProjectId((curr) => curr || targetProjId);
    if (mappedProjs.length === 0) {
      setSessions((prev) => prev.filter(isGenuineStandaloneSession));
      setActiveSessionIdState("");
      return;
    }
    const settled = await Promise.allSettled(
      mappedProjs.map(async (proj) => ({ projId: proj.id, sessList: await apiHarnessDaemon.listSessions(proj.id) }))
    );
    if (isCancelled()) return;
    const { fetchedByProject, allFetchedSessions, seenIds } = collectFetchedProjectSessions(settled, formatTime);
    const pinnedIds = readPinnedSessionIds();
    const hydratedFetched = allFetchedSessions.map((s) => (pinnedIds.has(s.id) ? { ...s, isPinned: true } : s));
    setSessions((prev) => [...hydratedFetched, ...prev.filter((ex) => (isGenuineStandaloneSession(ex) || !seenIds.has(ex.id)) && !seenIds.has(ex.id))]);
    const targetSessions = fetchedByProject.get(targetProjId) || allFetchedSessions;
    setActiveSessionIdState((curr) => curr || targetSessions[0]?.id || "");
  } catch (err) {
    if (!isCancelled()) {
      setProjects([]);
      setSessions((prev) => prev.filter(isGenuineStandaloneSession));
      setActiveProjectId("");
      setActiveSessionIdState("");
      onError(err);
    }
  }
}

function useInitialProjectHydration(args: InitialHydrationArgs) {
  useEffect(() => {
    let cancelled = false;
    void runInitialHydrationPass(args, () => cancelled);
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

export function useWorkbenchProjectsAndSessions() {
  const { t, locale } = useTranslation();
  const { formatTime } = useFormatter({ locale });
  const justNowText = t("workbench.justNow", "Vừa xong");
  const [projects, setProjects] = useState<WorkbenchProject[]>([]);
  const [sessions, setSessions] = useState<ChatSession[]>(() => readStandaloneSessions());
  const [activeSessionId, setActiveSessionIdState] = useState("");
  const [activeProjectId, setActiveProjectIdState] = useState("");
  const [isNewConvoModalOpen, setIsNewConvoModalOpen] = useState(false);

  const projectsRef = useRef(projects); projectsRef.current = projects;
  const sessionsRef = useRef(sessions); sessionsRef.current = sessions;
  const activeSessionIdRef = useRef(activeSessionId); activeSessionIdRef.current = activeSessionId;
  const activeProjectIdRef = useRef(activeProjectId); activeProjectIdRef.current = activeProjectId;

  const setActiveProjectId: React.Dispatch<React.SetStateAction<string>> = useCallback((valOrFn) => {
    const next = typeof valOrFn === "function" ? valOrFn(activeProjectIdRef.current) : valOrFn;
    setActiveProjectIdState(next); writeActiveProjectHint(next);
  }, []);

  const setActiveSessionId: React.Dispatch<React.SetStateAction<string>> = useCallback((valOrFn) => {
    const next = typeof valOrFn === "function" ? valOrFn(activeSessionIdRef.current) : valOrFn;
    setActiveSessionIdState(next);
    const hit = sessionsRef.current.find((s) => s.id === next);
    if (hit?.projectId) setActiveProjectId(hit.projectId);
  }, [setActiveProjectId]);

  const trash = useProjectTrashAndBrowse({ setProjects, setActiveProjectId, t });
  const { showToast, syncProjectDtos } = trash;
  const handleHydrationError = useCallback((err: unknown) => {
    showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  }, [showToast, t]);
  useInitialProjectHydration({ syncProjectDtos, setProjects, setActiveProjectId, setSessions, setActiveSessionIdState, formatTime, onError: handleHydrationError });
  useLaunchpadEventSync({ justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t });

  const toggleProjectFolder = useCallback((id: string) => {
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, isExpanded: p.isExpanded === false } : p)));
  }, []);
  const creation = useProjectCreationActions({ justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t });
  const crud = useSessionCrudActions({ projectsRef, sessionsRef, activeSessionIdRef, activeProjectIdRef, setActiveSessionIdState, setActiveProjectId, setSessions, setProjects, showToast, addTrashedProject: trash.addTrashedProject, t });

  return {
    projects, sessions, activeSessionId, setActiveSessionId, activeProjectId, setActiveProjectId,
    isNewConvoModalOpen, setIsNewConvoModalOpen, toggleProjectFolder, ...creation, ...crud, ...trash,
  };
}
