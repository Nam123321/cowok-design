// @story 04.1, 04.2
// @implements AC-03, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectDto } from "../../../lib/validations/harness-daemon";
import { DEFAULT_FLOW_PROJECT_ID, type WorkbenchProject } from "../constants";
import type { ChatSession } from "../types";
import {
  collectFetchedProjectSessions,
  extractDaemonErrorMessage,
  mapDtoToWorkbenchProject,
  resolveValidatedActiveProjectId,
  toManagedSlug,
  toPlaceholderTrashedDto,
  useProjectTrashAndBrowse,
  type WorkbenchToastState,
  writeActiveProjectHint,
} from "./useProjectTrashAndBrowse";
export {
  WORKBENCH_SETTINGS_STORAGE_KEY,
  loadPersistedSettings,
  savePersistedSettings,
  useWorkbenchSettings,
} from "./useWorkbenchSettingsStorage";

function buildNewProjectObjects(input: {
  projectName: string;
  initialTitle: string;
  provenance: "folder-backed" | "scratch";
  justNowText: string;
  localPath?: string;
  managedSlug?: string;
}): { newProj: WorkbenchProject; newSession: ChatSession; slug: string } {
  const newProjId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : DEFAULT_FLOW_PROJECT_ID;
  const newSessionId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : "22222222-2222-4222-8222-222222222299";
  const slug = toManagedSlug(input.managedSlug || input.projectName);
  const resolvedPath = input.localPath || (input.provenance === "scratch" ? `~/.cowok-design/projects/${slug}` : `~/CowokProjects/${slug}`);
  return {
    slug,
    newProj: { id: newProjId, name: input.projectName, isExpanded: true, provenance: input.provenance, localPath: resolvedPath },
    newSession: { id: newSessionId, title: input.initialTitle, projectId: newProjId, updatedAt: input.justNowText },
  };
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

function useSessionCrudActions(deps: SessionCrudDeps) {
  const { projectsRef, sessionsRef, activeSessionIdRef, setActiveSessionIdState, setSessions, setProjects, showToast, t } = deps;
  const notifyErr = useCallback((err: unknown) => {
    showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  }, [showToast, t]);

  // Pin state is UI-only because SQLite `chat_sessions` has no `is_pinned` column.
  const handleTogglePinSession = useCallback((id: string) => {
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, isPinned: !s.isPinned } : s)));
  }, [setSessions]);

  const handleRenameSession = useCallback((id: string, title: string) => {
    const trimmed = title.trim();
    if (!trimmed) return;
    const target = sessionsRef.current.find((s) => s.id === id);
    const prevTitle = target?.title;
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, title: trimmed } : s)));
    if (target?.projectId) {
      void apiHarnessDaemon.updateSession(target.projectId, id, { title: trimmed }).catch((err) => {
        if (prevTitle !== undefined) {
          setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, title: prevTitle } : s)));
        }
        notifyErr(err);
      });
    }
  }, [notifyErr, sessionsRef, setSessions]);

  const handleDeleteSession = useCallback((id: string) => {
    const prevSessions = sessionsRef.current;
    const prevActSess = activeSessionIdRef.current;
    const target = prevSessions.find((s) => s.id === id);
    const remaining = prevSessions.filter((s) => s.id !== id);
    setSessions(remaining);
    if (prevActSess === id && remaining.length > 0) setActiveSessionIdState(remaining[0].id);
    if (target?.projectId) {
      void apiHarnessDaemon.deleteSession(target.projectId, id).catch((err) => {
        setSessions(prevSessions);
        setActiveSessionIdState(prevActSess);
        notifyErr(err);
      });
    }
  }, [activeSessionIdRef, notifyErr, sessionsRef, setActiveSessionIdState, setSessions]);

  const handleRenameProject = useCallback((id: string, name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const prevName = projectsRef.current.find((p) => p.id === id)?.name;
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, name: trimmed } : p)));
    void apiHarnessDaemon.updateProject(id, { name: trimmed }).catch((err) => {
      if (prevName !== undefined) {
        setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, name: prevName } : p)));
      }
      notifyErr(err);
    });
  }, [notifyErr, projectsRef, setProjects]);

  const handleDeleteProject = useCallback((id: string) => executeProjectDelete(id, deps, notifyErr), [deps, notifyErr]);

  return { handleTogglePinSession, handleRenameSession, handleDeleteSession, handleRenameProject, handleDeleteProject };
}

