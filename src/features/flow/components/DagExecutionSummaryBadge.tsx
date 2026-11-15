// @story 04.4
// @implements AC-01, AC-03 (DAG Execution Summary Badge with Daemon Cancel Dispatch)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type { NodeExecutionJob, NodeQueueSummary } from "../store/keyed-parallel-coordinator";

const BADGE_ICON_SM = 11;
const BADGE_ICON_MD = 12;

const ExecutionQueuePopover: React.FC<{
  activeJobs: NodeExecutionJob[];
  onCancelJob: (jobId: string) => void;
}> = ({ activeJobs, onCancelJob }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div
      role="dialog"
      aria-label={t("flow.runningJobsAria", "Danh sách tác vụ đang chạy")}
      style={{
        position: "absolute", top: "34px", left: "50%", transform: "translateX(-50%)",
        width: "300px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-md)", boxShadow: "var(--shadow-lg)", padding: "8px",
        display: "flex", flexDirection: "column", gap: "6px", fontSize: "11px",
      }}
    >
      <div style={{ fontWeight: 600, borderBottom: "1px solid var(--border-subtle)", paddingBottom: "4px" }}>
        {t("flow.executionQueueTitle", "Hàng đợi thực thi")} ({formatNumber(activeJobs.length)})
      </div>
      {activeJobs.map((job) => (
        <div
          key={job.id}
          style={{
            display: "flex", alignItems: "center", justifyContent: "space-between",
            padding: "4px 6px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)",
          }}
        >
          <div style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}>
            <span style={{ fontWeight: 600, color: job.status === "running" ? "var(--action-primary)" : "var(--state-warning)" }}>
              {job.status.toUpperCase()}
            </span>
            <span style={{ marginLeft: "4px", color: "var(--text-secondary)" }}>{job.prompt || job.nodeId}</span>
          </div>
          <button
            type="button"
            onClick={() => onCancelJob(job.id)}
            title={t("floatingChat.cancelJob", "Hủy lệnh")}
            style={{ border: "none", background: "transparent", color: "var(--state-danger)", cursor: "pointer", fontSize: "11px", padding: "0 4px" }}
          >
            <BrandIcon name="close" size={BADGE_ICON_SM} color="currentColor" />
          </button>
        </div>
      ))}
    </div>
  );
};

export const DagExecutionSummaryBadge: React.FC<{
  summary: NodeQueueSummary;
  activeJobs: NodeExecutionJob[];
  onCancelJob: (jobId: string) => void;
}> = ({ summary, activeJobs, onCancelJob }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const [popoverOpen, setPopoverOpen] = useState(false);
  if (summary.totalActive === 0) return null;

  return (
    <div
      className="dag-execution-summary-badge"
      style={{ position: "absolute", top: "14px", left: "50%", transform: "translateX(-50%)", zIndex: 25 }}
    >
      <button
        type="button"
        onClick={() => setPopoverOpen((p) => !p)}
        style={{
          display: "flex", alignItems: "center", gap: "6px", padding: "5px 12px",
          borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-elevated)",
          border: "1px solid var(--action-primary)", color: "var(--text-primary)",
          fontSize: "11px", fontWeight: 600, boxShadow: "var(--shadow-md)", cursor: "pointer",
        }}
      >
        <BrandIcon name="execution-pipeline" size={BADGE_ICON_MD} color="var(--action-primary)" />
        <span>{formatNumber(summary.runningCount)} {t("floatingChat.processing", "Đang xử lý")}</span>
        <span style={{ color: "var(--text-muted)" }}>•</span>
        <BrandIcon name="clock" size={BADGE_ICON_SM} color="var(--state-warning)" />
        <span style={{ color: "var(--state-warning)" }}>{formatNumber(summary.queuedCount)} {t("floatingChat.queuedStatus", "Chờ")}</span>
        <BrandIcon name="chevron-down" size={BADGE_ICON_SM} color="var(--text-muted)" />
      </button>
      {popoverOpen && <ExecutionQueuePopover activeJobs={activeJobs} onCancelJob={onCancelJob} />}
    </div>
  );
};
