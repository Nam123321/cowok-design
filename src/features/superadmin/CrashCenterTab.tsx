// @story 14.2
// @implements AC-04 (DESIGN.md [5.21] DataTableView — Crash Center Grouped by Fingerprint & App Version)
import React, { useState } from "react";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { CrashGroup } from "../../lib/validations/cloud-admin";

export interface CrashCenterTabProps {
  crashes: CrashGroup[];
  loading: boolean;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const CRASH_TABLE_COL_SPAN = 6;

const CRASH_STATUS_TONES: Record<CrashGroup["status"], { fg: string; bg: string }> = {
  resolved: { fg: "var(--state-success)", bg: "var(--state-success-soft)" },
  investigating: { fg: "var(--state-warning)", bg: "var(--state-warning-soft)" },
  open: { fg: "var(--state-danger)", bg: "var(--state-danger-soft)" },
};

const WRAP_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
  overflowX: "auto",
};

const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "10px 12px",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
  backgroundColor: "var(--bg-subtle)",
  borderBottom: "1px solid var(--border-default)",
};

const TD_STYLE: React.CSSProperties = {
  padding: "9px 12px",
  fontSize: "13px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
  height: "40px",
};

function CrashStatusBadge({ status }: { status: CrashGroup["status"] }) {
  const tone = CRASH_STATUS_TONES[status];
  return (
    <span
      style={{
        display: "inline-block",
        padding: "2px 8px",
        borderRadius: "999px",
        fontSize: "11px",
        fontWeight: FONT_WEIGHT_BOLD,
        color: tone.fg,
        backgroundColor: tone.bg,
      }}
    >
      {status}
    </span>
  );
}

function CrashGroupRow({
  crash,
  expanded,
  onToggle,
}: {
  crash: CrashGroup;
  expanded: boolean;
  onToggle: (fp: string) => void;
}) {
  const { formatDate, formatNumber } = useFormatter();
  return (
    <>
      <tr
        data-testid={`crash-row-${crash.fingerprint}`}
        onClick={() => onToggle(crash.fingerprint)}
        style={{ cursor: "pointer", backgroundColor: expanded ? "var(--bg-selected)" : "transparent" }}
      >
        <td style={TD_STYLE}>
          <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD }}>{crash.errorSignature}</div>
          <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{crash.fingerprint}</div>
        </td>
        <td style={TD_STYLE}>v{crash.appVersion}</td>
        <td style={TD_STYLE}>{formatNumber(crash.occurrenceCount)}</td>
        <td style={TD_STYLE}>{formatNumber(crash.affectedInstalls)}</td>
        <td style={TD_STYLE}><CrashStatusBadge status={crash.status} /></td>
        <td style={TD_STYLE}>{formatDate(crash.lastSeenAt)}</td>
      </tr>
      {expanded && (
        <tr data-testid={`crash-stack-${crash.fingerprint}`}>
          <td colSpan={CRASH_TABLE_COL_SPAN} style={{ padding: "10px 14px", backgroundColor: "var(--bg-subtle)" }}>
            <pre style={{ margin: 0, fontSize: "11px", color: "var(--text-secondary)", overflowX: "auto" }}>
              {crash.sampleStackSanitized}
            </pre>
          </td>
        </tr>
      )}
    </>
  );
}

export const CrashCenterTab: React.FC<CrashCenterTabProps> = ({ crashes, loading }) => {
  const { t } = useTranslation();
  const [expandedFp, setExpandedFp] = useState<string | null>(null);
  const toggleRow = (fp: string) => setExpandedFp((prev) => (prev === fp ? null : fp));

  return (
    <div style={WRAP_STYLE} data-ff-id="superadmin-crash-table" data-testid="superadmin-crash-table">
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th style={TH_STYLE}>{t("superadmin.crashes.colSignature", "Error Signature & Fingerprint")}</th>
            <th style={TH_STYLE}>{t("superadmin.crashes.colVersion", "App Version")}</th>
            <th style={TH_STYLE}>{t("superadmin.crashes.colOccurrences", "Occurrences")}</th>
            <th style={TH_STYLE}>{t("superadmin.crashes.colInstalls", "Affected Installs")}</th>
            <th style={TH_STYLE}>{t("superadmin.crashes.colStatus", "Status")}</th>
            <th style={TH_STYLE}>{t("superadmin.crashes.colLastSeen", "Last Seen")}</th>
          </tr>
        </thead>
        <tbody>
          {crashes.length === 0 ? (
            <tr>
              <td colSpan={CRASH_TABLE_COL_SPAN} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                {loading
                  ? t("superadmin.common.loading", "Loading...")
                  : t("superadmin.crashes.empty", "No crash groups reported.")}
              </td>
            </tr>
          ) : (
            crashes.map((crash) => (
              <CrashGroupRow
                key={crash.fingerprint}
                crash={crash}
                expanded={expandedFp === crash.fingerprint}
                onToggle={toggleRow}
              />
            ))
          )}
        </tbody>
      </table>
    </div>
  );
};
