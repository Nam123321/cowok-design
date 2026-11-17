// @story 05.3
import React, { useState } from "react";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectIntegrationsStatus } from "../../../lib/validations/github-ide-integration";
import {
  type ChatArtifactSnapshotResponse,
  type ExportZipResponse,
  type GitHubPRInput,
  type GitHubPRResponse,
  type IdeTarget,
  type OpenIdeHandoffResponse,
  type RevertSnapshotInput,
  type VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";
import {
  isDeletedNodeConflictError,
  useSnapshotWorkspaceController,
} from "../hooks/useSnapshotWorkspaceController";
import { DrawerSidebar } from "./DrawerSidebar";
import {
  ExportBundleModal,
  type ExportInitialSection,
  type PrSubmissionError,
} from "./ExportBundleModal";
import { RevertConfirmModal } from "./RevertConfirmModal";
import {
  type SelectedHistoryEntry,
  SnapshotDetailInspector,
} from "./SnapshotDetailInspector";
import { TurnSnapshotReceiptCard } from "./TurnSnapshotReceiptCard";
import { VersionPublishMenuButton } from "./VersionPublishMenuButton";

export { VersionPublishMenuButton, useSnapshotWorkspaceController };

const SHORT_HASH_LEN = 8;

export interface VersionHistoryDrawerProps {
  isOpen: boolean;
  snapshots: VersionSnapshotResponse[];
  turnSnapshots?: ChatArtifactSnapshotResponse[];
  artifactFormat?: "dag" | "slide-cms" | "md" | "assets" | "code";
  activePreviewTurnId?: string | null;
  forkedSessionByTurnId?: Record<string, { sessionId: string; detachedNodeIds: string[] }>;
  integrationsStatus?: ProjectIntegrationsStatus | null;
  integrationBusy?: boolean;
  exportInitialSection?: ExportInitialSection;
  exportMetadata?: ExportZipResponse | null;
  prResult?: GitHubPRResponse | null;
  prError?: PrSubmissionError | null;
  handoffResult?: OpenIdeHandoffResponse | null;
  handoffError?: string | null;
  isExportOpen?: boolean;
  onSetExportOpen?: (open: boolean) => void;
  onClose: () => void;
  onRevert: (snapshotId: string, input?: RevertSnapshotInput) => Promise<void>;
  onCreateCheckpoint?: () => Promise<void> | void;
  onPreviewTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onUndoTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onForkTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onExportZip: () => Promise<void>;
  onCreatePR: (inputOrBranch: GitHubPRInput | string, title?: string) => Promise<void>;
  onOpenIde?: (ide: IdeTarget) => Promise<void>;
  onConnectGitHub?: (input: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onSelectGitHubAccount?: (login: string) => Promise<void>;
  onBindGitRemote?: (remoteUrl: string) => Promise<void>;
  onSetDefaultIde?: (ide: IdeTarget) => Promise<void> | void;
}

function resolveDefaultHistoryEntry(
  snapshots: VersionSnapshotResponse[],
  turnSnapshots?: ChatArtifactSnapshotResponse[],
  activePreviewTurnId?: string | null
): SelectedHistoryEntry | null {
  if (activePreviewTurnId && turnSnapshots?.length) {
    const activeTurn = turnSnapshots.find((item) => item.id === activePreviewTurnId);
    if (activeTurn) return { kind: "turn", item: activeTurn };
  }
  if (snapshots.length > 0) return { kind: "snapshot", item: snapshots[0] };
  if (turnSnapshots && turnSnapshots.length > 0) return { kind: "turn", item: turnSnapshots[0] };
  return null;
}

const EmptyDetailPlaceholder: React.FC = () => {
  const { t } = useTranslation();
  return (
    <section className="snap-detail-pane" data-testid="version-detail-pane">
      <div className="snap-drawer-empty" style={{ margin: "auto", maxWidth: "320px", textAlign: "center" }}>
        <span>{t("snapshots.emptyDetailPlaceholder", "Chọn một bản chụp hoặc tạo Snapshot đầu tiên để xem trước HTML / Userflow / Danh sách tệp.")}</span>
      </div>
    </section>
  );
};

const DrawerOverlayHost: React.FC<{
  props: VersionHistoryDrawerProps;
  selected: SelectedHistoryEntry | null;
  detailCollapsed: boolean;
  onInspectSnapshot: (s: VersionSnapshotResponse) => void;
  onInspectTurn: (t: ChatArtifactSnapshotResponse) => void;
  onClearDetail: () => void;
  onSelectSnap: (snap: VersionSnapshotResponse) => void;
  onOpenExport: () => void;
}> = ({
  props,
  selected,
  detailCollapsed,
  onInspectSnapshot,
  onInspectTurn,
  onClearDetail,
  onSelectSnap,
  onOpenExport,
}) => {
  if (!props.isOpen) return null;
  const effectiveSelected =
    detailCollapsed
      ? null
      : (selected ?? resolveDefaultHistoryEntry(props.snapshots, props.turnSnapshots, props.activePreviewTurnId));
  const selectedKey = effectiveSelected
    ? `${effectiveSelected.kind === "snapshot" ? "snap" : "turn"}:${effectiveSelected.item.id}`
    : null;
  const detailNode = detailCollapsed ? null : effectiveSelected ? (
    <SnapshotDetailInspector
      selected={effectiveSelected}
      artifactFormat={props.artifactFormat}
      onCloseDetail={onClearDetail}
      onRequestRevertSnapshot={onSelectSnap}
      onPreviewTurn={props.onPreviewTurn}
      onForkTurn={props.onForkTurn}
    />
  ) : (
    <EmptyDetailPlaceholder />
  );

  return (
    <>
      <div className="snap-drawer-backdrop" onClick={props.onClose} aria-hidden="true" />
      <DrawerSidebar
        snapshots={props.snapshots}
        turnSnapshots={props.turnSnapshots}
        activePreviewTurnId={props.activePreviewTurnId}
        forkedSessionByTurnId={props.forkedSessionByTurnId}
        selectedEntryKey={selectedKey}
        detailNode={detailNode}
        onClose={props.onClose}
        onSelect={onSelectSnap}
        onInspectSnapshot={onInspectSnapshot}
        onInspectTurn={onInspectTurn}
        onOpenExport={onOpenExport}
        onCreateCheckpoint={props.onCreateCheckpoint}
        onPreviewTurn={(turn) => {
          onInspectTurn(turn);
          props.onPreviewTurn?.(turn);
        }}
        onUndoTurn={props.onUndoTurn}
        onForkTurn={props.onForkTurn}
      />
    </>
  );
};

function useDrawerRevertModalState(onRevert: VersionHistoryDrawerProps["onRevert"]) {
  const [target, setTarget] = useState<VersionSnapshotResponse | null>(null);
  const [isReverting, setIsReverting] = useState(false);
  const [requiresGraphRestore, setRequiresGraphRestore] = useState(false);
  const [restoreMissingNodes, setRestoreMissingNodes] = useState(false);
  const [revertError, setRevertError] = useState<string | null>(null);

  const resetRevert = () => {
    setTarget(null);
    setRequiresGraphRestore(false);
    setRestoreMissingNodes(false);
    setRevertError(null);
  };

  const handleRevert = async () => {
    if (!target) return;
    setIsReverting(true);
    setRevertError(null);
    try { await onRevert(target.id, { restoreMissingNodes }); resetRevert(); }
    catch (err) {
      if (isDeletedNodeConflictError(err)) setRequiresGraphRestore(true);
      else setRevertError(err instanceof Error ? err.message : "Failed to restore snapshot");
    } finally { setIsReverting(false); }
  };

  return { target, setTarget, isReverting, requiresGraphRestore, restoreMissingNodes, setRestoreMissingNodes, revertError, resetRevert, handleRevert };
}

export const VersionHistoryDrawer: React.FC<VersionHistoryDrawerProps> = (props) => {
  const rev = useDrawerRevertModalState(props.onRevert);
  const [selected, setSelected] = useState<SelectedHistoryEntry | null>(null);
  const [detailCollapsed, setDetailCollapsed] = useState(false);
  const [internalExportOpen, setInternalExportOpen] = useState(false);
  const [isExporting, setIsExporting] = useState(false);

  const exportOpen = props.isExportOpen ?? internalExportOpen;
  const setExportOpen = props.onSetExportOpen ?? setInternalExportOpen;
  if (!props.isOpen && !exportOpen) return null;

  const withBusy = async (task: () => Promise<void>) => { setIsExporting(true); try { await task(); } finally { setIsExporting(false); } };

  return (
    <>
      <DrawerOverlayHost
        props={props}
        selected={selected}
        detailCollapsed={detailCollapsed}
        onInspectSnapshot={(s) => { setDetailCollapsed(false); setSelected({ kind: "snapshot", item: s }); }}
        onInspectTurn={(t) => { setDetailCollapsed(false); setSelected({ kind: "turn", item: t }); }}
        onClearDetail={() => { setDetailCollapsed(true); setSelected(null); }}
        onSelectSnap={(s) => { rev.resetRevert(); rev.setTarget(s); }}
        onOpenExport={() => setExportOpen(true)}
      />
      <RevertConfirmModal isOpen={Boolean(rev.target)} targetSnapshot={rev.target} isReverting={rev.isReverting} requiresGraphRestore={rev.requiresGraphRestore} restoreMissingNodes={rev.restoreMissingNodes} errorMessage={rev.revertError} onToggleRestoreMissingNodes={rev.setRestoreMissingNodes} onConfirm={() => void rev.handleRevert()} onClose={rev.resetRevert} />
      <ExportBundleModal isOpen={exportOpen} metadata={props.exportMetadata ?? null} isExporting={isExporting} initialSection={props.exportInitialSection} integrationsStatus={props.integrationsStatus} integrationBusy={props.integrationBusy} prResult={props.prResult} prError={props.prError} handoffResult={props.handoffResult} handoffError={props.handoffError} onDownloadZip={() => void withBusy(props.onExportZip)} onCreateGitHubPR={(inVal) => void withBusy(() => props.onCreatePR(inVal))} onOpenIde={props.onOpenIde ? (ide) => void withBusy(() => props.onOpenIde!(ide)) : undefined} onConnectGitHub={props.onConnectGitHub} onSelectGitHubAccount={props.onSelectGitHubAccount} onBindGitRemote={props.onBindGitRemote} onSetDefaultIde={props.onSetDefaultIde} onClose={() => setExportOpen(false)} />
    </>
  );
};

export const HistoricalPreviewBanner: React.FC<{
  activePreviewTurn: ChatArtifactSnapshotResponse | null;
  onExit: (turn: ChatArtifactSnapshotResponse) => void;
}> = ({ activePreviewTurn, onExit }) => {
  const { t } = useTranslation();
  if (!activePreviewTurn) return null;
  return (
    <div className="snap-historical-banner" data-testid="historical-preview-readonly-banner">
      <span>
        {t("snapshots.historicalReadOnlyBanner", "Historical Preview (Read-Only):")}{" "}
        <code>{activePreviewTurn.contentHash.slice(0, SHORT_HASH_LEN)}</code>
      </span>
      <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" data-testid="exit-historical-preview-btn" onClick={() => onExit(activePreviewTurn)}>
        {t("snapshots.exitPreviewBtn", "Exit Preview")}
      </button>
    </div>
  );
};

export const CanvasSnapshotDrawerAndModalHost: React.FC<{
  projectId?: string;
  artifactFormat?: "dag" | "slide-cms" | "md" | "assets" | "code";
  controller: ReturnType<typeof useSnapshotWorkspaceController>;
}> = ({ projectId, artifactFormat, controller }) => (
  <>
    <VersionHistoryDrawer
      isOpen={controller.isDrawerOpen}
      snapshots={controller.snapshots}
      turnSnapshots={controller.turnSnapshots}
      artifactFormat={artifactFormat}
      activePreviewTurnId={controller.turn.activePreviewTurn?.id ?? null}
      forkedSessionByTurnId={controller.turn.forkedSessionByTurnId}
      integrationsStatus={controller.integrationsStatus}
      integrationBusy={controller.integrationBusy}
      exportInitialSection={controller.exp.exportInitialSection}
      exportMetadata={controller.exp.exportMetadata}
      prResult={controller.exp.prResult}
      prError={controller.exp.prError}
      handoffResult={controller.exp.handoffResult}
      handoffError={controller.exp.handoffError}
      isExportOpen={controller.exp.isExportOpen}
      onSetExportOpen={controller.exp.setIsExportOpen}
      onClose={() => controller.setIsDrawerOpen(false)}
      onRevert={async (snapId, input) => { if (!projectId) return; await apiVersionSnapshot.revertSnapshot(projectId, snapId, input); await controller.refreshData(); }}
      onCreateCheckpoint={controller.createManualCheckpoint}
      onPreviewTurn={controller.turn.handlePreviewTurn}
      onUndoTurn={(tSnap) => void controller.turn.handleUndoTurn(tSnap)}
      onForkTurn={(tSnap) => void controller.turn.handleForkTurn(tSnap)}
      onExportZip={controller.exp.handleDownloadZip}
      onCreatePR={controller.exp.handleCreatePR}
      onOpenIde={controller.exp.handleOpenIde}
      onConnectGitHub={controller.connectGitHub}
      onSelectGitHubAccount={controller.selectGitHubAccount}
      onBindGitRemote={controller.bindGitRemote}
      onSetDefaultIde={controller.setDefaultIde}
    />
    <RevertConfirmModal
      isOpen={Boolean(controller.turn.turnRevertTarget)}
      targetSnapshot={controller.turn.turnRevertTarget}
      isReverting={controller.turn.isTurnReverting}
      requiresGraphRestore={Boolean(controller.turn.turnRevertTarget)}
      restoreMissingNodes={controller.turn.turnRestoreMissingNodes}
      onToggleRestoreMissingNodes={controller.turn.setTurnRestoreMissingNodes}
      onConfirm={() => void controller.turn.handleConfirmTurnRevert()}
      onClose={() => controller.turn.setTurnRevertTarget(null)}
    />
  </>
);

export interface CanvasSnapshotControlsProps {
  projectId?: string;
  onPreviewStateChange?: (previewTurn: ChatArtifactSnapshotResponse | null) => void;
}

export const CanvasSnapshotControls: React.FC<CanvasSnapshotControlsProps> = ({
  projectId,
  onPreviewStateChange,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const ctrl = useSnapshotWorkspaceController(projectId, onPreviewStateChange);
  const latestTurn = ctrl.turnSnapshots[0] || null;

  return (
    <>
      <div className="snap-canvas-strip" data-testid="canvas-snapshot-strip">
        <div className="snap-item-actions">
          <button type="button" className="snap-btn snap-btn--compact" data-testid="toggle-version-history-btn" onClick={() => ctrl.setIsDrawerOpen((p) => !p)}>
            {t("snapshots.historyTitle", "Version History")} ({formatNumber(ctrl.snapshots.length)})
          </button>
          <button type="button" className="snap-btn snap-btn--primary snap-btn--compact" data-testid="open-export-handoff-btn" onClick={() => void ctrl.exp.handleOpenExport("zip")}>
            {t("snapshots.exportHandoffBtn", "Export / IDE Handoff")}
          </button>
        </div>
        {latestTurn && (
          <TurnSnapshotReceiptCard
            turnSnapshot={latestTurn}
            isPreviewActive={ctrl.turn.activePreviewTurn?.id === latestTurn.id}
            forkedSessionId={ctrl.turn.forkedSessionByTurnId[latestTurn.id]?.sessionId ?? null}
            detachedNodeIds={ctrl.turn.forkedSessionByTurnId[latestTurn.id]?.detachedNodeIds ?? []}
            onPreview={ctrl.turn.handlePreviewTurn}
            onUndoTurn={(tSnap) => void ctrl.turn.handleUndoTurn(tSnap)}
            onForkSession={(tSnap) => void ctrl.turn.handleForkTurn(tSnap)}
          />
        )}
      </div>
      <HistoricalPreviewBanner activePreviewTurn={ctrl.turn.activePreviewTurn} onExit={ctrl.turn.handlePreviewTurn} />
      <CanvasSnapshotDrawerAndModalHost projectId={projectId} controller={ctrl} />
    </>
  );
};
