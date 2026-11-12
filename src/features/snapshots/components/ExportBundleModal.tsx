import React, { useState } from "react";
import { ExportZipResponse } from "../../../lib/validations/version-snapshot";

export interface ExportBundleModalProps {
  isOpen: boolean;
  metadata: ExportZipResponse | null;
  isExporting: boolean;
  onDownloadZip: () => void;
  onCreateGitHubPR: (branch: string, title: string) => void;
  onClose: () => void;
}

const MetadataCard: React.FC<{ metadata: ExportZipResponse }> = ({ metadata }) => (
  <div className="mt-4 rounded border p-3 text-xs">
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">Tệp xuất bản:</span>
      <span className="font-mono font-medium">{metadata.fileName}</span>
    </div>
    <div className="mt-1 flex items-center justify-between">
      <span className="text-muted-foreground">Dung lượng:</span>
      <span className="font-mono">{metadata.byteLength} bytes</span>
    </div>
    <div className="mt-2">
      <span className={`inline-flex rounded-full px-2 py-0.5 font-medium ${
        metadata.readyToRun ? "bg-emerald-500/10 text-emerald-500" : "bg-secondary text-secondary-foreground"
      }`}>
        {metadata.readyToRun ? "✓ Sẵn sàng chạy (pnpm install && pnpm build)" : "Thiếu tài nguyên"}
      </span>
    </div>
  </div>
);

const FormInputs: React.FC<{
  branch: string;
  setBranch: (v: string) => void;
  title: string;
  setTitle: (v: string) => void;
}> = ({ branch, setBranch, title, setTitle }) => (
  <div className="mt-4 space-y-3">
    <div>
      <label className="text-xs font-medium text-muted-foreground">Nhánh GitHub</label>
      <input
        type="text"
        value={branch}
        onChange={(e) => setBranch(e.target.value)}
        className="mt-1 w-full rounded border bg-transparent px-3 py-1.5 text-sm"
      />
    </div>
    <div>
      <label className="text-xs font-medium text-muted-foreground">Tiêu đề Pull Request</label>
      <input
        type="text"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        className="mt-1 w-full rounded border bg-transparent px-3 py-1.5 text-sm"
      />
    </div>
  </div>
);

const ActionRow: React.FC<{
  isExporting: boolean;
  onClose: () => void;
  onPR: () => void;
  onDownload: () => void;
}> = ({ isExporting, onClose, onPR, onDownload }) => (
  <div className="mt-6 flex justify-end gap-2">
    <button type="button" onClick={onClose} className="rounded px-3 py-2 text-sm font-medium hover:bg-muted">
      Đóng
    </button>
    <button
      type="button"
      onClick={onPR}
      disabled={isExporting}
      className="rounded border border-primary/40 px-3 py-2 text-sm font-medium text-primary hover:bg-primary/10 disabled:opacity-50"
    >
      Tạo GitHub PR
    </button>
    <button
      type="button"
      onClick={onDownload}
      disabled={isExporting}
      className="rounded bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
    >
      {isExporting ? "Đang xuất..." : "Tải ZIP Full Repo"}
    </button>
  </div>
);

export const ExportBundleModal: React.FC<ExportBundleModalProps> = ({
  isOpen,
  metadata,
  isExporting,
  onDownloadZip,
  onCreateGitHubPR,
  onClose,
}) => {
  const [branch, setBranch] = useState("feature/design-intent-export");
  const [prTitle, setPrTitle] = useState("Sync executable UI design intent");

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-lg rounded-lg border bg-background p-6 shadow-xl">
        <h3 className="text-lg font-semibold text-foreground">Xuất bản Mã nguồn Toàn diện (Full Repo)</h3>
        <p className="mt-1 text-sm text-muted-foreground">Đóng gói mã nguồn dự án chạy được độc lập hoặc tạo GitHub PR.</p>
        {metadata && <MetadataCard metadata={metadata} />}
        <FormInputs branch={branch} setBranch={setBranch} title={prTitle} setTitle={setPrTitle} />
        <ActionRow
          isExporting={isExporting}
          onClose={onClose}
          onPR={() => onCreateGitHubPR(branch, prTitle)}
          onDownload={onDownloadZip}
        />
      </div>
    </div>
  );
};
