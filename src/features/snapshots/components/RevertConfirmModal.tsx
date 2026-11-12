import React from "react";
import { VersionSnapshotResponse } from "../../../lib/validations/version-snapshot";

export interface RevertConfirmModalProps {
  isOpen: boolean;
  targetSnapshot: VersionSnapshotResponse | null;
  isReverting: boolean;
  onConfirm: () => void;
  onClose: () => void;
}

const SafetyNotice: React.FC = () => (
  <div className="mt-4 rounded-md border border-border bg-secondary p-3 text-xs text-secondary-foreground">
    <p className="font-medium">Cơ chế an toàn (Safety First)</p>
    <p className="mt-1">
      Hệ thống sẽ tự động chụp bản lưu an toàn <span className="font-mono">pre_revert_safety</span> trước khi ghi đè tệp.
    </p>
  </div>
);

const ModalActionButtons: React.FC<{
  isReverting: boolean;
  onClose: () => void;
  onConfirm: () => void;
}> = ({ isReverting, onClose, onConfirm }) => (
  <div className="mt-6 flex justify-end gap-3">
    <button type="button" onClick={onClose} disabled={isReverting} className="rounded px-4 py-2 text-sm font-medium hover:bg-muted">
      Hủy bỏ
    </button>
    <button
      type="button"
      onClick={onConfirm}
      disabled={isReverting}
      className="rounded bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
    >
      {isReverting ? "Đang khôi phục..." : "Xác nhận Khôi phục"}
    </button>
  </div>
);

export const RevertConfirmModal: React.FC<RevertConfirmModalProps> = ({
  isOpen,
  targetSnapshot,
  isReverting,
  onConfirm,
  onClose,
}) => {
  if (!isOpen || !targetSnapshot) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-md rounded-lg border bg-background p-6 shadow-xl">
        <h3 className="text-lg font-semibold text-foreground">Xác nhận khôi phục phiên bản</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          Khôi phục về <span className="font-mono font-medium text-foreground">{targetSnapshot.versionTag}</span> ({targetSnapshot.label}).
        </p>
        <SafetyNotice />
        <ModalActionButtons isReverting={isReverting} onClose={onClose} onConfirm={onConfirm} />
      </div>
    </div>
  );
};