function resolveProjectLocationArg(location?: string | { localPath?: string; managedSlug?: string }) {
  if (!location) return {};
  return typeof location === "string" ? { localPath: location } : location;
}

function useInitialProjectHydration(args: {
  syncProjectDtos: (dtos: ProjectDto[]) => { activeDtos: ProjectDto[] };
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>;
  setActiveSessionIdState: React.Dispatch<React.SetStateAction<string>>;
  formatTime: (v: number) => string;
  onError: (err: unknown) => void;
}) {
  const { syncProjectDtos, setProjects, setActiveProjectId, setSessions, setActiveSessionIdState, formatTime, onError } = args;
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await apiHarnessDaemon.listProjects({ includeTrashed: true });
        if (!Array.isArray(list) || cancelled) return;
        const { activeDtos } = syncProjectDtos(list);
        const mappedProjs = activeDtos.map(mapDtoToWorkbenchProject);
        setProjects(mappedProjs);
        const targetProjId = resolveValidatedActiveProjectId(mappedProjs);
        setActiveProjectId(targetProjId);
        if (mappedProjs.length === 0) {
          setSessions((prev) => prev.filter(isGenuineStandaloneSession));
          setActiveSessionIdState("");
          return;
        }
        const settled = await Promise.allSettled(mappedProjs.map(async (proj) => ({ projId: proj.id, sessList: await apiHarnessDaemon.listSessions(proj.id) })));
        if (cancelled) return;
        const { fetchedByProject, allFetchedSessions, seenIds } = collectFetchedProjectSessions(settled, formatTime);
        setSessions((prev) => [...allFetchedSessions, ...prev.filter((ex) => isGenuineStandaloneSession(ex) && !seenIds.has(ex.id))]);
        const targetSessions = fetchedByProject.get(targetProjId) || allFetchedSessions;
        setActiveSessionIdState(targetSessions[0]?.id || "");
      } catch (err) {
        if (!cancelled) {
          setProjects([]);
          setSessions((prev) => prev.filter(isGenuineStandaloneSession));
          setActiveProjectId("");
          setActiveSessionIdState("");
          onError(err);
        }
      }
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

