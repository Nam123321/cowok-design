import React, { useState } from "react";
import { VersionSnapshotResponse } from "../../../lib/validations/version-snapshot";
import { DrawerSidebar } from "./DrawerSidebar";
import { RevertConfirmModal } from "./RevertConfirmModal";
import { ExportBundleModal } from "./ExportBundleModal";

export interface VersionHistoryDrawerProps {
  isOpen: boolean;
  snapshots: VersionSnapshotResponse[];
  onClose: () => void;
  onRevert: (snapshotId: string) => Promise<void>;
  onExportZip: () => Promise<void>;
  onCreatePR: (branch: string, title: string) => Promise<void>;
}

const DrawerModals: React.FC<{
  target: VersionSnapshotResponse | null;
  setTarget: (s: VersionSnapshotResponse | null) => void;
  isReverting: boolean;
  onRevert: () => void;
  isExportOpen: boolean;
  setIsExportOpen: (v: boolean) => void;
  isExporting: boolean;
  onZip: () => void;
  onPR: (b: string, t: string) => void;
}> = (props) => (
  <>
    <RevertConfirmModal
      isOpen={Boolean(props.target)}
      targetSnapshot={props.target}
      isReverting={props.isReverting}
      onConfirm={props.onRevert}
      onClose={() => props.setTarget(null)}
    />
    <ExportBundleModal
      isOpen={props.isExportOpen}
      metadata={null}
      isExporting={props.isExporting}
      onDownloadZip={props.onZip}
      onCreateGitHubPR={props.onPR}
      onClose={() => props.setIsExportOpen(false)}
    />
  </>
);

export const VersionHistoryDrawer: React.FC<VersionHistoryDrawerProps> = ({
  isOpen,
  snapshots,
  onClose,
  onRevert,
  onExportZip,
  onCreatePR,
}) => {
  const [target, setTarget] = useState<VersionSnapshotResponse | null>(null);
  const [isReverting, setIsReverting] = useState(false);
  const [isExportOpen, setIsExportOpen] = useState(false);
  const [isExporting, setIsExporting] = useState(false);

  if (!isOpen) return null;

  const handleRevert = async () => {
    if (!target) return;
    setIsReverting(true);
    try { await onRevert(target.id); setTarget(null); } finally { setIsReverting(false); }
  };

  return (
    <>
      <DrawerSidebar
        snapshots={snapshots}
        onClose={onClose}
        onSelect={setTarget}
        onOpenExport={() => setIsExportOpen(true)}
      />
      <DrawerModals
        target={target}
        setTarget={setTarget}
        isReverting={isReverting}
        onRevert={handleRevert}
        isExportOpen={isExportOpen}
        setIsExportOpen={setIsExportOpen}
        isExporting={isExporting}
        onZip={async () => {
          setIsExporting(true);
          try { await onExportZip(); } finally { setIsExporting(false); }
        }}
        onPR={async (b, t) => {
          setIsExporting(true);
          try { await onCreatePR(b, t); } finally { setIsExporting(false); }
        }}
      />
    </>
  );
};
