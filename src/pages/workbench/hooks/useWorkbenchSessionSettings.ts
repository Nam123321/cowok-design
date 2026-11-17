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
import { DEFAULT_FLOW_PROJECT_ID, type WorkbenchProject } from "../constants";
import type { ChatSession } from "../types";
import {
  readPinnedSessionIds,
  writePinnedSessionIds,
} from "./usePinnedSessionsStorage";
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

function buildNewProjectObjects(input: {
  projectName: string;
  initialTitle: string;
  provenance: "folder-backed" | "scratch";
  justNowText: string;
  localPath?: string;
  managedSlug?: string;
}): { newProj: WorkbenchProject; newSession: ChatSession; slug: string } {
  const newProjId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : DEFAULT_FLOW_PROJECT_ID;
  const newSessionId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
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
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, title: trimmed } : s)));
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

function resolveProjectLocationArg(location?: string | { localPath?: string; managedSlug?: string }) {
  if (!location) return {};
  return typeof location === "string" ? { localPath: location } : location;
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

interface ProjectSyncDeps {
  justNowText: string;
  sessionsRef: React.MutableRefObject<ChatSession[]>;
  activeSessionIdRef: React.MutableRefObject<string>;
  activeProjectIdRef: React.MutableRefObject<string>;
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setActiveSessionIdState: React.Dispatch<React.SetStateAction<string>>;
  showToast: (toast: WorkbenchToastState) => void;
  t: (k: string, fallback: string) => string;
}

function readEventErrorMessage(ev: Event, fallback: string): string {
  const detail = (ev as CustomEvent<{ message?: unknown }>).detail;
  return typeof detail?.message === "string" && detail.message.trim() ? detail.message : fallback;
}

function handleLaunchpadCreatedDto(ev: Event, deps: ProjectSyncDeps) {
  const { justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const parsed = ProjectDtoSchema.safeParse((ev as CustomEvent<unknown>).detail);
  if (!parsed.success) return;
  const dto = parsed.data;
  const mapped = mapDtoToWorkbenchProject(dto);
  setProjects((prev) => (prev.some((p) => p.id === dto.id) ? prev.map((p) => (p.id === dto.id ? mapped : p)) : [...prev, mapped]));
  setActiveProjectId(dto.id);
  if (!sessionsRef.current.some((s) => s.projectId === dto.id) && typeof crypto?.randomUUID === "function") {
    const prevSnapshot = { sess: sessionsRef.current, actSess: activeSessionIdRef.current, actProj: activeProjectIdRef.current };
    const sId = crypto.randomUUID();
    const initialTitle = t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới");
    setSessions((prev) => [{ id: sId, title: initialTitle, projectId: dto.id, updatedAt: justNowText }, ...prev]);
    setActiveSessionIdState(sId);
    void apiHarnessDaemon.createSession(dto.id, { id: sId, title: initialTitle }).catch((err) => {
      setSessions(prevSnapshot.sess);
      setActiveSessionIdState(prevSnapshot.actSess);
      setActiveProjectId(prevSnapshot.actProj);
      showToast({ kind: "warning", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }
  showToast({ kind: "success", message: `${t("workbench.projectCreatedToast", "Đã tạo dự án mới trong SQLite V7")}: ${dto.name}` });
}

function useLaunchpadEventSync(deps: ProjectSyncDeps) {
  useEffect(() => {
    if (typeof window === "undefined") return;
    const daemonErrFallback = deps.t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7");
    const onProjectCreated = (ev: Event) => handleLaunchpadCreatedDto(ev, deps);
    const onProjectError = (ev: Event) => {
      const msg = readEventErrorMessage(ev, daemonErrFallback);
      deps.showToast({ kind: "danger", message: deps.t("launchpad.createError", "Unable to create Launchpad project: {{message}}").replace("{{message}}", msg) });
    };
    const onFixtureError = (ev: Event) => {
      const msg = readEventErrorMessage(ev, daemonErrFallback);
      deps.showToast({ kind: "danger", message: deps.t("sampleData.saveError", "Unable to save sample fixture: {{message}}").replace("{{message}}", msg) });
    };
    window.addEventListener("cowok:project-created", onProjectCreated);
    window.addEventListener("cowok:project-create-error", onProjectError);
    window.addEventListener("cowok:sample-fixture-error", onFixtureError);
    return () => {
      window.removeEventListener("cowok:project-created", onProjectCreated);
      window.removeEventListener("cowok:project-create-error", onProjectError);
      window.removeEventListener("cowok:sample-fixture-error", onFixtureError);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deps.justNowText, deps.showToast, deps.t]);
}

function executeCreateNewProject(
  input: { name: string; title: string; provenance: "folder-backed" | "scratch"; location?: string | { localPath?: string; managedSlug?: string } },
  deps: ProjectSyncDeps,
) {
  const { justNowText, sessionsRef, activeSessionIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const prevSnapshot = { sess: sessionsRef.current, actSess: activeSessionIdRef.current };
  const { localPath, managedSlug } = resolveProjectLocationArg(input.location);
  const { newProj, newSession, slug } = buildNewProjectObjects({
    projectName: input.name, initialTitle: input.title, provenance: input.provenance, justNowText, localPath, managedSlug,
  });
  const isManaged = input.provenance === "folder-backed" && (!localPath || localPath.endsWith(`/CowokProjects/${managedSlug || slug}`));
  void apiHarnessDaemon.createProject({
    id: newProj.id, slug, managed_slug: isManaged ? (managedSlug || slug) : undefined, name: newProj.name,
    product_brief: `Created via workbench in ${input.provenance} mode`, workspace_root: input.provenance === "scratch" ? null : (localPath || null), provenance: input.provenance,
  }).then((dto) => {
    const mapped = mapDtoToWorkbenchProject(dto);
    setProjects((prev) => (prev.some((p) => p.id === dto.id) ? prev.map((p) => (p.id === dto.id ? mapped : p)) : [...prev, mapped]));
    setSessions((prev) => (prev.some((s) => s.id === newSession.id) ? prev : [newSession, ...prev]));
    setActiveSessionIdState(newSession.id);
    setActiveProjectId(dto.id);
    showToast({ kind: "success", message: `${t("workbench.projectCreatedToast", "Đã tạo dự án mới trong SQLite V7")}: ${dto.name}` });
    void apiHarnessDaemon.createSession(dto.id, { id: newSession.id, title: newSession.title }).catch((sessErr) => {
      setSessions(prevSnapshot.sess);
      setActiveSessionIdState(prevSnapshot.actSess);
      showToast({ kind: "warning", message: extractDaemonErrorMessage(sessErr, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }).catch((err) => {
    showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  });
}

function useProjectCreationActions(deps: ProjectSyncDeps) {
  const { justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const handleCreateInProject = useCallback((targetProjectId: string, title: string) => {
    const prev = { sess: sessionsRef.current, actSess: activeSessionIdRef.current, actProj: activeProjectIdRef.current };
    const sId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    setSessions((cur) => [{ id: sId, title, projectId: targetProjectId, updatedAt: justNowText }, ...cur]);
    setActiveSessionIdState(sId);
    setActiveProjectId(targetProjectId);
    void apiHarnessDaemon.createSession(targetProjectId, { id: sId, title }).catch((err) => {
      setSessions(prev.sess);
      setActiveSessionIdState(prev.actSess);
      setActiveProjectId(prev.actProj);
      showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }, [activeProjectIdRef, activeSessionIdRef, justNowText, sessionsRef, setActiveProjectId, setActiveSessionIdState, setSessions, showToast, t]);

  const handleCreateNewProject = useCallback((name: string, title: string, provenance: "folder-backed" | "scratch" = "folder-backed", location?: string | { localPath?: string; managedSlug?: string }) => {
    executeCreateNewProject({ name, title, provenance, location }, deps);
  }, [deps]);

  const handleCreateStandalone = useCallback((title: string) => {
    const sId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    setSessions((prev) => [{ id: sId, title, projectId: undefined, updatedAt: justNowText }, ...prev]);
    setActiveSessionIdState(sId);
  }, [justNowText, setActiveSessionIdState, setSessions]);

  return { handleCreateInProject, handleCreateNewProject, handleCreateStandalone };
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