export function useWorkbenchProjectsAndSessions() {
  const { t, locale } = useTranslation();
  const { formatTime } = useFormatter({ locale });
  const justNowText = t("workbench.justNow", "Vừa xong");
  const [projects, setProjects] = useState<WorkbenchProject[]>([]);
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [activeSessionId, setActiveSessionIdState] = useState("");
  const [activeProjectId, setActiveProjectIdState] = useState("");
  const [isNewConvoModalOpen, setIsNewConvoModalOpen] = useState(false);

  const projectsRef = useRef(projects); projectsRef.current = projects;
  const sessionsRef = useRef(sessions); sessionsRef.current = sessions;
  const activeSessionIdRef = useRef(activeSessionId); activeSessionIdRef.current = activeSessionId;
  const activeProjectIdRef = useRef(activeProjectId); activeProjectIdRef.current = activeProjectId;

  const setActiveProjectId: React.Dispatch<React.SetStateAction<string>> = useCallback((valOrFn) => {
    const next = typeof valOrFn === "function" ? valOrFn(activeProjectIdRef.current) : valOrFn;
    setActiveProjectIdState(next);
    writeActiveProjectHint(next);
  }, []);

  const setActiveSessionId: React.Dispatch<React.SetStateAction<string>> = useCallback((valOrFn) => {
    const next = typeof valOrFn === "function" ? valOrFn(activeSessionIdRef.current) : valOrFn;
    setActiveSessionIdState(next);
    const hit = sessionsRef.current.find((s) => s.id === next);
    if (hit?.projectId) setActiveProjectId(hit.projectId);
  }, [setActiveProjectId]);

  const trash = useProjectTrashAndBrowse({ setProjects, setActiveProjectId, t });
  const handleHydrationError = useCallback((err: unknown) => {
    trash.showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  }, [t, trash]);
  useInitialProjectHydration({ syncProjectDtos: trash.syncProjectDtos, setProjects, setActiveProjectId, setSessions, setActiveSessionIdState, formatTime, onError: handleHydrationError });

  const toggleProjectFolder = useCallback((id: string) => {
    setProjects((prev) => prev.map((p) => (p.id === id ? { ...p, isExpanded: p.isExpanded === false } : p)));
  }, []);

  const handleCreateInProject = useCallback((targetProjectId: string, title: string) => {
    const prevSessions = sessionsRef.current;
    const prevActSess = activeSessionIdRef.current;
    const prevActProj = activeProjectIdRef.current;
    const sId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    setSessions((prev) => [{ id: sId, title, projectId: targetProjectId, updatedAt: justNowText }, ...prev]);
    setActiveSessionIdState(sId);
    setActiveProjectId(targetProjectId);
    void apiHarnessDaemon.createSession(targetProjectId, { id: sId, title }).catch((err) => {
      setSessions(prevSessions);
      setActiveSessionIdState(prevActSess);
      setActiveProjectId(prevActProj);
      trash.showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }, [justNowText, setActiveProjectId, t, trash]);

  const handleCreateNewProject = useCallback((name: string, title: string, provenance: "folder-backed" | "scratch" = "folder-backed", location?: string | { localPath?: string; managedSlug?: string }) => {
    const prevSnapshot = { sess: sessionsRef.current, actSess: activeSessionIdRef.current };
    const { localPath, managedSlug } = resolveProjectLocationArg(location);
    const { newProj, newSession, slug } = buildNewProjectObjects({ projectName: name, initialTitle: title, provenance, justNowText, localPath, managedSlug });
    const isManagedDestination =
      provenance === "folder-backed" &&
      (!localPath || localPath.endsWith(`/CowokProjects/${managedSlug || slug}`));
    const effectiveManagedSlug = isManagedDestination ? (managedSlug || slug) : undefined;
    void apiHarnessDaemon.createProject({
      id: newProj.id,
      slug,
      managed_slug: effectiveManagedSlug,
      name: newProj.name,
      product_brief: `Created via workbench in ${provenance} mode`,
      workspace_root: provenance === "scratch" ? null : (localPath || null),
      provenance,
    })
      .then((dto) => {
        const mappedProj = mapDtoToWorkbenchProject(dto);
        setProjects((prev) => (prev.some((p) => p.id === dto.id) ? prev.map((p) => (p.id === dto.id ? mappedProj : p)) : [...prev, mappedProj]));
        setSessions((prev) => (prev.some((s) => s.id === newSession.id) ? prev : [newSession, ...prev]));
        setActiveSessionIdState(newSession.id);
        setActiveProjectId(dto.id);
        trash.showToast({ kind: "success", message: `${t("workbench.projectCreatedToast", "Đã tạo dự án mới trong SQLite V7")}: ${dto.name}` });
        void apiHarnessDaemon.createSession(dto.id, { id: newSession.id, title: newSession.title }).catch((sessErr) => {
          setSessions(prevSnapshot.sess);
          setActiveSessionIdState(prevSnapshot.actSess);
          trash.showToast({ kind: "warning", message: extractDaemonErrorMessage(sessErr, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
        });
      })
      .catch((err) => {
        trash.showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
      });
  }, [justNowText, setActiveProjectId, t, trash]);

  const handleCreateStandalone = useCallback((title: string) => {
    const sId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    const s: ChatSession = { id: sId, title, projectId: undefined, updatedAt: justNowText };
    setSessions((prev) => [s, ...prev]);
    setActiveSessionIdState(s.id);
  }, [justNowText]);

  const crud = useSessionCrudActions({
    projectsRef, sessionsRef, activeSessionIdRef, activeProjectIdRef, setActiveSessionIdState, setActiveProjectId,
    setSessions, setProjects, showToast: trash.showToast, addTrashedProject: trash.addTrashedProject, t,
  });

  return {
    projects, sessions, activeSessionId, setActiveSessionId, activeProjectId, setActiveProjectId,
    isNewConvoModalOpen, setIsNewConvoModalOpen, toggleProjectFolder, handleCreateInProject,
    handleCreateNewProject, handleCreateStandalone, ...crud, ...trash,
  };
}
