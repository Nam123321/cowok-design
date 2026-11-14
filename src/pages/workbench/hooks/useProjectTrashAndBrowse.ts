// @story 04.2
// @implements AC-03, AC-04
import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import type { ProjectBoundaryDto, ProjectDto } from "../../../lib/validations/harness-daemon";
import { DEFAULT_FLOW_PROJECT_ID, type WorkbenchProject } from "../constants";
import type { ChatSession } from "../types";

export const ACTIVE_PROJECT_HINT_KEY = "cowok.activeProjectId";
export const MANAGED_SLUG_REGEX = /^[a-z0-9][a-z0-9_-]{0,62}$/;
const MANAGED_SLUG_MAX_LEN = 63;

export interface WorkbenchToastState {
  kind: "success" | "danger" | "warning";
  message: string;
  undoProjectId?: string;
}

export interface FolderPickerModalState {
  isOpen: boolean;
  mode: "create" | "promote";
  projectId?: string;
  initialSlug?: string;
}

interface TrashedStoreSnapshot {
  items: ProjectDto[];
  restoringId: string | null;
}

let trashedSnapshot: TrashedStoreSnapshot = { items: [], restoringId: null };
const trashedListeners = new Set<() => void>();
let activeRestoreHandler: ((projectId: string) => Promise<void>) | null = null;

function emitTrashedStore(): void {
  for (const listener of trashedListeners) listener();
}

export function publishTrashedProjects(items: ProjectDto[], restoringId?: string | null): void {
  trashedSnapshot = {
    items,
    restoringId: restoringId !== undefined ? restoringId : trashedSnapshot.restoringId,
  };
  emitTrashedStore();
}

export function useTrashedProjectsSubscription(): TrashedStoreSnapshot {
  return useSyncExternalStore(
    (cb) => {
      trashedListeners.add(cb);
      return () => trashedListeners.delete(cb);
    },
    () => trashedSnapshot,
    () => trashedSnapshot
  );
}

export async function triggerRestoreFromSidebar(projectId: string): Promise<void> {
  if (activeRestoreHandler) await activeRestoreHandler(projectId);
}

export function toManagedSlug(rawName: string): string {
  const normalized = rawName
    .replace(/[\u0111\u0110]/g, "d")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, MANAGED_SLUG_MAX_LEN)
    .replace(/[-_]+$/g, "");
  return MANAGED_SLUG_REGEX.test(normalized) ? normalized : "cowok-project";
}

export function readActiveProjectHint(): string | null {
  if (typeof window === "undefined") return null;
  try {
    const val = window.localStorage.getItem(ACTIVE_PROJECT_HINT_KEY);
    return val && val.trim() ? val.trim() : null;
  } catch {
    return null;
  }
}

export function writeActiveProjectHint(projectId: string): void {
  if (typeof window === "undefined") return;
  try {
    if (!projectId.trim()) {
      window.localStorage.removeItem(ACTIVE_PROJECT_HINT_KEY);
    } else {
      window.localStorage.setItem(ACTIVE_PROJECT_HINT_KEY, projectId);
    }
  } catch {
    // Restricted storage sandbox
  }
}

export function extractDaemonErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message.trim()) return err.message;
  if (typeof err === "string" && err.trim()) return err;
  return fallback;
}

export function mapDtoToWorkbenchProject(dto: ProjectDto): WorkbenchProject {
  const isScratch =
    dto.provenance === "scratch" ||
    !dto.workspace_root ||
    dto.workspace_root.includes("/.cowok-design/projects/");
  return {
    id: dto.id,
    name: dto.name,
    isExpanded: true,
    provenance: isScratch ? "scratch" : "folder-backed",
    localPath: dto.workspace_root || `~/CowokProjects/${toManagedSlug(dto.slug || dto.name)}`,
  };
}

export type SettledProjectSessions = PromiseSettledResult<{
  projId: string;
  sessList: { id: string; title: string; project_id?: string | null; updated_at?: number | null }[];
}>;

