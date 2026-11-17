// @story 05.3
import React, { useMemo, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  type ChatArtifactSnapshotResponse,
  type SnapshotTrigger,
  type VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";
import { TurnSnapshotReceiptCard } from "./TurnSnapshotReceiptCard";

export type SnapshotFilterTab = "all" | SnapshotTrigger;

export interface DrawerSidebarProps {
  snapshots: VersionSnapshotResponse[];
  turnSnapshots?: ChatArtifactSnapshotResponse[];
  activePreviewTurnId?: string | null;
  forkedSessionByTurnId?: Record<string, { sessionId: string; detachedNodeIds: string[] }>;
  selectedEntryKey?: string | null;
  detailNode?: React.ReactNode;
  onClose: () => void;
  onSelect: (snapshot: VersionSnapshotResponse) => void;
  onInspectSnapshot?: (snapshot: VersionSnapshotResponse) => void;
  onInspectTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onOpenExport: () => void;
  onCreateCheckpoint?: () => void;
  onPreviewTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onUndoTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onForkTurn?: (turn: ChatArtifactSnapshotResponse) => void;
}

function getTriggerBadgeClass(trigger: SnapshotTrigger): string {
  if (trigger === "auto_proposal_apply") return "snap-badge snap-badge-auto";
  if (trigger === "pre_revert_safety" || trigger === "pre_apply_safety") {
    return "snap-badge snap-badge-safety";
  }
  return "snap-badge snap-badge-manual";
}

function getTriggerLabel(trigger: SnapshotTrigger, t: (k: string, fb?: string) => string): string {
  if (trigger === "auto_proposal_apply") return t("snapshots.tabAutoApply", "Auto-Apply");
  if (trigger === "pre_revert_safety" || trigger === "pre_apply_safety") {
    return t("snapshots.tabSafetyGuards", "Safety Guards");
  }
  return t("snapshots.tabManual", "Manual");
}

const MIN_DUAL_WIDTH_PX = 620;
const FALLBACK_VIEWPORT_WIDTH = 1280;
const MAX_DUAL_WIDTH_RATIO = 0.88;

const SnapshotItem: React.FC<{
  snapshot: VersionSnapshotResponse;
  isSelected?: boolean;
  onInspect?: (s: VersionSnapshotResponse) => void;
  onSelectRevert: (s: VersionSnapshotResponse) => void;
}> = ({ snapshot, isSelected = false, onInspect, onSelectRevert }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();

  return (
    <div
      className={`snap-item-card${isSelected ? " snap-item-card--selected" : ""}`}
      data-testid={`snapshot-item-${snapshot.id}`}
      onClick={onInspect ? () => onInspect(snapshot) : undefined}
      style={onInspect ? { cursor: "pointer" } : undefined}
    >
      <div className="snap-item-header">
        <span className="snap-item-tag">{snapshot.versionTag}</span>
        <span className={getTriggerBadgeClass(snapshot.trigger)}>
          {getTriggerLabel(snapshot.trigger, t)}
        </span>
      </div>
      <p className="snap-item-label">{snapshot.label}</p>
      <div className="snap-item-meta">
        <span>{formatNumber(snapshot.files.length)} {t("snapshots.filesUnit", "files")}</span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onSelectRevert(snapshot);
          }}
          data-testid={`snapshot-restore-btn-${snapshot.id}`}
          className="snap-btn snap-btn--compact"
        >
          {t("snapshots.restoreBtn", "Restore")}
        </button>
      </div>
    </div>
  );
};

