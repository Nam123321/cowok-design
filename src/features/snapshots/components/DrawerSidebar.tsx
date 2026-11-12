import React from "react";
import {
  SnapshotTrigger,
  VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";

function getTriggerBadgeClass(trigger: SnapshotTrigger): string {
  if (trigger === "auto_proposal_apply") return "bg-cyan-500/10 text-cyan-500 border-cyan-500/20";
  if (trigger === "pre_revert_safety") return "bg-secondary text-secondary-foreground border-border";
  return "bg-purple-500/10 text-purple-500 border-purple-500/20";
}

const SnapshotItem: React.FC<{
  snapshot: VersionSnapshotResponse;
  onSelectRevert: (s: VersionSnapshotResponse) => void;
}> = ({ snapshot, onSelectRevert }) => (
  <div className="rounded-lg border bg-card p-3 transition-colors hover:border-primary/40">
    <div className="flex items-center justify-between">
      <span className="font-mono text-sm font-semibold">{snapshot.versionTag}</span>
      <span className={`rounded-full border px-2 py-0.5 text-xs font-medium ${getTriggerBadgeClass(snapshot.trigger)}`}>
        {snapshot.trigger}
      </span>
    </div>
    <p className="mt-1 text-sm font-medium">{snapshot.label}</p>
    <div className="mt-2 flex items-center justify-between text-xs text-muted-foreground">
      <span>{snapshot.files.length} tệp tin</span>
      <button
        type="button"
        onClick={() => onSelectRevert(snapshot)}
        className="rounded border border-primary/30 px-2 py-1 text-xs font-medium text-primary hover:bg-primary/10"
      >
        Khôi phục
      </button>
    </div>
  </div>
);

export const DrawerSidebar: React.FC<{
  snapshots: VersionSnapshotResponse[];
  onClose: () => void;
  onSelect: (s: VersionSnapshotResponse) => void;
  onOpenExport: () => void;
}> = ({ snapshots, onClose, onSelect, onOpenExport }) => (
  <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-md flex-col border-l bg-background shadow-2xl">
    <div className="flex items-center justify-between border-b px-5 py-4">
      <h2 className="text-base font-semibold">Lịch sử Phiên bản</h2>
      <button type="button" onClick={onClose} className="rounded p-1 hover:bg-muted">✕</button>
    </div>
    <div className="flex-1 space-y-2 overflow-y-auto p-4">
      {snapshots.map((s) => (
        <SnapshotItem key={s.id} snapshot={s} onSelectRevert={onSelect} />
      ))}
    </div>
    <div className="border-t p-4">
      <button
        type="button"
        onClick={onOpenExport}
        className="w-full rounded bg-primary py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        Xuất bản Toàn diện (ZIP / PR)
      </button>
    </div>
  </div>
);