export function collectFetchedProjectSessions(settled: SettledProjectSessions[], formatTime: (v: number) => string) {
  const fetchedByProject = new Map<string, ChatSession[]>();
  const allFetchedSessions: ChatSession[] = [];
  const seenIds = new Set<string>();
  for (const item of settled) {
    if (item.status !== "fulfilled" || !Array.isArray(item.value.sessList) || item.value.sessList.length === 0) continue;
    const mappedForProj: ChatSession[] = item.value.sessList.map((s) => ({
      id: s.id, title: s.title, projectId: s.project_id || item.value.projId, updatedAt: formatTime(s.updated_at || Date.now()),
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

export function resolveValidatedActiveProjectId(mappedProjs: WorkbenchProject[]): string {
  if (mappedProjs.length === 0) {
    writeActiveProjectHint("");
    return "";
  }
  const hint = readActiveProjectHint();
  const matched = (hint && mappedProjs.find((p) => p.id === hint)) || mappedProjs.find((p) => p.id === DEFAULT_FLOW_PROJECT_ID) || mappedProjs[0];
  writeActiveProjectHint(matched.id);
  return matched.id;
}

export function toPlaceholderTrashedDto(proj: WorkbenchProject, trashDeletedAt: number): ProjectDto {
  const now = Date.now();
  return {
    id: proj.id,
    slug: toManagedSlug(proj.name),
    name: proj.name,
    product_brief: "",
    workspace_root: proj.localPath || null,
    created_at: now,
    updated_at: now,
    provenance: proj.provenance,
    trash_deleted_at: trashDeletedAt,
  };
}

interface ActionDeps {
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setPickerState: React.Dispatch<React.SetStateAction<FolderPickerModalState>>;
  setTrashedProjects: React.Dispatch<React.SetStateAction<ProjectDto[]>>;
  setRestoringProjectId: React.Dispatch<React.SetStateAction<string | null>>;
  showToast: (next: WorkbenchToastState) => void;
  t: (key: string, fallback: string) => string;
}

function usePromoteFolderAction(deps: Pick<ActionDeps, "setProjects" | "setPickerState" | "showToast" | "t">) {
  const { setProjects, setPickerState, showToast, t } = deps;
  return useCallback(async (projectId: string, targetDirectory: string): Promise<ProjectBoundaryDto> => {
    try {
      const issued = await apiHarnessDaemon.issuePathToken(projectId, { target_path: targetDirectory });
      const canonical = issued.canonical_path || targetDirectory;
      await apiHarnessDaemon.verifyPathToken(projectId, { token: issued.token, target_directory: canonical });
      const promoted = await apiHarnessDaemon.promoteProject(projectId, {
        target_directory: canonical, token: issued.token, path_token: issued.path_token || issued.token,
      });
      setProjects((prev) => prev.map((p) => (p.id === projectId ? { ...p, provenance: "folder-backed", localPath: promoted.workspace_path } : p)));
      showToast({ kind: "success", message: t("workbench.projectPromotedToast", "Đã gắn thư mục dự án an toàn (PathToken)") });
      setPickerState((prev) => ({ ...prev, isOpen: false }));
      return promoted;
    } catch (err: unknown) {
      showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
      throw err;
    }
  }, [setPickerState, setProjects, showToast, t]);
}

function useRestoreProjectAction(
  deps: Pick<ActionDeps, "setProjects" | "setActiveProjectId" | "setTrashedProjects" | "setRestoringProjectId" | "showToast" | "t">
) {
  const { setProjects, setActiveProjectId, setTrashedProjects, setRestoringProjectId, showToast, t } = deps;
  return useCallback(async (projectId: string) => {
    setRestoringProjectId(projectId);
    try {
      const res = await apiHarnessDaemon.restoreProject(projectId);
      setTrashedProjects((prev) => prev.filter((item) => item.id !== projectId));
      setProjects((prev) => (prev.some((p) => p.id === projectId) ? prev : [...prev, mapDtoToWorkbenchProject(res.project)]));
      setActiveProjectId(projectId);
      writeActiveProjectHint(projectId);
      const suffix = res.renamed_slug ? ` (${t("workbench.projectRenamedSlugNote", "Slug đổi thành:")} ${res.renamed_slug})` : "";
      showToast({ kind: "success", message: `${t("workbench.projectRestoredToast", "Đã khôi phục dự án")}: ${res.project.name}${suffix}` });
    } catch (err: unknown) {
      showToast({ kind: "danger", message: extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")) });
    } finally {
      setRestoringProjectId(null);
    }
  }, [setActiveProjectId, setProjects, setRestoringProjectId, setTrashedProjects, showToast, t]);
}

export interface UseProjectTrashAndBrowseArgs {
  setProjects: React.Dispatch<React.SetStateAction<WorkbenchProject[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  t: (key: string, fallback: string) => string;
}

export function useProjectTrashAndBrowse({ setProjects, setActiveProjectId, t }: UseProjectTrashAndBrowseArgs) {
  const [trashedProjects, setTrashedProjects] = useState<ProjectDto[]>([]);
  const [restoringProjectId, setRestoringProjectId] = useState<string | null>(null);
  const [toastState, setToastState] = useState<WorkbenchToastState | null>(null);
  const [pickerState, setPickerState] = useState<FolderPickerModalState>({ isOpen: false, mode: "promote" });

  const showToast = useCallback((next: WorkbenchToastState) => setToastState(next), []);
  const dismissToast = useCallback(() => setToastState(null), []);
  const closeFolderPicker = useCallback(() => setPickerState((prev) => ({ ...prev, isOpen: false })), []);
  const openPromoteFolderPicker = useCallback((projectId: string, projectName?: string) => {
    setPickerState({ isOpen: true, mode: "promote", projectId, initialSlug: toManagedSlug(projectName || "cowok-project") });
  }, []);

  const syncProjectDtos = useCallback((allDtos: ProjectDto[]) => {
    const activeDtos = allDtos.filter((p) => p.trash_deleted_at == null);
    const trashedDtos = allDtos.filter((p) => p.trash_deleted_at != null);
    setTrashedProjects(trashedDtos);
    return { activeDtos, trashedDtos };
  }, []);

  const addTrashedProject = useCallback((projDto: ProjectDto) => {
    setTrashedProjects((prev) => [projDto, ...prev.filter((item) => item.id !== projDto.id)]);
  }, []);

  const handleConfirmPromote = usePromoteFolderAction({ setProjects, setPickerState, showToast, t });
  const handleRestoreProject = useRestoreProjectAction({
    setProjects, setActiveProjectId, setTrashedProjects, setRestoringProjectId, showToast, t,
  });

  useEffect(() => {
    publishTrashedProjects(trashedProjects, restoringProjectId);
  }, [restoringProjectId, trashedProjects]);

  useEffect(() => {
    activeRestoreHandler = handleRestoreProject;
    return () => {
      if (activeRestoreHandler === handleRestoreProject) activeRestoreHandler = null;
      publishTrashedProjects([], null);
    };
  }, [handleRestoreProject]);

  return {
    trashedProjects, restoringProjectId, toastState, showToast, dismissToast, pickerState,
    openPromoteFolderPicker, closeFolderPicker, syncProjectDtos, addTrashedProject,
    handleConfirmPromote, handleRestoreProject,
  };
}