const SnapshotTabsBar: React.FC<{
  activeTab: SnapshotFilterTab;
  onSelectTab: (tab: SnapshotFilterTab) => void;
}> = ({ activeTab, onSelectTab }) => {
  const { t } = useTranslation();
  const tabs: ReadonlyArray<{ id: SnapshotFilterTab; label: string }> = [
    { id: "all", label: t("snapshots.tabAll", "All") },
    { id: "manual", label: t("snapshots.tabManual", "Manual") },
    { id: "auto_proposal_apply", label: t("snapshots.tabAutoApply", "Auto-Apply") },
    { id: "pre_revert_safety", label: t("snapshots.tabSafetyGuards", "Safety Guards") },
  ];

  return (
    <div className="snap-drawer-tabs" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={activeTab === tab.id}
          data-testid={`snapshot-filter-tab-${tab.id}`}
          className={`snap-drawer-tab${activeTab === tab.id ? " snap-drawer-tab--active" : ""}`}
          onClick={() => onSelectTab(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
};

const EmptySnapshotsState: React.FC<{ onCreateCheckpoint?: () => void }> = ({ onCreateCheckpoint }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-drawer-empty" data-testid="snapshots-empty-state">
      <span>{t("snapshots.emptyTimelineText", "No snapshots recorded for this filter yet.")}</span>
      {onCreateCheckpoint && (
        <button
          type="button"
          className="snap-btn snap-btn--primary snap-btn--compact"
          data-testid="create-first-checkpoint-btn"
          onClick={onCreateCheckpoint}
        >
          {t("snapshots.createCheckpointBtn", "Create First Checkpoint")}
        </button>
      )}
    </div>
  );
};

const TurnReceiptsSection: React.FC<{
  turnSnapshots: ChatArtifactSnapshotResponse[];
  activePreviewTurnId: string | null;
  selectedEntryKey?: string | null;
  forkedSessionByTurnId: Record<string, { sessionId: string; detachedNodeIds: string[] }>;
  onInspectTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onPreviewTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onUndoTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onForkTurn?: (turn: ChatArtifactSnapshotResponse) => void;
}> = ({
  turnSnapshots,
  activePreviewTurnId,
  selectedEntryKey,
  forkedSessionByTurnId,
  onInspectTurn,
  onPreviewTurn,
  onUndoTurn,
  onForkTurn,
}) => {
  const { t } = useTranslation();
  if (turnSnapshots.length === 0 || !onPreviewTurn || !onUndoTurn || !onForkTurn) return null;
  return (
    <>
      <h3 className="snap-drawer-section-title">{t("snapshots.turnReceiptsHeading", "Per-Turn Receipts")}</h3>
      {turnSnapshots.map((turn) => {
        const isSelected = selectedEntryKey === `turn:${turn.id}`;
        return (
          <div
            key={turn.id}
            className={isSelected ? "snap-item-card--selected" : undefined}
            onClick={(e) => {
              if ((e.target as HTMLElement).closest("button")) return;
              onInspectTurn?.(turn);
            }}
          >
            <TurnSnapshotReceiptCard
              turnSnapshot={turn}
              isPreviewActive={activePreviewTurnId === turn.id}
              forkedSessionId={forkedSessionByTurnId[turn.id]?.sessionId ?? null}
              detachedNodeIds={forkedSessionByTurnId[turn.id]?.detachedNodeIds ?? []}
              onPreview={onPreviewTurn}
              onUndoTurn={onUndoTurn}
              onForkSession={onForkTurn}
            />
          </div>
        );
      })}
    </>
  );
};

const DrawerHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-drawer-header">
      <h2 className="snap-drawer-title">{t("snapshots.historyTitle", "Version History")}</h2>
      <button
        type="button"
        onClick={onClose}
        aria-label={t("common.close", "Close")}
        data-testid="version-drawer-close-btn"
        className="snap-drawer-close-btn"
      >
        {"✕"}
      </button>
    </div>
  );
};

const DrawerFooter: React.FC<{ onOpenExport: () => void }> = ({ onOpenExport }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-drawer-footer">
      <button
        type="button"
        onClick={onOpenExport}
        data-testid="open-export-bundle-modal-btn"
        className="snap-btn snap-btn--primary snap-btn--full"
      >
        {t("snapshots.exportBundleBtn", "Export Full Bundle (ZIP / PR / IDE)")}
      </button>
    </div>
  );
};

