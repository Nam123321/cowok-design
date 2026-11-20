// @story 05.3
import { useCallback, useEffect, useRef, useState } from "react";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import { emitSessionForked } from "../../flow/store/shared-chat-sync";
import type { ExportInitialSection, PrSubmissionError } from "../components/ExportBundleModal";
import {
  ProjectIntegrationsStatusSchema,
  type ProjectIntegrationsStatus,
} from "../../../lib/validations/github-ide-integration";
import type {
  ChatArtifactSnapshotResponse,
  ExportZipResponse,
  GitHubPRInput,
  GitHubPRResponse,
  IdeTarget,
  OpenIdeHandoffResponse,
  VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";

const SHORT_HASH_LEN = 8;
const HTTP_CONFLICT = 409;

export function isDeletedNodeConflictError(err: unknown): boolean {
  if (!err || typeof err !== "object") return false;
  const e = err as { status?: number; code?: string; message?: string };
  return (
    e.status === HTTP_CONFLICT &&
    (e.code === "NODE_DELETED_REQUIRES_GRAPH_RESTORE" ||
      String(e.message || "").includes("NODE_DELETED_REQUIRES_GRAPH_RESTORE"))
  );
}

export function triggerBrowserBlobDownload(blob: Blob, fileName: string): void {
  if (typeof document === "undefined" || typeof URL.createObjectURL !== "function") return;
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function buildFallbackTurnSnapshot(turn: ChatArtifactSnapshotResponse): VersionSnapshotResponse {
  return {
    id: turn.id,
    tenantId: turn.tenantId,
    projectId: turn.projectId,
    versionTag: turn.contentHash.slice(0, SHORT_HASH_LEN),
    label: turn.label || turn.artifactType,
    description: "",
    trigger: "auto_proposal_apply",
    treeSha256: turn.contentHash,
    files: [],
    createdAt: String(turn.createdAt),
  };
}

function useSnapshotListsState(projectId: string | undefined) {
  const [snapshots, setSnapshots] = useState<VersionSnapshotResponse[]>([]);
  const [turnSnapshots, setTurnSnapshots] = useState<ChatArtifactSnapshotResponse[]>([]);
  const [listError, setListError] = useState<string | null>(null);

  const refreshData = useCallback(async () => {
    if (!projectId) return;
    const [snaps, turns] = await Promise.all([
      apiVersionSnapshot.getSnapshots(projectId),
      apiVersionSnapshot.listTurnSnapshots(projectId),
    ]);
    setSnapshots(snaps);
    setTurnSnapshots(turns);
    setListError(null);
  }, [projectId]);

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    async function loadInitialLists() {
      try {
        const [snaps, turns] = await Promise.all([
          apiVersionSnapshot.getSnapshots(projectId!),
          apiVersionSnapshot.listTurnSnapshots(projectId!),
        ]);
        if (!cancelled) { setSnapshots(snaps); setTurnSnapshots(turns); setListError(null); }
      } catch (err) {
        if (!cancelled) setListError(err instanceof Error ? err.message : "Failed to load snapshots");
      }
    }
    void loadInitialLists();
    return () => { cancelled = true; };
  }, [projectId]);

  return { snapshots, turnSnapshots, listError, refreshData };
}

function useIntegrationsState(projectId: string | undefined) {
  const [integrationsStatus, setIntegrationsStatus] = useState<ProjectIntegrationsStatus | null>(null);
  const [integrationBusy, setIntegrationBusy] = useState(false);

  const refreshIntegrations = useCallback(async () => {
    if (!projectId) return;
    try { setIntegrationsStatus(await apiVersionSnapshot.getIntegrationsStatus(projectId)); }
    catch { /* daemon may be offline in unit tests */ }
  }, [projectId]);

  useEffect(() => { void refreshIntegrations(); }, [refreshIntegrations]);

  const mutate = async <T,>(task: () => Promise<T>) => {
    setIntegrationBusy(true);
    try {
      const next = await task();
      const parsed = ProjectIntegrationsStatusSchema.safeParse(next);
      if (parsed.success) {
        setIntegrationsStatus(parsed.data);
      } else { await refreshIntegrations(); }
    } finally { setIntegrationBusy(false); }
  };

  return {
    integrationsStatus,
    integrationBusy,
    refreshIntegrations,
    connectGitHub: (input: { token?: string; useGhCli?: boolean }) =>
      projectId ? mutate(() => apiVersionSnapshot.connectGitHubAccount(projectId, input)) : Promise.resolve(),
    selectGitHubAccount: (login: string) =>
      projectId ? mutate(() => apiVersionSnapshot.selectGitHubAccount(projectId, login)) : Promise.resolve(),
    bindGitRemote: (remoteUrl: string) =>
      projectId ? mutate(() => apiVersionSnapshot.bindGitRemote(projectId, remoteUrl)) : Promise.resolve(),
    setDefaultIde: (ide: IdeTarget) =>
      projectId ? mutate(() => apiVersionSnapshot.setDefaultIde(projectId, ide)) : Promise.resolve(),
  };
}

