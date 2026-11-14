import React, { useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { ChangeProposalResponse, FileDiffItem } from "../../../lib/validations/change-proposal";
import { DiffViewer } from "./DiffViewer";
import { ValidationGateBadge } from "./ValidationGateBadge";

interface DrawerProps {
  proposal: ChangeProposalResponse | null;
  isOpen: boolean;
  onClose: () => void;
  onApply: (proposalId: string) => void;
  onReject: (proposalId: string) => void;
}

const ProposalHeader: React.FC<{ proposal: ChangeProposalResponse; onClose: () => void }> = ({ proposal, onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between pb-3 border-b border-border">
      <div>
        <h3 className="text-sm font-bold text-foreground">{proposal.title}</h3>
        <p className="text-xs text-muted-foreground">{proposal.description || t("proposals.noExtraDesc", "Không có mô tả bổ sung")}</p>
      </div>
      <button type="button" onClick={onClose} aria-label={t("common.close", "Đóng")} className="p-1 rounded text-muted-foreground hover:bg-muted">
        ✕
      </button>
    </div>
  );
};

const ProposalFileItem: React.FC<{ file: FileDiffItem }> = ({ file }) => {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border border-border rounded-md overflow-hidden bg-card">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between p-2 text-xs hover:bg-muted/50 transition-colors"
      >
        <span className="font-mono font-medium text-foreground">{file.path}</span>
        <div className="flex items-center gap-2">
          <span className="text-emerald-500 font-mono">+{file.additions}</span>
          <span className="text-rose-500 font-mono">-{file.deletions}</span>
          <span className="text-muted-foreground text-[10px]">{expanded ? "▲" : "▼"}</span>
        </div>
      </button>
      {expanded && (
        <div className="p-2 border-t border-border bg-muted/20">
          <DiffViewer patch={file.patch} />
        </div>
      )}
    </div>
  );
};

interface FooterProps {
  proposal: ChangeProposalResponse;
  onApply: () => void;
  onReject: () => void;
}

const ProposalActionFooter: React.FC<FooterProps> = ({ proposal, onApply, onReject }) => {
  const { t } = useTranslation();
  const hasFailedCheck = proposal.validationChecks.some((c) => c.status === "failed");
  const canApply = proposal.status === "ready" && proposal.validationStatus === "passed" && !hasFailedCheck;

  return (
    <div className="flex items-center justify-between pt-3 border-t border-border">
      <button type="button" onClick={onReject} className="px-3 py-1.5 text-xs rounded-md border border-border">
        {t("proposals.rejectBtn", "Từ Chối (Reject)")}
      </button>
      <button
        type="button"
        disabled={!canApply}
        onClick={onApply}
        title={!canApply ? t("proposals.applyBlockedHint", "Apply bị khóa do kiểm tra an toàn chưa đạt [AC-03]") : ""}
        className="px-4 py-1.5 text-xs font-semibold rounded-md bg-primary text-primary-foreground disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {t("proposals.applyBtn", "Áp Dụng Thay Đổi (Apply)")}
      </button>
    </div>
  );
};

export const ChangeProposalDrawer: React.FC<DrawerProps> = ({
  proposal,
  isOpen,
  onClose,
  onApply,
  onReject,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  if (!isOpen || !proposal) return null;

  return (
    <div className="fixed inset-y-0 right-0 z-50 w-full max-w-lg bg-background border-l border-border p-4 shadow-2xl flex flex-col justify-between">
      <div className="flex flex-col gap-4 overflow-y-auto">
        <ProposalHeader proposal={proposal} onClose={onClose} />
        <ValidationGateBadge checks={proposal.validationChecks} />
        <div className="flex flex-col gap-2">
          <span className="text-xs font-semibold text-foreground">
            {t("proposals.modifiedFilesLabel", "Danh Sách Tệp Đã Sửa Đổi")} ({formatNumber(proposal.files.length)})
          </span>
          {proposal.files.map((f) => (
            <ProposalFileItem key={f.path} file={f} />
          ))}
        </div>
      </div>
      <ProposalActionFooter
        proposal={proposal}
        onApply={() => onApply(proposal.id)}
        onReject={() => onReject(proposal.id)}
      />
    </div>
  );
};