function useDualDrawerResize(): {
  dualWidthPx: number | null;
  handleStartResize: (ev: React.MouseEvent<HTMLDivElement>) => void;
} {
  const [dualWidthPx, setDualWidthPx] = useState<number | null>(null);
  const handleStartResize = (ev: React.MouseEvent<HTMLDivElement>) => {
    ev.preventDefault();
    const onMove = (moveEv: MouseEvent) => {
      const vw = typeof window !== "undefined" ? window.innerWidth : FALLBACK_VIEWPORT_WIDTH;
      const clamped = Math.min(Math.floor(vw * MAX_DUAL_WIDTH_RATIO), Math.max(MIN_DUAL_WIDTH_PX, vw - moveEv.clientX));
      setDualWidthPx(clamped);
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };
  return { dualWidthPx, handleStartResize };
}

const DrawerMasterBody: React.FC<{
  activeTab: SnapshotFilterTab;
  setActiveTab: (t: SnapshotFilterTab) => void;
  filteredSnapshots: VersionSnapshotResponse[];
  props: DrawerSidebarProps;
}> = ({ activeTab, setActiveTab, filteredSnapshots, props }) => (
  <>
    <DrawerHeader onClose={props.onClose} />
    <SnapshotTabsBar activeTab={activeTab} onSelectTab={setActiveTab} />
    <div className="snap-drawer-list">
      {filteredSnapshots.length === 0 ? (
        <EmptySnapshotsState onCreateCheckpoint={props.onCreateCheckpoint} />
      ) : (
        filteredSnapshots.map((s) => (
          <SnapshotItem
            key={s.id}
            snapshot={s}
            isSelected={props.selectedEntryKey === `snap:${s.id}`}
            onInspect={props.onInspectSnapshot}
            onSelectRevert={props.onSelect}
          />
        ))
      )}
      <TurnReceiptsSection
        turnSnapshots={props.turnSnapshots ?? []}
        activePreviewTurnId={props.activePreviewTurnId ?? null}
        selectedEntryKey={props.selectedEntryKey}
        forkedSessionByTurnId={props.forkedSessionByTurnId ?? {}}
        onInspectTurn={props.onInspectTurn}
        onPreviewTurn={props.onPreviewTurn}
        onUndoTurn={props.onUndoTurn}
        onForkTurn={props.onForkTurn}
      />
    </div>
    <DrawerFooter onOpenExport={props.onOpenExport} />
  </>
);

export const DrawerSidebar: React.FC<DrawerSidebarProps> = (props) => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SnapshotFilterTab>("all");
  const { dualWidthPx, handleStartResize } = useDualDrawerResize();
  const filteredSnapshots = useMemo(
    () =>
      activeTab === "all"
        ? props.snapshots
        : props.snapshots.filter((item) =>
            activeTab === "pre_revert_safety"
              ? item.trigger === "pre_revert_safety" || item.trigger === "pre_apply_safety"
              : item.trigger === activeTab
          ),
    [activeTab, props.snapshots]
  );

  const isDual = Boolean(props.detailNode);
  const masterContent = (
    <DrawerMasterBody activeTab={activeTab} setActiveTab={setActiveTab} filteredSnapshots={filteredSnapshots} props={props} />
  );

  return (
    <aside
      className={`snap-drawer-panel${isDual ? " snap-drawer-panel--dual" : ""}`}
      style={isDual && dualWidthPx ? { width: `${dualWidthPx}px` } : undefined}
      role="dialog"
      aria-label={t("snapshots.historyTitle", "Version History")}
      data-testid="version-history-drawer"
    >
      {isDual ? (
        <>
          <div className="snap-drawer-resize-handle" data-testid="version-drawer-resize-handle" onMouseDown={handleStartResize} />
          <div className="snap-master-detail-body">
            <div className="snap-master-pane">{masterContent}</div>
            {props.detailNode}
          </div>
        </>
      ) : (
        masterContent
      )}
    </aside>
  );
};