function useExportActionsState(projectId: string | undefined, snapshotsCount: number, refreshIntegrations: () => Promise<void>) {
  const [isExportOpen, setIsExportOpen] = useState(false);
  const [exportInitialSection, setExportInitialSection] = useState<ExportInitialSection>("zip");
  const [exportMetadata, setExportMetadata] = useState<ExportZipResponse | null>(null);
  const [prResult, setPrResult] = useState<GitHubPRResponse | null>(null);
  const [prError, setPrError] = useState<PrSubmissionError | null>(null);
  const [handoffResult, setHandoffResult] = useState<OpenIdeHandoffResponse | null>(null);
  const [handoffError, setHandoffError] = useState<string | null>(null);

  const handleOpenExport = useCallback(async (section: ExportInitialSection = "zip") => {
    setExportInitialSection(section);
    setIsExportOpen(true);
    void refreshIntegrations();
    if (!projectId || snapshotsCount === 0) return;
    try { setExportMetadata(await apiVersionSnapshot.exportBundle(projectId)); } catch { /* ignore empty */ }
  }, [projectId, refreshIntegrations, snapshotsCount]);

  const handleDownloadZip = async () => {
    if (!projectId) return;
    setExportMetadata(await apiVersionSnapshot.exportBundle(projectId));
    const { blob, fileName } = await apiVersionSnapshot.downloadBundle(projectId);
    triggerBrowserBlobDownload(blob, fileName);
  };

  const handleCreatePR = async (inputOrBranch: GitHubPRInput | string, title?: string) => {
    if (!projectId) return;
    setPrError(null);
    setPrResult(null);
    const payload = typeof inputOrBranch === "string"
      ? { branchName: inputOrBranch, prTitle: title || "Sync executable UI design intent" }
      : inputOrBranch;
    try { setPrResult(await apiVersionSnapshot.createGitHubPR(projectId, payload)); }
    catch (err) {
      const e = err as { status?: number; code?: string; message?: string };
      setPrError({ status: e.status, code: e.code, message: e.message || "GitHub PR creation failed" });
    }
  };

  const handleOpenIde = async (ide: IdeTarget) => {
    if (!projectId) return;
    setHandoffError(null);
    try { setHandoffResult(await apiVersionSnapshot.openIdeHandoff(projectId, { ide })); }
    catch (err) { setHandoffError(err instanceof Error ? err.message : "Failed to launch IDE handoff"); }
  };

  return { isExportOpen, setIsExportOpen, exportInitialSection, exportMetadata, prResult, prError, handoffResult, handoffError, handleOpenExport, handleDownloadZip, handleCreatePR, handleOpenIde };
}

function dispatchTurnForkNavigation(
  projectId: string,
  turn: ChatArtifactSnapshotResponse,
  res: { forkedSessionId: string; forkedFromMessageId: string; detachedNodeIds: string[]; copiedMessagesCount: number },
  onAfterFork?: () => void
) {
  const parsedTs = typeof turn.createdAt === "number" ? turn.createdAt : Date.parse(String(turn.createdAt));
  onAfterFork?.();
  emitSessionForked({
    projectId,
    sourceSessionId: turn.sessionId,
    forkedSessionId: res.forkedSessionId,
    cutoffMessageId: res.forkedFromMessageId,
    cutoffCreatedAtMs: Number.isFinite(parsedTs) ? parsedTs : undefined,
    detachedNodeIds: res.detachedNodeIds,
    copiedMessagesCount: res.copiedMessagesCount,
  });
}

function emitSnapshotEvent(name: "cowok:project-state-invalidated" | "cowok:session-fork-error", detail: Record<string, unknown>): void {
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(name, { detail }));
}

async function runRevertSnapshotPass(projectId: string, snapId: string, restoreMissingNodes: boolean, refreshData: () => Promise<void>): Promise<void> {
  await apiVersionSnapshot.revertSnapshot(projectId, snapId, { restoreMissingNodes });
  await refreshData();
  emitSnapshotEvent("cowok:project-state-invalidated", { projectId, reason: "revert" });
}

