// @story 04.1, 04.2
// @implements AC-03, AC-04 (Project creation and Launchpad / Fork event synchronization)
import { useCallback, useEffect } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import { ProjectDtoSchema } from "../../../lib/validations/harness-daemon";
import type { WorkbenchProject } from "../constants";
import { SESSION_FORKED_EVENT } from "../../../features/flow/store/shared-chat-sync";
import type { ChatSession, PostCreateProjectOptions } from "../types";
import { writeStandaloneSessions } from "./usePinnedSessionsStorage";
import {
  extractDaemonErrorMessage,
  mapDtoToWorkbenchProject,
  toManagedSlug,
  type WorkbenchToastState,
} from "./useProjectTrashAndBrowse";
import { handleSessionForkedEvent } from "./useSessionForkSync";

export interface ProjectSyncDeps {
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

export type CreateProjectArgs = [
  name: string,
  title: string,
  provenance?: "folder-backed" | "scratch",
  location?: string | { localPath?: string; managedSlug?: string },
  entryKind?: "fe-dag" | "presentation",
  options?: PostCreateProjectOptions,
];

function resolveProjectLocationArg(location?: string | { localPath?: string; managedSlug?: string }) {
  if (!location) return {};
  return typeof location === "string" ? { localPath: location } : location;
}

function generateSafeUuidV4(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") return crypto.randomUUID();
  if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    bytes[6] = (bytes[6]! & 0x0f) | 0x40;
    bytes[8] = (bytes[8]! & 0x3f) | 0x80;
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  return `id-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
}

function buildNewProjectObjects(input: {
  projectName: string;
  initialTitle: string;
  provenance: "folder-backed" | "scratch";
  justNowText: string;
  localPath?: string;
  managedSlug?: string;
}): { newProj: WorkbenchProject; newSession: ChatSession; slug: string } {
  const newProjId = generateSafeUuidV4();
  const newSessionId = generateSafeUuidV4();
  const slug = toManagedSlug(input.managedSlug || input.projectName);
  const resolvedPath = input.localPath || (input.provenance === "scratch" ? `~/.cowok-design/projects/${slug}` : `~/CowokProjects/${slug}`);
  return {
    slug,
    newProj: { id: newProjId, name: input.projectName, isExpanded: true, provenance: input.provenance, localPath: resolvedPath },
    newSession: { id: newSessionId, title: input.initialTitle, projectId: newProjId, updatedAt: input.justNowText },
  };
}

function readEventErrorMessage(ev: Event, fallback: string): string {
  const detail = (ev as CustomEvent<{ message?: unknown }>).detail;
  return typeof detail?.message === "string" && detail.message.trim() ? detail.message : fallback;
}

function handleLaunchpadCreatedDto(ev: Event, deps: ProjectSyncDeps) {
  const { justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const rawDetail = (ev as CustomEvent<Record<string, unknown>>).detail;
  const parsed = ProjectDtoSchema.safeParse(rawDetail);
  if (!parsed.success) return;
  const entryKind: "fe-dag" | "presentation" = rawDetail?.entryKind === "presentation" ? "presentation" : "fe-dag";
  const dto = parsed.data;
  const mapped = mapDtoToWorkbenchProject(dto);
  setProjects((prev) => (prev.some((p) => p.id === dto.id) ? prev.map((p) => (p.id === dto.id ? mapped : p)) : [...prev, mapped]));
  setActiveProjectId(dto.id);
  if (!sessionsRef.current.some((s) => s.projectId === dto.id) && typeof crypto?.randomUUID === "function") {
    const prevSnapshot = { sess: sessionsRef.current, actSess: activeSessionIdRef.current, actProj: activeProjectIdRef.current };
    const sId = crypto.randomUUID();
    const initialTitle = t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới");
    setSessions((prev) => [{ id: sId, title: initialTitle, projectId: dto.id, entryKind, updatedAt: justNowText }, ...prev]);
    setActiveSessionIdState(sId);
    void apiHarnessDaemon.createSession(dto.id, { id: sId, title: initialTitle, entry_kind: entryKind }).catch((err) => {
      setProjects((prev) => prev.filter((p) => p.id !== dto.id));
      setSessions(prevSnapshot.sess);
      setActiveSessionIdState(prevSnapshot.actSess);
      setActiveProjectId(prevSnapshot.actProj);
      void apiHarnessDaemon.deleteProject(dto.id).catch(() => {});
      showToast({ kind: "warning", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }
  showToast({ kind: "success", message: `${t("workbench.projectCreatedToast", "Đã tạo dự án mới trong SQLite V7")}: ${dto.name}` });
}

export function useLaunchpadEventSync(deps: ProjectSyncDeps) {
  useEffect(() => {
    if (typeof window === "undefined") return;
    const daemonErrFallback = deps.t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7");
    const onProjectCreated = (ev: Event) => handleLaunchpadCreatedDto(ev, deps);
    const onSessionForked = (ev: Event) => handleSessionForkedEvent(ev, deps);
    const onProjectError = (ev: Event) => {
      const msg = readEventErrorMessage(ev, daemonErrFallback);
      deps.showToast({ kind: "danger", message: deps.t("launchpad.createError", "Unable to create Launchpad project: {{message}}").replace("{{message}}", msg) });
    };
    const onForkError = (ev: Event) => {
      const msg = readEventErrorMessage(ev, daemonErrFallback);
      deps.showToast({ kind: "danger", message: msg });
    };
    const onFixtureError = (ev: Event) => {
      const msg = readEventErrorMessage(ev, daemonErrFallback);
      deps.showToast({ kind: "danger", message: deps.t("sampleData.saveError", "Unable to save sample fixture: {{message}}").replace("{{message}}", msg) });
    };
    window.addEventListener("cowok:project-created", onProjectCreated);
    window.addEventListener(SESSION_FORKED_EVENT, onSessionForked);
    window.addEventListener("cowok:project-create-error", onProjectError);
    window.addEventListener("cowok:session-fork-error", onForkError);
    window.addEventListener("cowok:sample-fixture-error", onFixtureError);
    return () => {
      window.removeEventListener("cowok:project-created", onProjectCreated);
      window.removeEventListener(SESSION_FORKED_EVENT, onSessionForked);
      window.removeEventListener("cowok:project-create-error", onProjectError);
      window.removeEventListener("cowok:session-fork-error", onForkError);
      window.removeEventListener("cowok:sample-fixture-error", onFixtureError);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deps.justNowText, deps.showToast, deps.t]);
}

function postCreateProjectFollowups(args: {
  dto: { id: string; name: string };
  enrichedSession: ChatSession;
  resolvedEntryKind: "fe-dag" | "presentation";
  options?: PostCreateProjectOptions;
  prevSnapshot: { sess: ChatSession[]; actSess: string; actProj: string };
  deps: ProjectSyncDeps;
}) {
  const { dto, enrichedSession, resolvedEntryKind, options, prevSnapshot, deps } = args;
  const { setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  if (options?.githubRepoUrl) {
    void apiVersionSnapshot.bindGitRemote(dto.id, options.githubRepoUrl).catch((gitErr) => {
      showToast({ kind: "warning", message: extractDaemonErrorMessage(gitErr, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }
  void apiHarnessDaemon
    .createSession(dto.id, { id: enrichedSession.id, title: enrichedSession.title, entry_kind: resolvedEntryKind })
    .then(() => {
      if (options?.openMaterialLaunchpad && typeof window !== "undefined") {
        window.dispatchEvent(new CustomEvent("cowok:open-launchpad", {
          detail: { mode: "post-create-material", projectId: dto.id, projectName: dto.name, entryKind: resolvedEntryKind },
        }));
      }
    })
    .catch((sessErr) => {
      setProjects((prev) => prev.filter((p) => p.id !== dto.id));
      setSessions(prevSnapshot.sess);
      setActiveSessionIdState(prevSnapshot.actSess);
      setActiveProjectId(prevSnapshot.actProj);
      void apiHarnessDaemon.deleteProject(dto.id).catch(() => {});
      showToast({ kind: "warning", message: extractDaemonErrorMessage(sessErr, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
}

function executeCreateNewProject(
  input: {
    name: string;
    title: string;
    provenance: "folder-backed" | "scratch";
    location?: string | { localPath?: string; managedSlug?: string };
    entryKind?: "fe-dag" | "presentation";
    options?: PostCreateProjectOptions;
  },
  deps: ProjectSyncDeps
) {
  const { justNowText, sessionsRef, activeSessionIdRef, setProjects, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const prevSnapshot = { sess: sessionsRef.current, actSess: activeSessionIdRef.current, actProj: deps.activeProjectIdRef.current };
  const { localPath, managedSlug } = resolveProjectLocationArg(input.location);
  const { newProj, newSession, slug } = buildNewProjectObjects({
    projectName: input.name, initialTitle: input.title, provenance: input.provenance, justNowText, localPath, managedSlug,
  });
  const resolvedEntryKind = input.entryKind ?? ("fe-dag" as const);
  const enrichedSession = { ...newSession, entryKind: resolvedEntryKind };
  const isManaged = input.provenance === "folder-backed" && (!localPath || localPath.endsWith(`/CowokProjects/${managedSlug || slug}`));
  void apiHarnessDaemon.createProject({
    id: newProj.id, slug, managed_slug: isManaged ? (managedSlug || slug) : undefined, name: newProj.name,
    product_brief: `Created via workbench in ${input.provenance} mode (${resolvedEntryKind})`, workspace_root: input.provenance === "scratch" ? null : (localPath || null), provenance: input.provenance,
  }).then((dto) => {
    const mapped = mapDtoToWorkbenchProject(dto);
    setProjects((prev) => (prev.some((p) => p.id === dto.id) ? prev.map((p) => (p.id === dto.id ? mapped : p)) : [...prev, mapped]));
    setSessions((prev) => (prev.some((s) => s.id === enrichedSession.id) ? prev : [enrichedSession, ...prev]));
    setActiveSessionIdState(enrichedSession.id);
    setActiveProjectId(dto.id);
    showToast({ kind: "success", message: `${t("workbench.projectCreatedToast", "Đã tạo dự án mới trong SQLite V7")}: ${dto.name}` });
    postCreateProjectFollowups({ dto, enrichedSession, resolvedEntryKind, options: input.options, prevSnapshot, deps });
  }).catch((err) => {
    showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
  });
}

export function useProjectCreationActions(deps: ProjectSyncDeps) {
  const { justNowText, sessionsRef, activeSessionIdRef, activeProjectIdRef, setSessions, setActiveProjectId, setActiveSessionIdState, showToast, t } = deps;
  const handleCreateInProject = useCallback((targetProjectId: string, title: string, entryKind: "fe-dag" | "presentation" = "fe-dag") => {
    const prev = { sess: sessionsRef.current, actSess: activeSessionIdRef.current, actProj: activeProjectIdRef.current };
    const sId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    setSessions((cur) => [{ id: sId, title, projectId: targetProjectId, entryKind, updatedAt: justNowText }, ...cur]);
    setActiveSessionIdState(sId);
    setActiveProjectId(targetProjectId);
    void apiHarnessDaemon.createSession(targetProjectId, { id: sId, title, entry_kind: entryKind }).catch((err) => {
      setSessions(prev.sess);
      setActiveSessionIdState(prev.actSess);
      setActiveProjectId(prev.actProj);
      showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    });
  }, [activeProjectIdRef, activeSessionIdRef, justNowText, sessionsRef, setActiveProjectId, setActiveSessionIdState, setSessions, showToast, t]);

  const handleCreateNewProject = useCallback((...args: CreateProjectArgs) => {
    const [name, title, provenance = "folder-backed", location, entryKind = "fe-dag", options] = args;
    executeCreateNewProject({ name, title, provenance, location, entryKind, options }, deps);
  }, [deps]);

  const handleCreateStandalone = useCallback((title: string, entryKind: "fe-dag" | "presentation" = "fe-dag") => {
    const sId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `session-${Date.now()}`;
    setSessions((prev) => {
      const next = [{ id: sId, title, projectId: undefined, entryKind, updatedAt: justNowText }, ...prev];
      writeStandaloneSessions(next);
      return next;
    });
    setActiveSessionIdState(sId);
  }, [justNowText, setActiveSessionIdState, setSessions]);

  return { handleCreateInProject, handleCreateNewProject, handleCreateStandalone };
}