function useTurnReceiptsState(
  projectId: string | undefined,
  refreshData: () => Promise<void>,
  onPreviewStateChange?: (turn: ChatArtifactSnapshotResponse | null) => void,
  onAfterFork?: () => void
) {
  const [activePreviewTurn, setActivePreviewTurn] = useState<ChatArtifactSnapshotResponse | null>(null);
  const [forkedSessionByTurnId, setForkedSessionByTurnId] = useState<Record<string, { sessionId: string; detachedNodeIds: string[] }>>({});
  const [turnRevertTarget, setTurnRevertTarget] = useState<VersionSnapshotResponse | null>(null);
  const [turnRestoreMissingNodes, setTurnRestoreMissingNodes] = useState(false);
  const [isTurnReverting, setIsTurnReverting] = useState(false);
  const forkBusyRef = useRef(false);
  const handlePreviewTurn = (turn: ChatArtifactSnapshotResponse) => {
    const next = activePreviewTurn?.id === turn.id ? null : turn;
    setActivePreviewTurn(next);
    onPreviewStateChange?.(next);
  };
  const handleForkTurn = async (turn: ChatArtifactSnapshotResponse) => {
    if (!projectId || forkBusyRef.current) return;
    forkBusyRef.current = true;
    try {
      const res = await apiVersionSnapshot.forkSessionFromTurn(projectId, turn.id, { includeNodeScope: true });
      setForkedSessionByTurnId((prev) => ({ ...prev, [turn.id]: { sessionId: res.forkedSessionId, detachedNodeIds: res.detachedNodeIds } }));
      if (activePreviewTurn?.id === turn.id) { setActivePreviewTurn(null); onPreviewStateChange?.(null); }
      dispatchTurnForkNavigation(projectId, turn, res, onAfterFork);
    } catch (err) {
      emitSnapshotEvent("cowok:session-fork-error", { message: err instanceof Error ? err.message : "Unable to fork from snapshot" });
    } finally { forkBusyRef.current = false; }
  };
  const handleUndoTurn = async (turn: ChatArtifactSnapshotResponse) => {
    if (!projectId) return;
    try { await runRevertSnapshotPass(projectId, turn.id, false, refreshData); }
    catch (err) {
      if (isDeletedNodeConflictError(err)) { setTurnRevertTarget(buildFallbackTurnSnapshot(turn)); setTurnRestoreMissingNodes(false); }
      else emitSnapshotEvent("cowok:session-fork-error", { message: err instanceof Error ? err.message : "Revert snapshot failed" });
    }
  };
  const handleConfirmTurnRevert = async () => {
    if (!projectId || !turnRevertTarget) return;
    setIsTurnReverting(true);
    try { await runRevertSnapshotPass(projectId, turnRevertTarget.id, turnRestoreMissingNodes, refreshData); setTurnRevertTarget(null); }
    catch (err) { emitSnapshotEvent("cowok:session-fork-error", { message: err instanceof Error ? err.message : "Revert snapshot failed" }); }
    finally { setIsTurnReverting(false); }
  };
  return { activePreviewTurn, forkedSessionByTurnId, turnRevertTarget, setTurnRevertTarget, turnRestoreMissingNodes, setTurnRestoreMissingNodes, isTurnReverting, handlePreviewTurn, handleForkTurn, handleUndoTurn, handleConfirmTurnRevert };
}

export function useSnapshotWorkspaceController(
  projectId: string | undefined,
  onPreviewStateChange?: (previewTurn: ChatArtifactSnapshotResponse | null) => void
) {
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const lists = useSnapshotListsState(projectId);
  const integrations = useIntegrationsState(projectId);
  const exp = useExportActionsState(projectId, lists.snapshots.length, integrations.refreshIntegrations);
  const turn = useTurnReceiptsState(projectId, lists.refreshData, onPreviewStateChange, () => setIsDrawerOpen(false));

  useEffect(() => {
    if (isDrawerOpen) void lists.refreshData();
  }, [isDrawerOpen, lists.refreshData]);

  const createManualCheckpoint = useCallback(async () => {
    if (!projectId) return;
    try {
      await apiVersionSnapshot.createSnapshot(projectId, {
        label: "Manual Workspace Checkpoint",
        trigger: "manual",
        files: [],
      });
      await lists.refreshData();
    } catch (err) {
      emitSnapshotEvent("cowok:session-fork-error", {
        message: err instanceof Error ? err.message : "Unable to create workspace checkpoint",
      });
    }
  }, [lists, projectId]);

  return { ...lists, ...integrations, isDrawerOpen, setIsDrawerOpen, createManualCheckpoint, exp, turn };
}
